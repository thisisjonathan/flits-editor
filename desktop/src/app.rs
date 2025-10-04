use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::anyhow;
use flits_editor_lib::FlitsEvent;
use ruffle_render::{backend::RenderBackend, quality::StageQuality};
use ruffle_render_wgpu::{backend::WgpuRenderBackend, descriptors::Descriptors};
use wgpu::{Backends, PowerPreference};
use windowing::{
    Config, GuiController, MovieView, NeedsRedraw, PlayerController, RuffleGui, RuffleWindow,
};
use winit::{
    application::ApplicationHandler,
    event::StartCause,
    event_loop::{ActiveEventLoop, EventLoopProxy},
    window::WindowAttributes,
};

use crate::{cli::CliParams, player::FlitsPlayer};

struct FlitsArguments {
    event_loop: EventLoopProxy<FlitsEvent>,
    cli_params: CliParams,
}

struct FlitsGui {}
impl RuffleGui for FlitsGui {
    type Player = FlitsPlayer;
    type Arguments = FlitsArguments;

    fn on_player_created(
        &self,
        _arguments: &Self::Arguments,
        _player: MutexGuard<Self::Player>,
    ) -> () {
    }

    fn update(
        &self,
        egui_ctx: &egui::Context,
        _show_menu: bool,
        player: Option<&mut Self::Player>,
        _menu_height_offset: f64,
    ) -> NeedsRedraw {
        if let Some(player) = player {
            // we need the conversion because they are defined in different crates and they don't depend on each other
            return match player.do_ui(egui_ctx) {
                flits_editor_lib::NeedsRedraw::Yes => NeedsRedraw::Yes,
                flits_editor_lib::NeedsRedraw::No => NeedsRedraw::No,
            };
        }
        NeedsRedraw::No
    }

    fn on_player_destroyed(&self) {}

    fn cursor_icon(&self) -> Option<egui::CursorIcon> {
        None
    }

    fn after_window_init(&self, _window: Arc<winit::window::Window>, _egui_ctx: &egui::Context) {}
    fn after_render(&self, _instance: &wgpu::Instance) {}
}
struct FlitsPlayerController {
    descriptors: Arc<Descriptors>,
    player: Option<Mutex<FlitsPlayer>>,
}
impl PlayerController for FlitsPlayerController {
    type Player = FlitsPlayer;
    type Arguments = FlitsArguments;

    fn create(&mut self, arguments: &Self::Arguments, movie_view: MovieView) {
        let mut renderer = WgpuRenderBackend::new(self.descriptors.clone(), movie_view)
            .map_err(|e| anyhow!(e.to_string()))
            .expect("Couldn't create wgpu rendering backend");
        // this sets the quality to High which turns on anti-aliasing
        // this is the same default as in Ruffle
        renderer.set_quality(StageQuality::default());
        self.player = Some(Mutex::new(FlitsPlayer::new(
            Box::new(renderer),
            arguments.event_loop.clone(),
            arguments.cli_params.clone(),
        )));
    }

    fn destroy(&mut self) {}

    fn get(&self) -> Option<MutexGuard<FlitsPlayer>> {
        match &self.player {
            None => None,
            Some(player) => Some(player.try_lock().expect("Player lock must be available")),
        }
    }
}

pub struct App {
    main_window: Option<RuffleWindow<FlitsGui, FlitsPlayerController>>,
    event_loop: EventLoopProxy<FlitsEvent>,
    title: String,
    cli_params: CliParams,
}
impl App {
    pub fn new(event_loop: EventLoopProxy<FlitsEvent>, cli_params: CliParams) -> Self {
        App {
            main_window: None,
            event_loop,
            title: String::new(),
            cli_params,
        }
    }
}
impl ApplicationHandler<FlitsEvent> for App {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if cause == StartCause::Init {
            let window_attributes = WindowAttributes::default()
                .with_title("Flits Editor")
                .with_visible(true);
            let window = event_loop
                .create_window(window_attributes)
                .expect("Window should be created");
            let window = Arc::new(window);
            let gui = GuiController::new(
                window,
                Config {
                    preferred_backends: Backends::all(),
                    power_preference: PowerPreference::None,
                    trace_path: None,
                    // without this, dragged things lag behind the cursor
                    present_mode: wgpu::PresentMode::AutoNoVsync,
                    // changing this from the default 2 doesn't seem to have an effect but change it anyway to be sure
                    desired_maximum_frame_latency: 1,
                    height_offset_unscaled: 0,
                    send_tab_to_player: false,
                },
                FlitsGui {},
                false,
            )
            .unwrap();
            let descriptors = gui.descriptors().clone();
            self.main_window = Some(RuffleWindow::new(
                gui,
                FlitsPlayerController {
                    player: None,
                    descriptors,
                },
            ));
            let arguments = FlitsArguments {
                event_loop: self.event_loop.clone(),
                cli_params: self.cli_params.clone(),
            };
            self.main_window.as_mut().unwrap().create_movie(&arguments);
        }
    }
    fn resumed(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {}

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        if let Some(main_window) = &mut self.main_window {
            if main_window.window_event(event_loop, event.clone()) {
                return;
            }
            if let Some(player) = &mut main_window.player_mut().player {
                player
                    .try_lock()
                    .expect("Player lock must be available")
                    .window_event(event_loop, event);
            }
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: FlitsEvent) {
        if let Some(main_window) = &mut self.main_window {
            if matches!(event, FlitsEvent::UpdateTitle) {
                self.title = main_window.player_mut().get().unwrap().title();
                main_window.window().set_title(&self.title);
            }
            if matches!(event, FlitsEvent::UpdateHeightOffset) {
                let height_offset_unscaled = main_window
                    .player_mut()
                    .get()
                    .unwrap()
                    .height_offset_unscaled();
                main_window.set_height_offset_unscaled(height_offset_unscaled);
            }
            let needs_redraw = main_window
                .player_mut()
                .get()
                .unwrap()
                .user_event(event_loop, event);
            match needs_redraw {
                flits_editor_lib::NeedsRedraw::Yes => main_window.request_redraw(),
                flits_editor_lib::NeedsRedraw::No => (),
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(main_window) = &mut self.main_window {
            main_window.about_to_wait(event_loop);
        }
    }
}
