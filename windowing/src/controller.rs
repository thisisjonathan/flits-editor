use crate::movie::{MovieView, MovieViewRenderer};
use crate::{Config, NeedsRedraw, Player, PlayerController, RuffleGui};
use anyhow::anyhow;
use egui::{Context, ViewportId};
use ruffle_render_wgpu::backend::{request_adapter_and_device, WgpuRenderBackend};
use ruffle_render_wgpu::descriptors::Descriptors;
use ruffle_render_wgpu::utils::{format_list, get_backend_names};
use std::any::Any;
use std::sync::{Arc, MutexGuard};
use std::time::{Duration, Instant};
use wgpu::SurfaceError;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::WindowEvent;
use winit::keyboard::{Key, NamedKey};
use winit::window::Window;

/// Integration layer connecting wgpu+winit to egui.
pub struct GuiController<G: RuffleGui> {
    descriptors: Arc<Descriptors>,
    egui_winit: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    gui: G,
    window: Arc<Window>,
    last_update: Instant,
    repaint_after: Duration,
    surface: wgpu::Surface<'static>,
    surface_format: wgpu::TextureFormat,
    present_mode: wgpu::PresentMode,
    desired_maximum_frame_latency: u32,
    movie_view_renderer: Arc<MovieViewRenderer>,
    // Note that `window.get_inner_size` can change at any point on x11, even between two lines of code.
    // Use this instead.
    size: PhysicalSize<u32>,
    /// If this is set, we should not render the main menu.
    no_gui: bool,
    height_offset_unscaled: u32,
    send_tab_to_player: bool,
}

