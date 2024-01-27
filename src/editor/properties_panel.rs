use egui::Vec2;

use crate::core::{
    Movie, MovieProperties, PlaceSymbol, PlacedSymbolIndex, Symbol, SymbolIndex, SymbolIndexOrRoot,
};

use super::{
    edit::{MovePlacedSymbolEdit, MovieEdit, MoviePropertiesEdit},
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
                if self.before_edit != movie.properties
                {
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
}
impl SymbolPropertiesPanel {
    pub fn do_ui(
        &mut self,
        movie: &mut Movie,
        ui: &mut egui::Ui,
    ) -> Option<MovieEdit> {
        let symbol = &mut movie.symbols[self.symbol_index];
        match symbol {
            Symbol::Bitmap(_) => {
                ui.heading("Bitmap properties");
            }
            Symbol::MovieClip(movieclip) => {
                ui.heading("Movieclip properties");
                egui::Grid::new(format!("movieclip_{}_properties_grid", self.symbol_index)).show(
                    ui,
                    |ui| {
                        ui.label("Name:");
                        ui.add(
                            egui::TextEdit::singleline(&mut movieclip.name)
                                .min_size(Vec2::new(200.0, 0.0)),
                        );
                        ui.end_row();

                        ui.label("Class:");
                        ui.add(
                            egui::TextEdit::singleline(&mut movieclip.class_name)
                                .min_size(Vec2::new(200.0, 0.0)),
                        );
                        ui.end_row();
                    },
                );
            }
        }

        // TODO: actual undo/redo
        None
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
            let response = ui.add(egui::DragValue::new(&mut placed_symbol.x));
            if response.lost_focus() || response.drag_released() {
                position_edited = true;
            }
            ui.end_row();

            ui.label("y");
            let response = ui.add(egui::DragValue::new(&mut placed_symbol.y));
            if response.lost_focus() || response.drag_released() {
                position_edited = true;
            }
            ui.end_row();

            if position_edited {
                let placed_symbol_before_edit = &self.before_edit;
                // only add edit when the position actually changed
                if f64::abs(placed_symbol_before_edit.x - placed_symbol.x) > EDIT_EPSILON
                    || f64::abs(placed_symbol_before_edit.y - placed_symbol.y) > EDIT_EPSILON
                {
                    edit = Some(MovieEdit::MovePlacedSymbol(MovePlacedSymbolEdit {
                        editing_symbol_index: editing_clip,
                        placed_symbol_index,
                        start_x: placed_symbol_before_edit.x,
                        start_y: placed_symbol_before_edit.y,
                        end_x: placed_symbol.x,
                        end_y: placed_symbol.y,
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
