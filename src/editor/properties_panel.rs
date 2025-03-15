use egui::Vec2;

use crate::core::{
    Animation, Bitmap, BitmapCacheStatus, BitmapProperties, EditorColor, Movie, MovieClip,
    MovieClipProperties, MovieProperties, PlaceSymbol, PlacedSymbolIndex, Symbol, SymbolIndex,
    SymbolIndexOrRoot,
};

use super::{
    edit::{
        BitmapPropertiesEdit, MovieClipPropertiesEdit, MovieEdit, MoviePropertiesEdit,
        RemoveSymbolEdit, TransformPlacedSymbolEdit,
    },
    EDIT_EPSILON,
};

pub enum PropertiesPanel {
    MovieProperties(MoviePropertiesPanel),
    SymbolProperties(SymbolPropertiesPanel),
    PlacedSymbolProperties(PlacedSymbolPropertiesPanel),
    MultiSelectionProperties(MultiSelectionPropertiesPanel),
}

pub struct MoviePropertiesPanel {
    pub before_edit: MovieProperties,
}
impl MoviePropertiesPanel {
    pub fn do_ui(&mut self, movie: &mut Movie, ui: &mut egui::Ui) -> Option<MovieEdit> {
        let mut edit: Option<MovieEdit> = None;

        ui.heading("Movie properties");
        egui::Grid::new("movie_properties_grid").show(ui, |ui| {
            let mut properties_edited = false;

            ui.label("Width:");
            let response = ui.add(egui::DragValue::new(&mut movie.properties.width));
            if response.lost_focus() || response.drag_stopped() {
                properties_edited = true;
            }

            ui.label("Framerate:");
            let response = ui.add(egui::DragValue::new(&mut movie.properties.frame_rate));
            if response.lost_focus() || response.drag_stopped() {
                properties_edited = true;
            }
            ui.end_row();

            ui.label("Height:");
            let response = ui.add(egui::DragValue::new(&mut movie.properties.height));
            if response.lost_focus() || response.drag_stopped() {
                properties_edited = true;
            }

            ui.label("Background color:");
            let mut color: egui::Rgba = egui::Rgba::from_rgba_premultiplied(
                movie.properties.background_color.r as f32 / 255.0,
                movie.properties.background_color.g as f32 / 255.0,
                movie.properties.background_color.b as f32 / 255.0,
                movie.properties.background_color.a as f32 / 255.0,
            );
            let response = egui::color_picker::color_edit_button_rgba(
                ui,
                &mut color,
                egui::color_picker::Alpha::OnlyBlend,
            );
            movie.properties.background_color = EditorColor {
                r: (color.r() * 255.0) as u8,
                g: (color.g() * 255.0) as u8,
                b: (color.b() * 255.0) as u8,
                a: (color.a() * 255.0) as u8,
            };
            // this is true even when you don't have the color picker selected
            // and you click anywhere in the program
            // but that is mitigated by the equality check below
            if response.clicked_elsewhere() {
                properties_edited = true;
            }
            ui.end_row();

            if properties_edited {
                // only add edit when the properties actually changed
                if self.before_edit != movie.properties {
                    edit = Some(MovieEdit::EditMovieProperties(MoviePropertiesEdit {
                        before: self.before_edit.clone(),
                        after: movie.properties.clone(),
                    }));
                }
            }
        });

        edit
    }
}

pub struct SymbolPropertiesPanel {
    pub symbol_index: SymbolIndex,
    pub before_edit: SymbolProperties,
}
pub enum SymbolProperties {
    Bitmap(BitmapProperties),
    MovieClip(MovieClipProperties),
}
impl SymbolPropertiesPanel {
    pub fn do_ui(&mut self, movie: &mut Movie, ui: &mut egui::Ui) -> Option<MovieEdit> {
        let mut edit1: Option<MovieEdit> = None;

        let symbol = &mut movie.symbols[self.symbol_index];
        ui.horizontal(|ui| {
            ui.heading(format!("{} properties", symbol.type_name()));
            ui.with_layout(
                egui::Layout::default().with_cross_align(egui::Align::RIGHT),
                |ui| {
                    if ui.button("Remove").clicked() {
                        edit1 = Some(MovieEdit::RemoveSymbol(RemoveSymbolEdit {
                            symbol_index: self.symbol_index,
                            symbol: symbol.clone_without_cache(),
                            remove_place_symbol_edits: vec![],
                        }));
                    }
                },
            );
        });
        let edit2 = match symbol {
            Symbol::Bitmap(bitmap) => self.bitmap_ui(bitmap, ui),
            Symbol::MovieClip(movieclip) => self.movieclip_ui(movieclip, ui),
        };
        if edit1.is_some() {
            edit1
        } else {
            edit2
        }
    }

