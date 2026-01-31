use crate::{editor::Context, message::EditorMessage};

#[derive(Default)]
pub struct BreadcrumbBar {}
impl BreadcrumbBar {
    pub fn do_ui(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        ui.horizontal(|ui| {
            if let Some(editing_clip) = ctx.selection.stage_symbol_index {
                if ui.selectable_label(false, "Scene").clicked() {
                    ctx.message_bus
                        .publish(EditorMessage::ChangeSelectedSymbol(None));
                }
                let _ = ui.selectable_label(true, ctx.movie.symbols[editing_clip].name());
            } else {
                let _ = ui.selectable_label(true, "Scene");
            }
        });
    }
}
