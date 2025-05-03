use super::movie::{MovieView, MovieViewRenderer};
use super::RuffleGui;
use crate::desktop::custom_event::RuffleEvent;
use crate::editor::{Editor, NeedsRedraw};
use anyhow::anyhow;
use egui::{Context, ViewportId};
use ruffle_render_wgpu::backend::{request_adapter_and_device, WgpuRenderBackend};
use ruffle_render_wgpu::descriptors::Descriptors;
use ruffle_render_wgpu::target::RenderTarget;
use ruffle_render_wgpu::utils::{format_list, get_backend_names};
use std::path::Path;
use std::sync::{Arc, MutexGuard};
use std::time::{Duration, Instant};
use wgpu::PresentMode;
use winit::dpi::PhysicalSize;
use winit::event_loop::EventLoop;
use winit::window::Window;

/// Integration layer connecting wgpu+winit to egui.
pub struct GuiController {
    descriptors: Arc<Descriptors>,
    egui_winit: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    gui: RuffleGui,
    window: Arc<Window>,
    last_update: Instant,
    repaint_after: Duration,
    surface: wgpu::Surface<'static>,
    surface_format: wgpu::TextureFormat,
    movie_view_renderer: Arc<MovieViewRenderer>,
    // Note that `window.get_inner_size` can change at any point on x11, even between two lines of code.
    // Use this instead.
    size: PhysicalSize<u32>,
}

impl GuiController {
    pub fn new(
        window: Arc<Window>,
        event_loop: &EventLoop<RuffleEvent>,
        trace_path: Option<&Path>,
        backend: wgpu::Backends,
        power_preference: wgpu::PowerPreference,
    ) -> anyhow::Result<Self> {
        if wgpu::Backends::SECONDARY.contains(backend) {
            tracing::warn!(
                "{} graphics backend support may not be fully supported.",
                format_list(&get_backend_names(backend), "and")
            );
        }
        /*let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: backend,
            dx12_shader_compiler: wgpu::Dx12Compiler::default(),
        });*/
        // this tries the preferred backend first and if that doesn't work it tries the other ones
        let (instance, backend) = create_wgpu_instance(backend)?;
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::from_window(window.as_ref())?)
        }?;
        let (adapter, device, queue) = futures::executor::block_on(request_adapter_and_device(
            backend,
            &instance,
            Some(&surface),
            power_preference,
            trace_path,
        ))
        .map_err(|e| anyhow!(e.to_string()))?;
        let adapter_info = adapter.get_info();
        tracing::info!(
            "Using graphics API {} on {} (type: {:?})",
            adapter_info.backend.to_str(),
            adapter_info.name,
            adapter_info.device_type
        );
        let surface_format = surface
            .get_capabilities(&adapter)
            .formats
            .first()
            .cloned()
            .expect("At least one format should be supported");
        let size = window.inner_size();
        surface.configure(
            &device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: size.width,
                height: size.height,
                present_mode: PresentMode::AutoNoVsync,
                desired_maximum_frame_latency: 1,
                alpha_mode: Default::default(),
                view_formats: Default::default(),
            },
        );
        let event_loop = event_loop.create_proxy();
        let descriptors = Descriptors::new(instance, adapter, device, queue);
        let egui_ctx = Context::default();

        let event_loop_proxy = event_loop.clone();
        egui_ctx.set_request_repaint_callback(move |info| {
            event_loop_proxy
                .send_event(RuffleEvent::RedrawRequested(info.delay))
                .expect("Cannot send custom repaint event");
        });

        let mut egui_winit = egui_winit::State::new(
            egui_ctx,
            ViewportId::ROOT,
            window.as_ref(),
            None,
            None,
            None,
        );
        egui_winit.set_max_texture_side(descriptors.limits.max_texture_dimension_2d as usize);

        let movie_view_renderer = Arc::new(MovieViewRenderer::new(
            &descriptors.device,
            surface_format,
            window.fullscreen().is_none(),
            size.height,
        ));
        let egui_renderer =
            egui_wgpu::Renderer::new(&descriptors.device, surface_format, None, 1, true);
        let gui = RuffleGui::new(event_loop);
        Ok(Self {
            descriptors: Arc::new(descriptors),
            egui_winit,
            egui_renderer,
            gui,
            window,
            last_update: Instant::now(),
            repaint_after: Duration::ZERO,
            surface,
            surface_format,
            movie_view_renderer,
            size,
        })
    }

    pub fn descriptors(&self) -> &Arc<Descriptors> {
        &self.descriptors
    }

    #[must_use]
    pub fn handle_event(&mut self, event: &winit::event::WindowEvent) -> bool {
        if let winit::event::WindowEvent::Resized(size) = &event {
            self.surface.configure(
                &self.descriptors.device,
                &wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format: self.surface_format,
                    width: size.width,
                    height: size.height,
                    present_mode: PresentMode::AutoNoVsync,
                    desired_maximum_frame_latency: 1,
                    alpha_mode: Default::default(),
                    view_formats: Default::default(),
                },
            );
            self.movie_view_renderer.update_resolution(
                &self.descriptors,
                self.window.fullscreen().is_none(),
                size.height,
            );
            self.size = *size;
        }
        let response = self.egui_winit.on_window_event(&self.window, event);
        if response.repaint {
            self.window.request_redraw();
        }
        response.consumed
    }

    pub fn create_movie_view(&self) -> MovieView {
        MovieView::new(
            self.movie_view_renderer.clone(),
            &self.descriptors.device,
            self.size.width,
            self.size.height,
        )
    }

    pub fn render(&mut self, mut player: Option<MutexGuard<Editor>>) -> NeedsRedraw {
        let surface_texture = self
            .surface
            .get_current_texture()
            .expect("Surface became unavailable");
        // TODO: copy recreating surface code

        let mut needs_redraw = NeedsRedraw::No;

        let raw_input = self.egui_winit.take_egui_input(&self.window);
        let full_output = self.egui_winit.egui_ctx().run(raw_input, |context| {
            needs_redraw = self.gui.update(
                context,
                self.window.fullscreen().is_none(),
                player.as_deref_mut(),
            );
        });
        self.repaint_after = full_output
            .viewport_output
            .get(&ViewportId::ROOT)
            .expect("Root viewport must exist")
            .repaint_delay;

        self.egui_winit
            .handle_platform_output(&self.window, full_output.platform_output);
        let clipped_primitives = self
            .egui_winit
            .egui_ctx()
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        let scale_factor = self.window.scale_factor() as f32;
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.size.width, self.size.height],
            pixels_per_point: scale_factor,
        };

        let mut encoder =
            self.descriptors
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("egui encoder"),
                });

        for (id, image_delta) in &full_output.textures_delta.set {
            self.egui_renderer.update_texture(
                &self.descriptors.device,
                &self.descriptors.queue,
                *id,
                image_delta,
            );
        }

        let mut command_buffers = self.egui_renderer.update_buffers(
            &self.descriptors.device,
            &self.descriptors.queue,
            &mut encoder,
            &clipped_primitives,
            &screen_descriptor,
        );

        let movie_view = if let Some(player) = player.as_deref_mut() {
            let renderer = player
                .renderer_mut()
                .downcast_mut::<WgpuRenderBackend<MovieView>>()
                .expect("Renderer must be correct type");
            Some(renderer.target())
        } else {
            None
        };

        {
            let surface_view = surface_texture.texture.create_view(&Default::default());

            let mut render_pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &surface_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    label: Some("egui_render"),
                    ..Default::default()
                })
                .forget_lifetime();

            if let Some(movie_view) = movie_view {
                movie_view.render(&self.movie_view_renderer, &mut render_pass);
            }

            self.egui_renderer
                .render(&mut render_pass, &clipped_primitives, &screen_descriptor);
        }

        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        command_buffers.push(encoder.finish());
        self.descriptors.queue.submit(command_buffers);
        surface_texture.present();

        needs_redraw
    }

    pub fn show_about_screen(&mut self) {
        self.gui.show_about_screen();
    }

    /*pub fn show_context_menu(&mut self, menu: Vec<ruffle_core::ContextMenuItem>) {
        self.gui.show_context_menu(menu);
    }

    pub fn is_context_menu_visible(&self) -> bool {
        self.gui.is_context_menu_visible()
    }*/

    pub fn needs_render(&self) -> bool {
        Instant::now().duration_since(self.last_update) >= self.repaint_after
    }
}