impl<G: RuffleGui> GuiController<G> {
    pub fn new(window: Arc<Window>, config: Config, gui: G, no_gui: bool) -> anyhow::Result<Self> {
        let (instance, backend) = create_wgpu_instance(config.preferred_backends)?;
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::from_window(window.as_ref())?)
        }?;
        let (adapter, device, queue) = futures::executor::block_on(request_adapter_and_device(
            backend,
            &instance,
            Some(&surface),
            config.power_preference,
            config.trace_path,
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
                present_mode: config.present_mode,
                desired_maximum_frame_latency: config.desired_maximum_frame_latency,
                alpha_mode: Default::default(),
                view_formats: Default::default(),
            },
        );
        let descriptors = Descriptors::new(instance, adapter, device, queue);
        let egui_ctx = Context::default();

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
            Self::height_offset_scaled_without_self(
                window.fullscreen().is_some(),
                window.scale_factor(),
                config.height_offset_unscaled,
                no_gui,
            ) / size.height as f64,
        ));
        let egui_renderer =
            egui_wgpu::Renderer::new(&descriptors.device, surface_format, None, 1, true);
        let descriptors = Arc::new(descriptors);

        gui.after_window_init(window.clone(), egui_winit.egui_ctx());

        Ok(Self {
            descriptors,
            egui_winit,
            egui_renderer,
            gui,
            window,
            last_update: Instant::now(),
            repaint_after: Duration::ZERO,
            surface,
            surface_format,
            present_mode: config.present_mode,
            desired_maximum_frame_latency: config.desired_maximum_frame_latency,
            movie_view_renderer,
            size,
            no_gui,
            height_offset_unscaled: config.height_offset_unscaled,
            send_tab_to_player: config.send_tab_to_player,
        })
    }

    pub fn gui(&self) -> &G {
        &self.gui
    }

    pub fn gui_mut(&mut self) -> &mut G {
        &mut self.gui
    }

    pub fn descriptors(&self) -> &Arc<Descriptors> {
        &self.descriptors
    }

    pub fn window(&self) -> &Arc<Window> {
        &self.window
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width > 0 && size.height > 0 {
            self.size = size;
            self.reconfigure_surface();
        }
    }

    pub fn reconfigure_surface(&mut self) {
        self.surface.configure(
            &self.descriptors.device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: self.surface_format,
                width: self.size.width,
                height: self.size.height,
                present_mode: self.present_mode,
                desired_maximum_frame_latency: self.desired_maximum_frame_latency,
                alpha_mode: Default::default(),
                view_formats: Default::default(),
            },
        );
        self.movie_view_renderer.update_resolution(
            &self.descriptors,
            self.height_offset_scaled() / self.size.height as f64,
        );
    }

    #[must_use]
    pub fn handle_event(&mut self, event: &WindowEvent) -> bool {
        if let WindowEvent::Resized(size) = &event {
            self.resize(*size);
        }

        if self.send_tab_to_player
            && matches!(
                &event,
                WindowEvent::KeyboardInput {
                    event: winit::event::KeyEvent {
                        logical_key: Key::Named(NamedKey::Tab),
                        ..
                    },
                    ..
                }
            )
        {
            // Prevent egui from consuming the Tab key.
            return false;
        }

        let response = self.egui_winit.on_window_event(&self.window, event);
        if response.repaint {
            self.window.request_redraw();
        }
        response.consumed
    }

    pub fn close_movie<T: PlayerController>(&mut self, player: &mut T) {
        player.destroy();
        self.gui.on_player_destroyed();
    }

    pub fn create_movie<T: PlayerController>(&mut self, player: &mut T, arguments: &G::Arguments)
    where
        T: PlayerController<Arguments = G::Arguments, Player = G::Player>,
    {
        self.close_movie(player);
        let movie_view = MovieView::new(
            self.movie_view_renderer.clone(),
            &self.descriptors.device,
            self.size.width,
            self.size.height,
        );
        player.create(arguments, movie_view);
        self.gui.on_player_created(
            arguments,
            player
                .get()
                .expect("Player must exist after being created."),
        );
    }

    pub fn height_offset_scaled(&self) -> f64 {
        Self::height_offset_scaled_without_self(
            self.window.fullscreen().is_some(),
            self.window.scale_factor(),
            self.height_offset_unscaled,
            self.no_gui,
        )
    }

    fn height_offset_scaled_without_self(
        is_fullscreen: bool,
        scale_factor: f64,
        height_offset_unscaled: u32,
        no_gui: bool,
    ) -> f64 {
        if is_fullscreen || no_gui {
            0.0
        } else {
            height_offset_unscaled as f64 * scale_factor
        }
    }

    pub fn set_height_offset_unscaled(&mut self, height_offset_unscaled: u32) {
        self.height_offset_unscaled = height_offset_unscaled;
        self.reconfigure_surface();
    }

    pub fn window_to_movie_position(&self, position: PhysicalPosition<f64>) -> (f64, f64) {
        let x = position.x;
        let y = position.y - self.height_offset_scaled();
        (x, y)
    }

    pub fn movie_to_window_position(&self, x: f64, y: f64) -> PhysicalPosition<f64> {
        let y = y + self.height_offset_scaled();
        PhysicalPosition::new(x, y)
    }

    pub fn render(&mut self, mut player: Option<MutexGuard<G::Player>>) -> NeedsRedraw {
        let surface_texture = match self.surface.get_current_texture() {
            Ok(surface_texture) => surface_texture,
            Err(e @ (SurfaceError::Lost | SurfaceError::Outdated)) => {
                // Reconfigure the surface if lost or outdated.
                // Some sources suggest ignoring `Outdated` and waiting for the next frame,
                // but I suspect this advice is related explicitly to resizing,
                // because the future resize event will reconfigure the surface.
                // However, resizing is not the only possible reason for the surface
                // to become outdated (resolution / refresh rate change, some internal
                // platform-specific reasons, wgpu bugs?).
                // Testing on Vulkan shows that reconfiguring the surface works in that case.
                tracing::warn!("Surface became unavailable: {:?}, reconfiguring", e);
                self.reconfigure_surface();
                return NeedsRedraw::Yes;
            }
            Err(e @ SurfaceError::Timeout) => {
                // An operation related to the surface took too long to complete.
                // This error may happen due to many reasons (GPU overload, GPU driver bugs, etc.),
                // the best thing we can do is skip a frame and wait.
                tracing::warn!("Surface became unavailable: {:?}, skipping a frame", e);
                return NeedsRedraw::No;
            }
            Err(SurfaceError::OutOfMemory) => {
                // Cannot help with that :(
                panic!("wgpu: Out of memory: no more memory left to allocate a new frame");
            }
            Err(SurfaceError::Other) => {
                // Generic error, not much we can do.
                panic!("wgpu: Acquiring a texture failed with a generic error");
            }
        };

        let mut needs_redraw = NeedsRedraw::No;

        let raw_input = self.egui_winit.take_egui_input(&self.window);
        let show_menu = self.window.fullscreen().is_none() && !self.no_gui;
        let mut full_output = self.egui_winit.egui_ctx().run(raw_input, |context| {
            needs_redraw = self.gui.update(
                context,
                show_menu,
                player.as_deref_mut(),
                self.height_offset_scaled(),
            );
        });
        self.repaint_after = full_output
            .viewport_output
            .get(&ViewportId::ROOT)
            .expect("Root viewport must exist")
            .repaint_delay;

        // If we're not in a UI, tell egui which cursor we prefer to use instead
        if !self.egui_winit.egui_ctx().wants_pointer_input() {
            if let Some(icon) = self.gui.cursor_icon() {
                full_output.platform_output.cursor_icon = icon;
            }
        }
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
            let renderer =
                <dyn Any>::downcast_ref::<WgpuRenderBackend<MovieView>>(player.renderer_mut())
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
        self.window.pre_present_notify();
        surface_texture.present();

        needs_redraw
    }

    pub fn needs_render(&self) -> bool {
        Instant::now().duration_since(self.last_update) >= self.repaint_after
    }

    pub fn set_ime_allowed(&self, allowed: bool) {
        self.window.set_ime_allowed(allowed);
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
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: backend,
        flags: wgpu::InstanceFlags::default().with_env(),
        ..Default::default()
    });
    if instance.enumerate_adapters(backend).is_empty() {
        None
    } else {
        Some(instance)
    }
}
