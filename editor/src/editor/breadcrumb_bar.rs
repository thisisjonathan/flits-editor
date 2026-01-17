use flits_core::Movie;

use crate::{editor::Selection, message::EditorMessage, message_bus::MessageBus};

#[derive(Default)]
pub struct BreadcrumbBar {}
impl BreadcrumbBar {
    pub fn do_ui(
        &mut self,
        ui: &mut egui::Ui,
        movie: &Movie,
        selection: &Selection,
        message_bus: &MessageBus<EditorMessage>,
    ) {
        ui.horizontal(|ui| {
            if let Some(editing_clip) = selection.stage_symbol_index {
                if ui.selectable_label(false, "Scene").clicked() {
                    message_bus.publish(EditorMessage::ChangeSelectedSymbol(None));
                }
                let _ = ui.selectable_label(true, movie.symbols[editing_clip].name());
            } else {
                let _ = ui.selectable_label(true, "Scene");
            }
        });
    }
}
