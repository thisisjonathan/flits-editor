use flits_core::Movie;

use crate::{editor::Selection, message::EditorMessage, message_bus::MessageBus};

#[derive(Default)]
pub struct Library {}
impl Library {
    pub fn do_ui(
        &mut self,
        ui: &mut egui::Ui,
        movie: &Movie,
        selection: &Selection,
        message_bus: &MessageBus<EditorMessage>,
    ) {
        ui.heading("Library");
        if ui.button("Add MovieClip...").clicked() {
            //self.new_symbol_window = Some(NewSymbolWindow::default());
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for i in 0..movie.symbols.len() {
                    let symbol = movie.symbols.get(i).unwrap();
                    let checked = selection
                        .symbol_index
                        .map_or(false, |symbol_index| symbol_index == i);
                    let mut text = egui::RichText::new(symbol.name());
                    if symbol.is_invalid() {
                        text = text.color(ui.style().visuals.error_fg_color);
                    }
                    let response = ui.selectable_label(checked, text);
                    let response = response.interact(egui::Sense::drag());

                    if response.clicked() {
                        message_bus.publish(EditorMessage::ChangeSelectedSymbol(Some(i)));
                        /*match movie.symbols[i] {
                            Symbol::MovieClip(_) => {
                                self.change_editing_clip(Some(i));
                            }
                            _ => {
                                self.properties_panel =
                                    Self::create_symbol_propeties_panel(i, symbol);
                            }
                        }

                        needs_redraw = NeedsRedraw::Yes;*/
                    } else if response.drag_stopped() {
                        /*// TODO: handle drag that doesn't end on stage
                        let mouse_pos = response.interact_pointer_pos().unwrap();
                        let mut matrix = self.camera.screen_to_world_matrix(self.stage_size())
                            * Matrix::translate(
                                Twips::from_pixels(mouse_pos.x as f64),
                                Twips::from_pixels(
                                    // TODO: don't hardcode the menu height
                                    mouse_pos.y as f64 - MENU_HEIGHT as f64,
                                ),
                            );
                        // reset zoom (otherwise when you are zoomed in the symbol becomes smaller)
                        matrix.a = Matrix::IDENTITY.a;
                        matrix.b = Matrix::IDENTITY.b;
                        matrix.c = Matrix::IDENTITY.c;
                        matrix.d = Matrix::IDENTITY.d;
                        self.do_edit(MovieEdit::AddPlacedSymbol(AddPlacedSymbolEdit {
                            editing_symbol_index: self.editing_clip,
                            placed_symbol: PlaceSymbol {
                                symbol_index: i,
                                transform: EditorTransform {
                                    x: matrix.tx.to_pixels(),
                                    y: matrix.ty.to_pixels(),
                                    x_scale: 1.0,
                                    y_scale: 1.0,
                                },
                                instance_name: "".into(),
                                text: match &movie.symbols[i] {
                                    Symbol::Font(_) => Some(Box::new(TextProperties::new())),
                                    _ => None,
                                },
                            },
                            placed_symbol_index: None,
                        }));
                        needs_redraw = NeedsRedraw::Yes;*/
                    }
                }
            });
    }
}
