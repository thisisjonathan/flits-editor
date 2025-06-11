use ruffle_render::backend::{RenderBackend, ViewportDimensions};
use windowing::Player;
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};

use crate::{custom_event::FlitsEvent, welcome::WelcomeScreen};

enum FlitsState {
    Welcome(WelcomeScreen),
    Editor,
}

pub struct FlitsPlayer {
    renderer: Box<dyn RenderBackend>,
    event_loop: EventLoopProxy<FlitsEvent>,
    state: FlitsState,
    is_about_visible: bool,
}
impl FlitsPlayer {
    pub fn new(renderer: Box<dyn RenderBackend>, event_loop: EventLoopProxy<FlitsEvent>) -> Self {
        FlitsPlayer {
            renderer,
            event_loop,
            state: FlitsState::Welcome(WelcomeScreen::new()),
            is_about_visible: false,
        }
    }
    pub fn do_ui(&mut self, egui_ctx: &egui::Context) {
        match &mut self.state {
            FlitsState::Welcome(welcome_screen) => {
                welcome_screen.do_ui(egui_ctx, self.event_loop.clone())
            }
            FlitsState::Editor => todo!(),
        }

        if self.is_about_visible {
            self.about_window(egui_ctx);
        }
    }

    fn about_window(&mut self, egui_ctx: &egui::Context) {
        egui::Window::new("About Flits Editor")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .open(&mut self.is_about_visible)
            .show(egui_ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("Flits Editor").size(32.0));
                    ui.label("Preview build");
                })
            });
    }

    pub fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: FlitsEvent) {
        match event {
            FlitsEvent::NewFile(_new_project_data) => todo!(),
            FlitsEvent::OpenFile => todo!(),
            FlitsEvent::CloseFile => self.state = FlitsState::Welcome(WelcomeScreen::new()),
            FlitsEvent::About => self.is_about_visible = true,
            FlitsEvent::CommandOutput(_) => todo!(),
            FlitsEvent::RuffleClosed => todo!(),
        }
    }
}
impl Player for FlitsPlayer {
    fn render(&mut self) {}

    fn renderer_mut(&mut self) -> &mut dyn RenderBackend {
        &mut *self.renderer
    }

    fn set_viewport_dimensions(&mut self, viewport_dimensions: ViewportDimensions) {
        self.renderer.set_viewport_dimensions(viewport_dimensions);
    }

    fn tick(&mut self, _dt: f64) {}

    fn time_til_next_frame(&self) -> Option<std::time::Duration> {
        None
    }
}
