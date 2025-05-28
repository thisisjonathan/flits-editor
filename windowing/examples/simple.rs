use std::{
    sync::{Arc, Mutex, MutexGuard},
    time::Instant,
};

use anyhow::anyhow;
use anyhow::Error;
use ruffle_render::backend::RenderBackend;
use ruffle_render_wgpu::{backend::WgpuRenderBackend, descriptors::Descriptors};
use wgpu::{Backends, PowerPreference};
use windowing::{Config, GuiController, MovieView, Player, PlayerController, RuffleGui};
use winit::{
    application::ApplicationHandler,
    event::{StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::WindowAttributes,
};

struct MyGui {}
impl RuffleGui for MyGui {
    type Player = MyPlayer;
    type Arguments = ();

    fn update(
        &self,
        egui_ctx: &egui::Context,
        _show_menu: bool,
        _player: Option<&mut Self::Player>,
        _menu_height_offset: f64,
    ) -> () {
        egui::Window::new("Test Window").show(egui_ctx, |ui| {
            ui.label("Hello, world!");
            // TODO: inputs to check if tab works
        });
    }

    fn is_context_menu_visible(&self) -> bool {
        false
    }
}
struct MyPlayer {
    renderer: Box<dyn RenderBackend>,
}
impl Player for MyPlayer {
    fn renderer_mut(&mut self) -> &mut dyn RenderBackend {
        &mut *self.renderer
    }
}
impl MyPlayer {
    fn render(&mut self) {
        self.renderer.submit_frame(
            swf::Color::GREEN,
            ruffle_render::commands::CommandList::new(),
            vec![],
        );
    }
}
struct MyPlayerController {
    descriptors: Arc<Descriptors>,
    player: Option<Mutex<MyPlayer>>,
}
impl PlayerController for MyPlayerController {
    type Player = MyPlayer;
    type Arguments = ();

    fn create(&mut self, _arguments: &Self::Arguments, movie_view: MovieView) {
        let renderer = WgpuRenderBackend::new(self.descriptors.clone(), movie_view)
            .map_err(|e| anyhow!(e.to_string()))
            .expect("Couldn't create wgpu rendering backend");
        self.player = Some(Mutex::new(MyPlayer {
            renderer: Box::new(renderer),
        }));
    }

    fn destroy(&mut self) {}

    fn get(&self) -> Option<MutexGuard<MyPlayer>> {
        match &self.player {
            None => None,
            Some(player) => Some(player.try_lock().expect("Player lock must be available")),
        }
    }
}

type MyCustomEvent = ();

struct MainWindow {
    minimized: bool,
    time: Instant,
    next_frame_time: Option<Instant>,
    gui: GuiController<MyGui>,
    player: MyPlayerController,
}
impl MainWindow {
    pub fn create_movie(&mut self) {
        let nothing = ();
        self.gui.create_movie(&mut self.player, &nothing);
    }
    pub fn window_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        if matches!(event, WindowEvent::RedrawRequested) {
            // Don't render when minimized to avoid potential swap chain errors in `wgpu`.
            if !self.minimized {
                if let Some(mut player) = self.player.get() {
                    // Even if the movie is paused, user interaction with debug tools can change the render output
                    player.render();
                    self.gui.render(Some(player));
                } else {
                    self.gui.render(None);
                }
                //plot_stats_in_tracy(&self.gui.descriptors().wgpu_instance);
            }

            // Important that we return here, or we'll get a feedback loop with egui
            // (winit says redraw, egui hears redraw and says redraw, we hear redraw and tell winit to redraw...)
            return;
        }

        if self.gui.handle_event(&event) {
            // Event consumed by GUI.
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                // TODO: Change this when winit adds a `Window::minimized` or `WindowEvent::Minimize`.
                self.minimized = size.width == 0 && size.height == 0;

                /*if let Some(mut player) = self.player.get() {
                    let viewport_scale_factor = self.gui.window().scale_factor();
                    player.set_viewport_dimensions(ViewportDimensions {
                        width: size.width,
                        height: size.height.saturating_sub(self.gui.height_offset() as u32),
                        scale_factor: viewport_scale_factor,
                    });
                }*/
                self.gui.window().request_redraw();
                /*if matches!(self.loaded, LoadingState::WaitingForResize) {
                    self.loaded = LoadingState::Loaded;
                }*/
            }
            WindowEvent::CursorMoved { position, .. } => {
                /*if self.gui.is_context_menu_visible() {
                    return;
                }

                self.mouse_pos = position;
                let (x, y) = self.gui.window_to_movie_position(position);
                let event = PlayerEvent::MouseMove { x, y };
                self.player.handle_event(event);*/
                self.check_redraw();
            }
            WindowEvent::DroppedFile(file) => {
                /*if let Ok(url) = parse_url(&file) {
                    self.gui.create_movie(
                        &mut self.player,
                        LaunchOptions::from(&self.preferences),
                        url,
                    );
                }*/
            }
            WindowEvent::Focused(true) => {
                //self.player.handle_event(PlayerEvent::FocusGained);
            }
            WindowEvent::Focused(false) => {
                //self.player.handle_event(PlayerEvent::FocusLost);
            }
            WindowEvent::MouseInput { button, state, .. } => {
                /*if self.gui.is_context_menu_visible() {
                    return;
                }

                use ruffle_core::events::MouseButton as RuffleMouseButton;
                use winit::event::MouseButton;
                let (x, y) = self.gui.window_to_movie_position(self.mouse_pos);
                let button = match button {
                    MouseButton::Left => RuffleMouseButton::Left,
                    MouseButton::Right => RuffleMouseButton::Right,
                    MouseButton::Middle => RuffleMouseButton::Middle,
                    _ => RuffleMouseButton::Unknown,
                };
                let event = match state {
                    // TODO We should get information about click index from the OS,
                    //   but winit does not support that yet.
                    ElementState::Pressed => PlayerEvent::MouseDown {
                        x,
                        y,
                        button,
                        index: None,
                    },
                    ElementState::Released => PlayerEvent::MouseUp { x, y, button },
                };
                let handled = self.player.handle_event(event);
                if !handled && state == ElementState::Pressed && button == RuffleMouseButton::Right
                {
                    // Show context menu.
                    if let Some(mut player) = self.player.get() {
                        let context_menu = player.prepare_context_menu();

                        // MouseUp event will be ignored when the context menu is shown,
                        // but it has to be dispatched when the menu closes.
                        let close_event = PlayerEvent::MouseUp {
                            x,
                            y,
                            button: RuffleMouseButton::Right,
                        };
                        self.gui.show_context_menu(context_menu, close_event);
                    }
                }*/
                self.check_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                /*if self.gui.is_context_menu_visible() {
                    return;
                }

                use ruffle_core::events::MouseWheelDelta;
                use winit::event::MouseScrollDelta;
                let delta = match delta {
                    MouseScrollDelta::LineDelta(_, dy) => MouseWheelDelta::Lines(dy.into()),
                    MouseScrollDelta::PixelDelta(pos) => MouseWheelDelta::Pixels(pos.y),
                };
                let event = PlayerEvent::MouseWheel { delta };
                self.player.handle_event(event);*/
                self.check_redraw();
            }
            WindowEvent::CursorEntered { .. } => {
                /*if let Some(mut player) = self.player.get() {
                    player.set_mouse_in_stage(true);
                    if player.needs_render() {
                        self.gui.window().request_redraw();
                    }
                }*/
            }
            WindowEvent::CursorLeft { .. } => {
                /*if let Some(mut player) = self.player.get() {
                    player.set_mouse_in_stage(false);
                }
                self.player.handle_event(PlayerEvent::MouseLeave);*/
                self.check_redraw();
            }
            WindowEvent::ModifiersChanged(new_modifiers) => {
                //self.modifiers = new_modifiers;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                /*if self.gui.is_context_menu_visible() {
                    return;
                }

                // Handle escaping from fullscreen.
                if let KeyEvent {
                    state: ElementState::Pressed,
                    logical_key: Key::Named(NamedKey::Escape),
                    ..
                } = event
                {
                    let _ = self
                        .event_loop_proxy
                        .send_event(RuffleEvent::ExitFullScreen);
                }

                let key = winit_input_to_ruffle_key_descriptor(&event);
                match event.state {
                    ElementState::Pressed => {
                        self.player.handle_event(PlayerEvent::KeyDown { key });
                        if let Some(control_code) =
                            winit_to_ruffle_text_control(&event, &self.modifiers)
                        {
                            self.player
                                .handle_event(PlayerEvent::TextControl { code: control_code });
                        } else if let Some(text) = event.text {
                            for codepoint in text.chars() {
                                self.player
                                    .handle_event(PlayerEvent::TextInput { codepoint });
                            }
                        }
                    }
                    ElementState::Released => {
                        self.player.handle_event(PlayerEvent::KeyUp { key });
                    }
                };*/
                self.check_redraw();
            }
            /*WindowEvent::Ime(ime) => match ime {
                Ime::Enabled => {}
                Ime::Preedit(text, cursor) => {
                    self.player
                        .handle_event(PlayerEvent::Ime(ImeEvent::Preedit(text, cursor)));
                }
                Ime::Commit(text) => {
                    self.player
                        .handle_event(PlayerEvent::Ime(ImeEvent::Commit(text)));
                }
                Ime::Disabled => {}
            },*/
            _ => (),
        }
    }

    fn about_to_wait(&mut self) {
        // Core loop
        // [NA] This used to be called `MainEventsCleared`, but I think the behaviour is different now.
        // We should look at changing our tick to happen somewhere else if we see any behavioural problems.
        let new_time = Instant::now();
        let dt = new_time.duration_since(self.time).as_nanos();
        if dt > 0 {
            self.time = new_time;
            /*if let Some(mut player) = self.player.get() {
                player.tick(dt as f64 / 1_000_000.0);
                self.next_frame_time = Some(new_time + player.time_til_next_frame());
            } else {*/
            self.next_frame_time = None;
            //}
            self.check_redraw();
        }
    }

    fn check_redraw(&self) {
        if self.gui.needs_render() {
            self.gui.window().request_redraw();
        }
    }
}
struct App {
    main_window: Option<MainWindow>,
    event_loop_proxy: EventLoopProxy<MyCustomEvent>,
}

