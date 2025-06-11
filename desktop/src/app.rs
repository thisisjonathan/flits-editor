use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::anyhow;
use ruffle_render_wgpu::{backend::WgpuRenderBackend, descriptors::Descriptors};
use wgpu::{Backends, PowerPreference};
use windowing::{Config, GuiController, MovieView, PlayerController, RuffleGui, RuffleWindow};
use winit::{
    application::ApplicationHandler,
    event::StartCause,
    event_loop::{ActiveEventLoop, EventLoopProxy},
    window::WindowAttributes,
};

use crate::{custom_event::FlitsEvent, player::FlitsPlayer};

struct FlitsArguments {
    event_loop: EventLoopProxy<FlitsEvent>,
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
    ) -> () {
        if let Some(player) = player {
            player.do_ui(egui_ctx);
        }
    }

    fn on_player_destroyed(&self) {}

    fn height_offset_unscaled(&self) -> u32 {
        48
    }

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
        let renderer = WgpuRenderBackend::new(self.descriptors.clone(), movie_view)
            .map_err(|e| anyhow!(e.to_string()))
            .expect("Couldn't create wgpu rendering backend");
        self.player = Some(Mutex::new(FlitsPlayer::new(
            Box::new(renderer),
            arguments.event_loop.clone(),
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
}
impl App {
    pub fn new(event_loop: EventLoopProxy<FlitsEvent>) -> Self {
        App {
            main_window: None,
            event_loop,
        }
    }
}
impl ApplicationHandler<FlitsEvent> for App {
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
                Config {
                    preferred_backends: Backends::all(),
                    power_preference: PowerPreference::None,
                    trace_path: None,
                    // without this, dragged things lag behind the cursor
                    present_mode: wgpu::PresentMode::AutoNoVsync,
                    // changing this from the default 2 doesn't seem to have an effect but change it anyway to be sure
                    desired_maximum_frame_latency: 1,
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
            main_window.window_event(event_loop, event);
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: FlitsEvent) {
        if let Some(main_window) = &mut self.main_window {
            main_window
                .player_mut()
                .get()
                .unwrap()
                .user_event(event_loop, event);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(main_window) = &mut self.main_window {
            main_window.about_to_wait(event_loop);
        }
    }
}
