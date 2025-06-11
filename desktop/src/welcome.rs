use winit::event_loop::EventLoopProxy;

use crate::custom_event::{FlitsEvent, NewProjectData};

pub struct WelcomeScreen {
    new_project: Option<NewProjectData>,
}
impl WelcomeScreen {
    pub fn new() -> Self {
        WelcomeScreen { new_project: None }
    }
    pub fn do_ui(&mut self, egui_ctx: &egui::Context, event_loop: EventLoopProxy<FlitsEvent>) {
        egui::Window::new("Welcome")
            .collapsible(false)
            .title_bar(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(egui_ctx, |ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(200.0, 100.0),
                    egui::Layout::top_down_justified(egui::Align::Center),
                    |ui| {
                        ui.style_mut().override_text_style = Some(egui::TextStyle::Heading);
                        if ui.button("New project...").clicked() {
                            self.open_new_project_window();
                        }
                        if ui.button("Open project...").clicked() {
                            let _ = event_loop.send_event(FlitsEvent::OpenFile);
                        }
                        if ui.button("About...").clicked() {
                            let _ = event_loop.send_event(FlitsEvent::About);
                        }
                    },
                );
            });

        self.new_project_window(egui_ctx, event_loop);
    }

    fn open_new_project_window(&mut self) {
        self.new_project = Some(NewProjectData::default());
    }

    fn new_project_window(
        &mut self,
        egui_ctx: &egui::Context,
        event_loop: EventLoopProxy<FlitsEvent>,
    ) {
        if self.new_project.is_some() {
            let event_loop = &event_loop;

            let mut is_window_open = true;
            egui::Window::new("New project")
                .collapsible(false)
                .resizable(false)
                .open(&mut is_window_open)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(egui_ctx, |ui| {
                    egui::Grid::new("movie_properties_grid").show(ui, |ui| {
                        let new_project = self.new_project.as_mut().unwrap();
                        ui.label("Directory:");
                        ui.label(new_project.path.to_str().unwrap());
                        if ui.button("Change...").clicked() {
                            if let Some(directory) = rfd::FileDialog::new().pick_folder() {
                                new_project.path = directory;
                            }
                        }
                        ui.end_row();

                        ui.label("Width:");
                        ui.add(egui::DragValue::new(
                            &mut new_project.movie_properties.width,
                        ));
                        ui.end_row();

                        ui.label("Height:");
                        ui.add(egui::DragValue::new(
                            &mut new_project.movie_properties.height,
                        ));
                        ui.end_row();

                        ui.label("Framerate:");
                        ui.add(egui::DragValue::new(
                            &mut new_project.movie_properties.frame_rate,
                        ));
                        ui.end_row();

                        if ui
                            .add_enabled(
                                !new_project.path.to_str().unwrap().is_empty(),
                                egui::Button::new("Create"),
                            )
                            .clicked()
                        {
                            let _ = event_loop.send_event(FlitsEvent::NewFile(new_project.clone()));
                            self.new_project = None;
                        }
                        ui.end_row();
                    });
                });
            if !is_window_open {
                self.new_project = None;
            }
        }
    }
}
