use std::path::PathBuf;

use flits_core::Movie;
use flits_editor_lib::{Editor, FlitsEvent, NeedsRedraw, MENU_HEIGHT};
use rfd::{FileDialog, MessageDialogResult};
use ruffle_render::backend::{RenderBackend, ViewportDimensions};
use windowing::Player;
use winit::{
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoopProxy},
};

use crate::{cli::CliParams, welcome::WelcomeScreen};

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
    pub fn new(
        renderer: Box<dyn RenderBackend>,
        event_loop: EventLoopProxy<FlitsEvent>,
        cli_params: CliParams,
    ) -> Self {
        let mut player = FlitsPlayer {
            renderer,
            event_loop: event_loop,
            state: FlitsState::Welcome(WelcomeScreen::new()),
            is_about_visible: false,
        };
        match cli_params {
            CliParams::NoProject => {
                // call set_state  to force title update
                player.set_state(FlitsState::Welcome(WelcomeScreen::new()));
            }
            CliParams::OpenProject(path) => player.open_editor(path),
        };
        player
    }
    pub fn do_ui(&mut self, egui_ctx: &egui::Context) -> NeedsRedraw {
        let mut needs_redraw = NeedsRedraw::No;
        match &mut self.state {
            FlitsState::Welcome(welcome_screen) => {
                welcome_screen.do_ui(egui_ctx, self.event_loop.clone())
            }
            FlitsState::Editor(editor) => {
                needs_redraw = editor.do_ui(egui_ctx, &self.event_loop);
            }
        }

        if self.is_about_visible {
            self.about_window(egui_ctx);
        }

        needs_redraw
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
            FlitsEvent::NewFile(new_project_data) => {
                if !new_project_data.path.is_dir() {
                    rfd::MessageDialog::new()
                        .set_description("Invalid path.")
                        .show();
                    return NeedsRedraw::Yes;
                }
                if !new_project_data.path.read_dir().unwrap().next().is_none() {
                    if rfd::MessageDialog::new()
                                    .set_buttons(rfd::MessageButtons::YesNo)
                                    .set_description("The directory is not empty, are you sure you want to create a project in this directory?")
                                    .show() != MessageDialogResult::Yes {
                                    return NeedsRedraw::Yes;
                                }
                }
                let json_path = new_project_data.path.join("movie.json");
                let movie = Movie::from_properties(new_project_data.movie_properties);
                movie.save(&json_path);
                self.open_editor(json_path);
                NeedsRedraw::Yes
            }
            FlitsEvent::OpenFile => {
                if let Some(path) = FileDialog::new()
                    .add_filter("Project Files", &["json"])
                    .add_filter("All Files", &["*"])
                    .set_title("Load a project")
                    .pick_file()
                {
                    self.open_editor(path);
                }
                NeedsRedraw::Yes
            }
            FlitsEvent::CloseFile => {
                self.set_state(FlitsState::Welcome(WelcomeScreen::new()));
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
            _ => NeedsRedraw::No,
        }
    }

    fn open_editor(&mut self, path: PathBuf) {
        let result = Editor::new(
            path,
            self.renderer.viewport_dimensions(),
            self.event_loop.clone(),
        );
        match result {
            Ok(editor) => self.set_state(FlitsState::Editor(editor)),
            Err(error) => {
                rfd::MessageDialog::new()
                    .set_level(rfd::MessageLevel::Error)
                    .set_title("Flits Editor")
                    .set_description(&format!("Unable to load file:\n{error}\n"))
                    .show();
            }
        }
    }

    fn set_state(&mut self, state: FlitsState) {
        self.state = state;
        self.event_loop
            .send_event(FlitsEvent::UpdateTitle)
            .unwrap_or_else(|err| {
                eprintln!("Unable to send command output event: {}", err);
            });
        self.event_loop
            .send_event(FlitsEvent::UpdateHeightOffset)
            .unwrap_or_else(|err| {
                eprintln!("Unable to send command output event: {}", err);
            });
    }

    pub fn title(&self) -> String {
        match &self.state {
            FlitsState::Welcome(_) => "Flits Editor".into(),
            FlitsState::Editor(editor) => {
                format!(
                    "{}{} - Flits Editor",
                    editor.project_name(),
                    if editor.unsaved_changes() { "*" } else { "" },
                )
            }
        }
    }

    pub fn height_offset_unscaled(&self) -> u32 {
        match self.state {
            FlitsState::Welcome(_) => 0,
            FlitsState::Editor(_) => MENU_HEIGHT,
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
