use flits_core::Movie;

use super::cli::Opt;
use super::custom_event::RuffleEvent;
use super::gui::{GuiController, MovieView, MENU_HEIGHT};
use super::player::PlayerController;
use super::util::{
    get_screen_size, parse_url, pick_file
};
use anyhow::{Context, Error};
use rfd::MessageDialogResult;
use ruffle_render::backend::ViewportDimensions;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, Modifiers, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes};

pub struct App {
    opt: Opt,
    window: Arc<Window>,
    event_loop: Option<EventLoop<RuffleEvent>>,
    gui: Arc<Mutex<GuiController>>,
    player: PlayerController,
    min_window_size: LogicalSize<u32>,
    max_window_size: PhysicalSize<u32>,
    redraw_next_wait: bool,
}

impl App {
    pub fn new(opt: Opt) -> Result<Self, Error> {
        let movie_url = if let Some(path) = &opt.input_path {
            Some(parse_url(path).context("Couldn't load specified path")?)
        } else {
            None
        };

        /*let icon_bytes = include_bytes!("../assets/favicon-32.rgba");
        let icon =
            Icon::from_rgba(icon_bytes.to_vec(), 32, 32).context("Couldn't load app icon")?;*/

        let event_loop = EventLoop::with_user_event().build()?;

        let min_window_size = (16, MENU_HEIGHT + 16).into();

        let window_attributes = WindowAttributes::default()
            .with_visible(false)
            .with_title("Flits Editor")
            //.with_window_icon(Some(icon))
            .with_min_inner_size(min_window_size);
        let window = event_loop.create_window(window_attributes)?;
        let max_window_size = get_screen_size(&window);
        window.set_max_inner_size(Some(max_window_size));
        let window = Arc::new(window);

        let gui = GuiController::new(
            window.clone(),
            &event_loop,
            opt.trace_path(),
            opt.graphics.into(),
            opt.power.into(),
        )?;

        let mut player = PlayerController::new(
            event_loop.create_proxy(),
            window.clone(),
            gui.descriptors().clone(),
        );

        if let Some(movie_url) = movie_url {
            player.create(&opt, movie_url, gui.create_movie_view());
        }
        

        Ok(Self {
            opt,
            window,
            event_loop: Some(event_loop),
            gui: Arc::new(Mutex::new(gui)),
            player,
            min_window_size,
            max_window_size,
            redraw_next_wait: false,
        })
    }