fn create_wgpu_instance(
    preferred_backends: wgpu::Backends,
) -> anyhow::Result<(wgpu::Instance, wgpu::Backends)> {
    for backend in preferred_backends.iter() {
        if let Some(instance) = try_wgpu_backend(backend) {
            tracing::info!(
                "Using preferred backend {}",
                format_list(&get_backend_names(backend), "and")
            );
            return Ok((instance, backend));
        }
    }

    tracing::warn!(
        "Preferred backend(s) of {} not available; falling back to any",
        format_list(&get_backend_names(preferred_backends), "or")
    );

    for backend in wgpu::Backends::all() - preferred_backends {
        if let Some(instance) = try_wgpu_backend(backend) {
            tracing::info!(
                "Using fallback backend {}",
                format_list(&get_backend_names(backend), "and")
            );
            return Ok((instance, backend));
        }
    }

    Err(anyhow!(
        "No compatible graphics backends of any kind were available"
    ))
}

fn try_wgpu_backend(backend: wgpu::Backends) -> Option<wgpu::Instance> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: backend,
        flags: wgpu::InstanceFlags::default()
            .with_env()
            // the gpu driver for my gpu is considered non-compliant and OpenGL on my machine doesn't support
            // the Immediate present mode for some reason, this makes it choose vulkan anyway
            .union(wgpu::InstanceFlags::ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER),

        ..Default::default()
    });
    if instance.enumerate_adapters(backend).is_empty() {
        None
    } else {
        Some(instance)
    }
}