    fn bitmap_ui(&self, bitmap: &mut Bitmap, ui: &mut egui::Ui) -> Option<MovieEdit> {
        let mut edit: Option<MovieEdit> = None;
        let mut edited = false;
        egui::Grid::new(format!("bitmap_{}_properties_grid", self.symbol_index)).show(ui, |ui| {
            ui.label("Name:");
            let response = ui.add(
                egui::TextEdit::singleline(&mut bitmap.properties.name)
                    .min_size(Vec2::new(200.0, 0.0)),
            );
            if response.lost_focus() {
                edited = true;
            }
            ui.end_row();

            ui.label("Path:");
            let mut path_text_edit = egui::TextEdit::singleline(&mut bitmap.properties.path)
                .min_size(Vec2::new(200.0, 0.0));
            if let BitmapCacheStatus::Invalid(_) = &bitmap.cache {
                path_text_edit = path_text_edit.text_color(ui.style().visuals.error_fg_color);
            }
            let response = ui.add(path_text_edit);
            if response.lost_focus() {
                edited = true;
            }
            ui.end_row();
        });

        let mut has_animation = bitmap.properties.animation.is_some();
        let response = ui.checkbox(&mut has_animation, "Animated");
        if response.changed() {
            edited = true;
            // change the property to match the new value of the checkbox
            if has_animation {
                bitmap.properties.animation = Some(Animation {
                    frame_count: 2,
                    frame_delay: 0,
                    end_action: "".into(),
                });
            } else {
                bitmap.properties.animation = None;
            }
        }
        if let Some(animation) = &mut bitmap.properties.animation {
            ui.horizontal(|ui| {
                ui.label("Frames:");
                let response = ui.add_sized(
                    Vec2::new(60.0, 20.0),
                    // TODO: if the value is too big the editor crashes because the frame is less than 1px
                    egui::DragValue::new(&mut animation.frame_count)
                        .speed(0.05)
                        .range(1..=egui::emath::Numeric::MAX),
                );
                if response.lost_focus() || response.drag_stopped() {
                    edited = true;
                }
                ui.label("Frames delay after each frame:");
                let response = ui.add_sized(
                    Vec2::new(60.0, 20.0),
                    egui::DragValue::new(&mut animation.frame_delay).speed(0.05),
                );
                if response.lost_focus() || response.drag_stopped() {
                    edited = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("On last frame call (e.g. 'stop' or 'removeMovieClip'): ");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut animation.end_action)
                        .min_size(Vec2::new(100.0, 0.0)),
                );
                if response.lost_focus() {
                    edited = true;
                }
            });
        }

        if let BitmapCacheStatus::Invalid(error) = &bitmap.cache {
            ui.colored_label(ui.style().visuals.error_fg_color, "Error:");
            ui.colored_label(ui.style().visuals.error_fg_color, error);
        } else {
            // add an empty row so the amount of rows is always the same
            // otherwise the height of the panel will only be updated on the next redraw
            ui.label("");
        }
        ui.end_row();

        let SymbolProperties::Bitmap(before_edit) = &self.before_edit else {
            panic!("before_edit is not a bitmap");
        };
        if edited && before_edit != &bitmap.properties {
            edit = Some(MovieEdit::EditBitmapProperties(BitmapPropertiesEdit {
                editing_symbol_index: self.symbol_index,
                before: before_edit.clone(),
                after: bitmap.properties.clone(),
            }));
        }

