use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::anyhow;
use anyhow::Error;
use ruffle_render::backend::RenderBackend;
use ruffle_render_wgpu::{backend::WgpuRenderBackend, descriptors::Descriptors};
use wgpu::{Backends, PowerPreference};
use windowing::{
    Config, GuiController, MovieView, Player, PlayerController, RuffleGui, RuffleWindow,
};
use winit::{
    application::ApplicationHandler,
    event::StartCause,
    event_loop::{ActiveEventLoop, EventLoop},
    window::WindowAttributes,
};

struct MyGui {}
impl RuffleGui for MyGui {
    type Player = MyPlayer;
    type Arguments = ();

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

    fn on_player_destroyed(&self) {}

    fn height_offset_unscaled(&self) -> u32 {
        48
    }

    fn cursor_icon(&self) -> Option<egui::CursorIcon> {
        None
    }
}
struct MyPlayer {
    renderer: Box<dyn RenderBackend>,
}
impl Player for MyPlayer {
    fn renderer_mut(&mut self) -> &mut dyn RenderBackend {
        &mut *self.renderer
    }
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

struct App {
    main_window: Option<RuffleWindow<MyGui, MyPlayerController>>,
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
                Config {
                    preferred_backends: Backends::all(),
                    power_preference: PowerPreference::None,
                    trace_path: None,
                    // without this, dragged things lag behind the cursor
                    present_mode: wgpu::PresentMode::AutoNoVsync,
                    // changing this from the default 2 doesn't seem to have an effect but change it anyway to be sure
                    desired_maximum_frame_latency: 1,
                },
                MyGui {},
                |_window, _egui_ctx| {},
                false,
            )
            .unwrap();
            let descriptors = gui.descriptors().clone();
            self.main_window = Some(RuffleWindow::new(
                gui,
                MyPlayerController {
                    player: None,
                    descriptors,
                },
            ));
            let nothing = ();
            self.main_window.as_mut().unwrap().create_movie(&nothing);
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
            main_window.about_to_wait(event_loop);
        }
    }
}

fn main() -> Result<(), Error> {
    let event_loop: EventLoop<()> = EventLoop::with_user_event().build()?;
    let mut app = App { main_window: None };
    event_loop.run_app(&mut app)?;
    Ok(())
}