    pub fn run(mut self) -> Result<(), Error> {
        enum LoadingState {
            Loading,
            WaitingForResize,
            Loaded,
        }
        let mut loaded = LoadingState::Loading;
        let mut mouse_pos = PhysicalPosition::new(0.0, 0.0);
        //let mut time = Instant::now();
        //let mut next_frame_time = Instant::now();
        let mut minimized = false;
        let mut modifiers = Modifiers::default();
        //let mut fullscreen_down = false;

        //if self.opt.input_path.is_none() {
            // No SWF provided on command line; show window with dummy movie immediately.
            self.window.set_visible(true);
            loaded = LoadingState::Loaded;
        //}

        // Poll UI events.
        let event_loop = self.event_loop.take().expect("App already running");
        event_loop.run(move |event, elwt| {
            let mut check_redraw = false;
            let mut redraw_delay: Option<std::time::Duration> = None;
            match event {
                winit::event::Event::LoopExiting => {
                    /*if let Some(mut player) = self.player.get() {
                        player.flush_shared_objects();
                    }*/
                    crate::shutdown();
                    return;
                }

                // Core loop
                winit::event::Event::AboutToWait => {
                    if self.redraw_next_wait {
                        self.redraw_next_wait = false;
                        check_redraw = true;
                    }
                },
                /*    if matches!(loaded, LoadingState::Loaded) =>
                {
                    println!("Doing frame loop");
                    let new_time = Instant::now();
                    let dt = new_time.duration_since(time).as_micros();
                    if dt > 0 {
                        time = new_time;
                        /*if let Some(mut player) = self.player.get() {
                            player.tick(dt as f64 / 1000.0);
                            next_frame_time = new_time + player.time_til_next_frame();
                        }*/
                        check_redraw = true;
                    }
                }*/

                // Render
                winit::event::Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    // Don't render when minimized to avoid potential swap chain errors in `wgpu`.
                    if !minimized {
                        if let Some(mut player) = self.player.get() {
                            player.render();
                            /*let renderer = player
                                .renderer_mut()
                                .downcast_mut::<WgpuRenderBackend<MovieView>>()
                                .expect("Renderer must be correct type");*/
                            let needs_redraw = self.gui
                                .lock()
                                .expect("Gui lock")
                                .render(Some(player));
                            match needs_redraw {
                                crate::editor::NeedsRedraw::Yes => self.window.request_redraw(),
                                crate::editor::NeedsRedraw::No => (),
                            }
                        } else {
                            self.gui.lock().expect("Gui lock").render(None);
                        }
                        #[cfg(feature = "tracy")]
                        tracing_tracy::client::Client::running()
                            .expect("tracy client must be running")
                            .frame_mark();
                    }
                }

                winit::event::Event::WindowEvent { event, .. } => {
                    if self.gui.lock().expect("Gui lock").handle_event(&event) {
                        // Event consumed by GUI.
                        return;
                    }
                    let height_offset = if self.window.fullscreen().is_some() {
                        0
                    } else {
                        MENU_HEIGHT
                    };
                    match event {
                        WindowEvent::CloseRequested => {
                            elwt.exit();
                            return;
                        }
                        WindowEvent::Resized(size) => {
                            // TODO: Change this when winit adds a `Window::minimzed` or `WindowEvent::Minimize`.
                            minimized = size.width == 0 && size.height == 0;

                            if let Some(mut player) = self.player.get() {
                                let viewport_scale_factor = self.window.scale_factor();
                                player.set_viewport_dimensions(ViewportDimensions {
                                    width: size.width,
                                    height: size.height - height_offset,
                                    scale_factor: viewport_scale_factor,
                                });
                            }
                            self.window.request_redraw();
                            if matches!(loaded, LoadingState::WaitingForResize) {
                                loaded = LoadingState::Loaded;
                            }
                        }
                        WindowEvent::CursorMoved { position, .. } => {
                            /*if self.gui.lock().expect("Gui lock").is_context_menu_visible() {
                                return;
                            }*/

                            if let Some(mut player) = self.player.get() {
                                mouse_pos = PhysicalPosition {
                                    x: position.x,
                                    y: position.y - height_offset as f64,
                                };
                                /*let event = PlayerEvent::MouseMove {
                                    x: position.x,
                                    y: position.y - height_offset as f64,
                                };
                                player.handle_event(event);*/
                                player.handle_mouse_move(mouse_pos.x, mouse_pos.y);
                            }
                            check_redraw = true;
                        }
                        WindowEvent::MouseInput { button, state, .. } => {
                            /*if self.gui.lock().expect("Gui lock").is_context_menu_visible() {
                                return;
                            }*/

                            if let Some(mut player) = self.player.get() {
                                let x = mouse_pos.x;
                                let y = mouse_pos.y;
                                /*if state == ElementState::Pressed
                                    && button == RuffleMouseButton::Right
                                {
                                    // Show context menu.
                                    // TODO: Should be squelched if player consumes the right click event.
                                    let context_menu = player.prepare_context_menu();
                                    self.gui
                                        .lock()
                                        .expect("Gui lock")
                                        .show_context_menu(context_menu);
                                }*/
                                player.handle_mouse_input(x, y, button, state);
                            }
                            check_redraw = true;
                        }
                        WindowEvent::MouseWheel { delta, .. } => {
                            /*use ruffle_core::events::MouseWheelDelta;
                            use winit::event::MouseScrollDelta;
                            if let Some(mut player) = self.player.get() {
                                let delta = match delta {
                                    MouseScrollDelta::LineDelta(_, dy) => {
                                        MouseWheelDelta::Lines(dy.into())
                                    }
                                    MouseScrollDelta::PixelDelta(pos) => {
                                        MouseWheelDelta::Pixels(pos.y)
                                    }
                                };
                                let event = PlayerEvent::MouseWheel { delta };
                                player.handle_event(event);
                            }*/
                            check_redraw = true;
                        }
                        WindowEvent::CursorEntered { .. } => {
                            /*if let Some(mut player) = self.player.get() {
                                player.set_mouse_in_stage(true);
                                if player.needs_render() {
                                    self.window.request_redraw();
                                }
                            }*/
                        }
                        WindowEvent::CursorLeft { .. } => {
                            /*if let Some(mut player) = self.player.get() {
                                player.set_mouse_in_stage(false);
                                player.handle_event(PlayerEvent::MouseLeave);
                            }*/
                            check_redraw = true;
                        }
                        WindowEvent::ModifiersChanged(new_modifiers) => {
                            modifiers = new_modifiers;
                        }
                        WindowEvent::KeyboardInput { event, .. } => {
                        }
                        _ => (),
                    }
                }
                /*winit::event::Event::UserEvent(RuffleEvent::OnMetadata(swf_header)) => {
                    let movie_width = swf_header.stage_size().width().to_pixels();
                    let movie_height = swf_header.stage_size().height().to_pixels();
                    let height_offset = if self.window.fullscreen().is_some() {
                        0
                    } else {
                        MENU_HEIGHT
                    };

                    let window_size: Size = match (self.opt.width, self.opt.height) {
                        (None, None) => {
                            LogicalSize::new(movie_width, movie_height + height_offset as f64)
                                .into()
                        }
                        (Some(width), None) => {
                            let scale = width / movie_width;
                            let height = movie_height * scale;
                            PhysicalSize::new(
                                width.max(1.0),
                                height.max(1.0) + height_offset as f64,
                            )
                            .into()
                        }
                        (None, Some(height)) => {
                            let scale = height / movie_height;
                            let width = movie_width * scale;
                            PhysicalSize::new(
                                width.max(1.0),
                                height.max(1.0) + height_offset as f64,
                            )
                            .into()
                        }
                        (Some(width), Some(height)) => PhysicalSize::new(
                            width.max(1.0),
                            height.max(1.0) + height_offset as f64,
                        )
                        .into(),
                    };

                    let window_size = Size::clamp(
                        window_size,
                        self.min_window_size.into(),
                        self.max_window_size.into(),
                        self.window.scale_factor(),
                    );

                    self.window.set_inner_size(window_size);
                    self.window.set_fullscreen(if self.opt.fullscreen {
                        Some(Fullscreen::Borderless(None))
                    } else {
                        None
                    });
                    self.window.set_visible(true);

                    let viewport_size = self.window.inner_size();

                    // On X11 (and possibly other platforms), the window size is not updated immediately.
                    // Wait for the window to be resized to the requested size before we start running
                    // the SWF (which can observe the viewport size in "noScale" mode)
                    if window_size != viewport_size.into() {
                        loaded = LoadingState::WaitingForResize;
                    } else {
                        loaded = LoadingState::Loaded;
                    }

                    let viewport_scale_factor = self.window.scale_factor();
                    if let Some(mut player) = self.player.get() {
                        player.set_viewport_dimensions(ViewportDimensions {
                            width: viewport_size.width,
                            height: viewport_size.height - height_offset,
                            scale_factor: viewport_scale_factor,
                        });
                    }
                }*/
                
                winit::event::Event::UserEvent(RuffleEvent::NewFile(new_project_data)) => {
                    if !new_project_data.path.is_dir() {
                        rfd::MessageDialog::new()
                            .set_description("Invalid path.")
                            .show();
                        return;
                    }
                    if !new_project_data.path.read_dir().unwrap().next().is_none() {
                        if rfd::MessageDialog::new()
                            .set_buttons(rfd::MessageButtons::OkCancel)
                            .set_description("The directory is not empty, are you sure you want to create a project in this directory?")
                            .show() != MessageDialogResult::Yes {
                            return;
                        }
                    }
                    let json_path = new_project_data.path.join("movie.json");
                    let movie = Movie::from_properties(new_project_data.movie_properties);
                    movie.save(&json_path);
                    let url = parse_url(&json_path).expect("Couldn't load specified path");
                    self.player.create(&self.opt, url, self.gui.lock().expect("Gui lock").create_movie_view());
                }

                winit::event::Event::UserEvent(RuffleEvent::OpenFile) => {
                    if let Some(path) = pick_file() {
                        // TODO: Show dialog on error.
                        let url = parse_url(&path).expect("Couldn't load specified path");
                        self.player.create(
                            &self.opt,
                            url,
                            self.gui.lock().expect("Gui lock").create_movie_view(),
                        );
                    }
                }

                winit::event::Event::UserEvent(RuffleEvent::CloseFile) => {
                    self.player.destroy();
                }

                winit::event::Event::UserEvent(RuffleEvent::ExitRequested) => {
                    elwt.exit();
                    return;
                }
                
                winit::event::Event::UserEvent(RuffleEvent::About) => {
                    self.gui.lock().expect("Gui locked").show_about_screen();
                }
                
                winit::event::Event::UserEvent(RuffleEvent::CommandOutput(line)) => {
                    if let Some(mut player) = self.player.get() {
                        match player.receive_command_output(line) {
                            crate::editor::NeedsRedraw::Yes =>  self.window.request_redraw(),
                            _  => ()
                        }
                    }
                }
                
                winit::event::Event::UserEvent(RuffleEvent::RuffleClosed) => {
                    if let Some(mut player) = self.player.get() {
                        player.on_ruffle_closed();
                        self.window.request_redraw();
                    }
                }
                
                winit::event::Event::UserEvent(RuffleEvent::RedrawRequested(delay)) => {
                    redraw_delay = Some(delay);
                }

                _ => (),
            }

