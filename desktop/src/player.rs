use flits_editor_lib::{Editor, FlitsEvent, NeedsRedraw};
use rfd::FileDialog;
use ruffle_render::backend::{RenderBackend, ViewportDimensions};
use windowing::Player;
use winit::{
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoopProxy},
};

use crate::welcome::WelcomeScreen;

enum FlitsState {
    Welcome(WelcomeScreen),
    Editor(Editor),
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
            FlitsState::Editor(editor) => {
                editor.do_ui(egui_ctx, &self.event_loop);
            }
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

    pub fn window_event(&mut self, _event_loop: &ActiveEventLoop, _event: WindowEvent) {}

    pub fn user_event(&mut self, event_loop: &ActiveEventLoop, event: FlitsEvent) -> NeedsRedraw {
        match event {
            FlitsEvent::NewFile(_new_project_data) => todo!(),
            FlitsEvent::OpenFile => {
                if let Some(path) = FileDialog::new()
                    .add_filter("Project Files", &["json"])
                    .add_filter("All Files", &["*"])
                    .set_title("Load a project")
                    .pick_file()
                {
                    self.state =
                        FlitsState::Editor(Editor::new(path, self.renderer.viewport_dimensions()));
                }
                NeedsRedraw::Yes
            }
            FlitsEvent::CloseFile => {
                self.state = FlitsState::Welcome(WelcomeScreen::new());
                NeedsRedraw::Yes
            }
            FlitsEvent::ExitRequested => {
                // TODO: the old code calls shutdown()
                event_loop.exit();
                NeedsRedraw::No
            }
            FlitsEvent::About => {
                self.is_about_visible = true;
                NeedsRedraw::Yes
            }

            FlitsEvent::CommandOutput(line) => {
                if let FlitsState::Editor(editor) = &mut self.state {
                    editor.receive_command_output(line)
                } else {
                    NeedsRedraw::No
                }
            }
            FlitsEvent::RuffleClosed => {
                if let FlitsState::Editor(editor) = &mut self.state {
                    editor.on_ruffle_closed();
                    NeedsRedraw::Yes
                } else {
                    NeedsRedraw::No
                }
            }
        }
    }
}
impl Player for FlitsPlayer {
    fn render(&mut self) {
        match &mut self.state {
            FlitsState::Welcome(welcome_screen) => welcome_screen.render(&mut self.renderer),
            FlitsState::Editor(editor) => editor.render(&mut self.renderer),
        }
    }

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

    fn handle_mouse_move(&mut self, mouse_x: f64, mouse_y: f64) {
        let FlitsState::Editor(editor) = &mut self.state else {
            return;
        };
        editor.handle_mouse_move(mouse_x, mouse_y);
    }

    fn handle_mouse_input(
        &mut self,
        mouse_x: f64,
        mouse_y: f64,
        button: winit::event::MouseButton,
        state: winit::event::ElementState,
    ) {
        let FlitsState::Editor(editor) = &mut self.state else {
            return;
        };
        editor.handle_mouse_input(mouse_x, mouse_y, button, state);
    }
}
