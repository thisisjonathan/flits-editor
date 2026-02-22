use crate::{
    editor::{stage::StageMessage, Context},
    message::EditorMessage,
};

#[derive(Default)]
pub struct Library {}
impl Library {
    pub fn do_ui(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        ui.heading("Library");
        if ui.button("Add MovieClip...").clicked() {
            //self.new_symbol_window = Some(NewSymbolWindow::default());
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for i in 0..ctx.movie.symbols.len() {
                    let symbol = ctx.movie.symbols.get(i).unwrap();
                    let checked = ctx
                        .selection
                        .properties_symbol_index
                        .map_or(false, |symbol_index| symbol_index == i);
                    let mut text = egui::RichText::new(symbol.name());
                    if symbol.is_invalid() {
                        text = text.color(ui.style().visuals.error_fg_color);
                    }
                    let response = ui.selectable_label(checked, text);
                    let response = response.interact(egui::Sense::drag());

                    if response.clicked() {
                        ctx.message_bus
                            .publish(EditorMessage::ChangeSelectedSymbol(Some(i)));
                        /*needs_redraw = NeedsRedraw::Yes;*/
                    } else if response.drag_stopped() {
                        // TODO: handle drag that doesn't end on stage
                        ctx.message_bus.publish(EditorMessage::Stage(
                            StageMessage::ReleaseSymbolDragDrop(
                                response.interact_pointer_pos().unwrap(),
                                i,
                            ),
                        ));
                        //needs_redraw = NeedsRedraw::Yes;
                    }
                }
            });
    }
}