            // Check for a redraw request.
            if check_redraw {
                //let player = self.player.get();
                let gui = self.gui.lock().expect("Gui lock");
                if /*player.map(|p| p.needs_render()).unwrap_or_default() ||*/ gui.needs_render() {
                    self.window.request_redraw();
                }
            }

            // unlike ruffle (at the time of writing), we use set_request_repaint_callback
            elwt.set_control_flow(if let Some(delay) = redraw_delay {
                if let Some(redraw_after_instant) =
                    std::time::Instant::now().checked_add(delay)
                {
                    self.redraw_next_wait = true;
                    ControlFlow::WaitUntil(redraw_after_instant)
                } else {
                    ControlFlow::Wait
                }
            } else {
                ControlFlow::Wait
            }); 
            // ruffle version:
            // After polling events, sleep the event loop until the next event or the next frame.
            /*elwt.set_control_flow(if matches!(loaded, LoadingState::Loaded) {
                /*if let Some(next_frame_time) = next_frame_time {
                    ControlFlow::WaitUntil(next_frame_time)
                } else {*/
                    // prevent 100% cpu use
                    // TODO: use set_request_repaint_callback to correctly get egui repaint requests.
                    ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(10))
                //}
            } else {
                ControlFlow::Wait
            });*/
        })?;
        Ok(())
    }
}
