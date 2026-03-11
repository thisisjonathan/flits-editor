#[derive(Default)]
pub struct ErrorWindow {
    error: String,
}
impl ErrorWindow {
    pub fn new(error: String) -> Option<Self> {
        Some(Self { error })
    }
}
pub trait ErrorWindowTrait {
    fn do_ui(&mut self, egui_ctx: &egui::Context);
}
impl ErrorWindowTrait for Option<ErrorWindow> {
    fn do_ui(&mut self, egui_ctx: &egui::Context) {
        let Some(me) = self else {
            return;
        };
        let mut is_window_open = true;
        egui::Modal::new(egui::Id::new("Error"))
            // make it really obvious a modal is open
            .backdrop_color(egui::Color32::from_black_alpha(230))
            .show(egui_ctx, |ui| {
                ui.heading(egui::RichText::new("Error").color(ui.style().visuals.error_fg_color));
                ui.label(egui::RichText::new(&me.error).color(ui.style().visuals.error_fg_color));
                ui.end_row();

                let response = ui.add(egui::Button::new("Ok"));
                let pressed_enter = response.lost_focus()
                    && response.ctx.input(|i| i.key_pressed(egui::Key::Enter));
                let pressed_escape = response.lost_focus()
                    && response.ctx.input(|i| i.key_pressed(egui::Key::Escape));
                if response.clicked() || pressed_enter || pressed_escape {
                    is_window_open = false;
                }
                response.request_focus();
                ui.end_row();
            });
        if !is_window_open {
            *self = None;
        }
    }
}