        edit
    }

    fn movieclip_ui(&self, movieclip: &mut MovieClip, ui: &mut egui::Ui) -> Option<MovieEdit> {
        let mut edit: Option<MovieEdit> = None;

        egui::Grid::new(format!("movieclip_{}_properties_grid", self.symbol_index)).show(
            ui,
            |ui| {
                let mut edited = false;

                ui.label("Name:");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut movieclip.properties.name)
                        .min_size(Vec2::new(200.0, 0.0)),
                );
                if response.lost_focus() {
                    edited = true;
                }
                ui.end_row();

                ui.label("Class:");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut movieclip.properties.class_name)
                        .min_size(Vec2::new(200.0, 0.0)),
                );
                if response.lost_focus() {
                    edited = true;
                }
                ui.end_row();

                let SymbolProperties::MovieClip(before_edit) = &self.before_edit else {
                    panic!("before_edit is not a movieclip");
                };
                if edited && before_edit != &movieclip.properties {
                    edit = Some(MovieEdit::EditMovieClipProperties(
                        MovieClipPropertiesEdit {
                            editing_symbol_index: self.symbol_index,
                            before: before_edit.clone(),
                            after: movieclip.properties.clone(),
                        },
                    ));
                }
            },
        );

        edit
    }
}

pub struct PlacedSymbolPropertiesPanel {
    pub before_edit: PlaceSymbol,
}
impl PlacedSymbolPropertiesPanel {
    pub fn do_ui(
        &mut self,
        movie: &mut Movie,
        ui: &mut egui::Ui,
        editing_clip: SymbolIndexOrRoot,
        placed_symbol_index: PlacedSymbolIndex,
    ) -> Option<MovieEdit> {
        ui.heading("Placed symbol properties");
        let placed_symbol = movie
            .get_placed_symbols_mut(editing_clip)
            .get_mut(placed_symbol_index)
            .unwrap();

        let mut edit: Option<MovieEdit> = None;
        egui::Grid::new(format!(
            "placed_symbol_{placed_symbol_index}_properties_grid"
        ))
        .show(ui, |ui| {
            let mut dvc = DragValueContext {
                ui,
                position_edited: false,
            };

            Self::drag_value(&mut dvc, "x", &mut placed_symbol.transform.x);
            Self::drag_value(&mut dvc, "X scale", &mut placed_symbol.transform.x_scale);
            dvc.ui.end_row();

            Self::drag_value(&mut dvc, "y", &mut placed_symbol.transform.y);
            Self::drag_value(&mut dvc, "Y scale", &mut placed_symbol.transform.y_scale);
            dvc.ui.end_row();

            if dvc.position_edited {
                let placed_symbol_before_edit = &self.before_edit;
                // only add edit when the position actually changed
                if f64::abs(placed_symbol_before_edit.transform.x - placed_symbol.transform.x)
                    > EDIT_EPSILON
                    || f64::abs(placed_symbol_before_edit.transform.y - placed_symbol.transform.y)
                        > EDIT_EPSILON
                    || f64::abs(
                        placed_symbol_before_edit.transform.x_scale
                            - placed_symbol.transform.x_scale,
                    ) > EDIT_EPSILON
                    || f64::abs(
                        placed_symbol_before_edit.transform.y_scale
                            - placed_symbol.transform.y_scale,
                    ) > EDIT_EPSILON
                {
                    edit = Some(MovieEdit::TransformPlacedSymbol(
                        TransformPlacedSymbolEdit {
                            editing_symbol_index: editing_clip,
                            placed_symbol_index,
                            start: placed_symbol_before_edit.transform.clone(),
                            end: placed_symbol.transform.clone(),
                        },
                    ));
                }
            }
        });

        edit
    }

    fn drag_value(dvc: &mut DragValueContext, label: &str, value: &mut f64) {
        dvc.ui.label(label);
        let response = dvc
            .ui
            .add_sized(Vec2::new(60.0, 20.0), egui::DragValue::new(value));
        if response.lost_focus() || response.drag_stopped() {
            dvc.position_edited = true;
        }
    }
}
struct DragValueContext<'a> {
    ui: &'a mut egui::Ui,
    position_edited: bool,
}

pub struct MultiSelectionPropertiesPanel {}
impl MultiSelectionPropertiesPanel {
    pub fn do_ui(&mut self, ui: &mut egui::Ui) -> Option<MovieEdit> {
        ui.label("Multiple items selected");
        None
    }
}
