use egui::Vec2;
use swf::Twips;

use crate::core::{
    Bitmap, Movie, MovieClip, MovieProperties, PlaceSymbol, PlacedSymbolIndex, Symbol, SymbolIndex,
    SymbolIndexOrRoot, MovieClipProperties, BitmapProperties, BitmapCacheStatus,
};

use super::{
    edit::{MovePlacedSymbolEdit, MovieEdit, MoviePropertiesEdit, MovieClipPropertiesEdit, BitmapPropertiesEdit},
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
            if response.lost_focus() || response.drag_released() {
                properties_edited = true;
            }
            ui.end_row();

            ui.label("Height:");
            let response = ui.add(egui::DragValue::new(&mut movie.properties.height));
            if response.lost_focus() || response.drag_released() {
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
        let symbol = &mut movie.symbols[self.symbol_index];
        match symbol {
            Symbol::Bitmap(bitmap) => self.bitmap_ui(bitmap, ui),
            Symbol::MovieClip(movieclip) => self.movieclip_ui(movieclip, ui),
        }
    }

    fn bitmap_ui(&self, bitmap: &mut Bitmap, ui: &mut egui::Ui) -> Option<MovieEdit> {
        ui.heading("Bitmap properties");

        let mut edit: Option<MovieEdit> = None;
        egui::Grid::new(format!("bitmap_{}_properties_grid", self.symbol_index)).show(
            ui,
            |ui| {
                let mut edited = false;
                
                ui.label("Name:");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut bitmap.properties.name).min_size(Vec2::new(200.0, 0.0)),
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
            },
        );
        
        edit
    }

    fn movieclip_ui(&self, movieclip: &mut MovieClip, ui: &mut egui::Ui) -> Option<MovieEdit> {
        ui.heading("Movieclip properties");
        
        let mut edit: Option<MovieEdit> = None;
        egui::Grid::new(format!("movieclip_{}_properties_grid", self.symbol_index)).show(
            ui,
            |ui| {
                let mut edited = false;
                
                ui.label("Name:");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut movieclip.properties.name).min_size(Vec2::new(200.0, 0.0)),
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
                    edit = Some(MovieEdit::EditMovieClipProperties(MovieClipPropertiesEdit {
                        editing_symbol_index: self.symbol_index,
                        before: before_edit.clone(),
                        after: movieclip.properties.clone(),
                    }));
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
            let mut position_edited = false;
            ui.label("x");
            let mut value = placed_symbol.transform.matrix.tx.to_pixels();
            let response = ui.add(egui::DragValue::new(&mut value));
            placed_symbol.transform.matrix.tx = Twips::from_pixels(value);
            if response.lost_focus() || response.drag_released() {
                position_edited = true;
            }
            ui.end_row();

            ui.label("y");
            let mut value = placed_symbol.transform.matrix.ty.to_pixels();
            let response = ui.add(egui::DragValue::new(&mut value));
            placed_symbol.transform.matrix.ty = Twips::from_pixels(value);
            if response.lost_focus() || response.drag_released() {
                position_edited = true;
            }
            ui.end_row();

            if position_edited {
                let placed_symbol_before_edit = &self.before_edit;
                // only add edit when the position actually changed
                if f64::abs(placed_symbol_before_edit.transform.matrix.tx.to_pixels() - placed_symbol.transform.matrix.ty.to_pixels()) > EDIT_EPSILON
                    || f64::abs(placed_symbol_before_edit.transform.matrix.ty.to_pixels() - placed_symbol.transform.matrix.ty.to_pixels()) > EDIT_EPSILON
                {
                    edit = Some(MovieEdit::MovePlacedSymbol(MovePlacedSymbolEdit {
                        editing_symbol_index: editing_clip,
                        placed_symbol_index,
                        start: placed_symbol_before_edit.transform.matrix,
                        end: placed_symbol.transform.matrix,
                    }));
                }
            }
        });

        edit
    }
}

pub struct MultiSelectionPropertiesPanel {}
impl MultiSelectionPropertiesPanel {
    pub fn do_ui(&mut self, ui: &mut egui::Ui) -> Option<MovieEdit> {
        ui.label("Multiple items selected");
        None
    }
}
