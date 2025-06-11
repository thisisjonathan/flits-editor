use winit::event_loop::EventLoopProxy;

use crate::{custom_event::FlitsEvent, welcome::WelcomeScreen};

pub enum FlitsState {
    Welcome(WelcomeScreen),
    Editor,
}
impl FlitsState {
    pub fn do_ui(&mut self, egui_ctx: &egui::Context, event_loop: EventLoopProxy<FlitsEvent>) {
        match self {
            FlitsState::Welcome(welcome_screen) => welcome_screen.do_ui(egui_ctx, event_loop),
            FlitsState::Editor => todo!(),
        }
    }
    pub fn render(&mut self) {}
}