impl ApplicationHandler<MyCustomEvent> for App {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if cause == StartCause::Init {
            let window_attributes = WindowAttributes::default()
                .with_title("Windowing Sample Program")
                .with_visible(true);
            let window = event_loop
                .create_window(window_attributes)
                .expect("Window should be created");
            let window = Arc::new(window);
            let gui = GuiController::new(
                window,
                self.event_loop_proxy.clone(),
                Config {
                    preferred_backends: Backends::all(),
                    power_preference: PowerPreference::None,
                    trace_path: None,
                },
                MyGui {},
                false,
            )
            .unwrap();
            self.main_window = Some(MainWindow {
                time: Instant::now(),
                next_frame_time: None,
                minimized: false,
                player: MyPlayerController {
                    player: None,
                    descriptors: gui.descriptors().clone(),
                },
                gui,
            });
            self.main_window.as_mut().unwrap().create_movie();
        }
    }
    fn resumed(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {}

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        println!("Event: {:?}", event);
        if let Some(main_window) = &mut self.main_window {
            main_window.window_event(event_loop, event);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(main_window) = &mut self.main_window {
            main_window.about_to_wait();

            // The event loop is finished; let's find out how long we need to wait for.
            // We don't need to worry about earlier update requests, as it's the
            // only place where we're setting control flow, and events cancel wait.
            // Note: the control flow might be set to `ControlFlow::WaitUntil` with a
            // timestamp in the past! Take that into consideration when changing this code.
            if let Some(next_frame_time) = main_window.next_frame_time {
                event_loop.set_control_flow(ControlFlow::WaitUntil(next_frame_time));
            }
        }
    }
}

fn main() -> Result<(), Error> {
    let event_loop: EventLoop<()> = EventLoop::with_user_event().build()?;
    let mut app = App {
        main_window: None,
        event_loop_proxy: event_loop.create_proxy(),
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}
