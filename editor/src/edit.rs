use undo::Edit;

use flits_core::{
    BitmapProperties, Movie, MovieClip, MovieClipProperties, MovieProperties, PlaceSymbol,
    PlacedSymbolIndex, Symbol, SymbolIndex, SymbolIndexOrRoot,
};

pub enum MovieEdit {
    EditMovieProperties(MoviePropertiesEdit),

    AddMovieClip(AddMovieClipEdit),
    RemoveSymbol(RemoveSymbolEdit),

    EditBitmapProperties(BitmapPropertiesEdit),
    EditMovieClipProperties(MovieClipPropertiesEdit),

    EditPlacedSymbol(PlacedSymbolEdit),
    AddPlacedSymbol(AddPlacedSymbolEdit),
    RemovePlacedSymbol(RemovePlacedSymbolEdit),
}
impl Edit for MovieEdit {
    type Target = Movie;
    type Output = MoviePropertiesOutput; // the symbol that has changed so the editor can show it

    fn edit(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        match self {
            MovieEdit::EditMovieProperties(edit) => edit.edit(target),
            MovieEdit::AddMovieClip(edit) => edit.edit(target),
            MovieEdit::RemoveSymbol(edit) => edit.edit(target),
            MovieEdit::EditBitmapProperties(edit) => edit.edit(target),
            MovieEdit::EditMovieClipProperties(edit) => edit.edit(target),
            MovieEdit::EditPlacedSymbol(edit) => edit.edit(target),
            MovieEdit::AddPlacedSymbol(edit) => edit.edit(target),
            MovieEdit::RemovePlacedSymbol(edit) => edit.edit(target),
        }
    }

    fn undo(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        match self {
            MovieEdit::EditMovieProperties(edit) => edit.undo(target),
            MovieEdit::AddMovieClip(edit) => edit.undo(target),
            MovieEdit::RemoveSymbol(edit) => edit.undo(target),
            MovieEdit::EditBitmapProperties(edit) => edit.undo(target),
            MovieEdit::EditMovieClipProperties(edit) => edit.undo(target),
            MovieEdit::EditPlacedSymbol(edit) => edit.undo(target),
            MovieEdit::AddPlacedSymbol(edit) => edit.undo(target),
            MovieEdit::RemovePlacedSymbol(edit) => edit.undo(target),
        }
    }
}
pub enum MoviePropertiesOutput {
    Stage(SymbolIndexOrRoot),
    Properties(SymbolIndexOrRoot),
    PlacedSymbolProperties(SymbolIndexOrRoot, PlacedSymbolIndex),
    // needs to be a seperate item because the selection needs to be reset
    RemovedPlacedSymbol(SymbolIndexOrRoot),
}

pub struct AddMovieClipEdit {
    pub name: String,
}
impl AddMovieClipEdit {
    fn edit(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        target.symbols.push(Symbol::MovieClip(MovieClip {
            properties: MovieClipProperties {
                name: self.name.clone(),
                class_name: "".to_string(),
            },
            place_symbols: vec![],
        }));
        MoviePropertiesOutput::Stage(Some(target.symbols.len() - 1))
    }
    fn undo(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        target.symbols.pop();
        MoviePropertiesOutput::Stage(None)
    }
}
pub struct RemoveSymbolEdit {
    pub symbol_index: SymbolIndex,
    pub symbol: Symbol, // for undoing
    pub remove_place_symbol_edits: Vec<RemovePlacedSymbolEdit>,
}
impl RemoveSymbolEdit {
    fn edit(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        self.remove_place_symbol_edits = vec![];
        // remove the placed symbols that place this symbol
        self.remove_placed_symbols(target, None);
        for i in 0..target.symbols.len() {
            match target.symbols[i] {
                Symbol::MovieClip(_) => {
                    self.remove_placed_symbols(target, Some(i));
                }
                _ => {}
            }
        }
        for i in 0..self.remove_place_symbol_edits.len() {
            self.remove_place_symbol_edits[i].edit(target);
        }
        target.symbols.remove(self.symbol_index);

        MoviePropertiesOutput::Stage(None)
    }
    fn remove_placed_symbols(&mut self, target: &mut Movie, symbol_index: SymbolIndexOrRoot) {
        let placed_symbols = target.get_placed_symbols_mut(symbol_index);
        for i in (0..placed_symbols.len()).rev() {
            // if the placed symbol is the movieclip we are removing
            if placed_symbols[i].symbol_index == self.symbol_index {
                // remove the placed symbol
                self.remove_place_symbol_edits.push(RemovePlacedSymbolEdit {
                    editing_symbol_index: symbol_index,
                    placed_symbol_index: i,
                    placed_symbol: placed_symbols[i].clone(),
                });
            } else if placed_symbols[i].symbol_index > self.symbol_index {
                // decrease the symbol index because removing the movieclip causes the index of the other moveclips to change
                placed_symbols[i].symbol_index -= 1;
            }
        }
    }
    fn undo(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        self.increase_placed_symbols(target.get_placed_symbols_mut(None));
        for i in 0..target.symbols.len() {
            match target.symbols[i] {
                Symbol::MovieClip(_) => {
                    self.increase_placed_symbols(target.get_placed_symbols_mut(Some(i)))
                }
                _ => {}
            }
        }
        target
            .symbols
            .insert(self.symbol_index, self.symbol.clone_without_cache());
        for i in 0..self.remove_place_symbol_edits.len() {
            self.remove_place_symbol_edits[i].undo(target);
        }
        match &self.symbol {
            Symbol::Bitmap(_) => MoviePropertiesOutput::Properties(Some(self.symbol_index)),
            Symbol::MovieClip(_) => MoviePropertiesOutput::Stage(Some(self.symbol_index)),
        }
    }
    fn increase_placed_symbols(&self, placed_symbols: &mut Vec<PlaceSymbol>) {
        for i in (0..placed_symbols.len()).rev() {
            if placed_symbols[i].symbol_index >= self.symbol_index {
                // increase the symbol index to make room for the reinserted symbol
                placed_symbols[i].symbol_index += 1;
            }
        }
    }
}

pub struct MoviePropertiesEdit {
    pub before: MovieProperties,
    pub after: MovieProperties,
}
impl MoviePropertiesEdit {
    fn edit(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        target.properties = self.after.clone();
        MoviePropertiesOutput::Properties(None) // root because you are editing the movie properties
    }
    fn undo(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        target.properties = self.before.clone();
        MoviePropertiesOutput::Properties(None) // root because you are editing the movie properties
    }
}

pub struct BitmapPropertiesEdit {
    pub editing_symbol_index: SymbolIndex,

    pub before: BitmapProperties,
    pub after: BitmapProperties,
}
impl BitmapPropertiesEdit {
    fn edit(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        let bitmap = match &mut target.symbols[self.editing_symbol_index] {
            flits_core::Symbol::Bitmap(bitmap) => bitmap,
            _ => panic!("Editing symbol that isn't a bitmap"),
        };
        bitmap.invalidate_cache();
        bitmap.properties = self.after.clone();

        MoviePropertiesOutput::Properties(Some(self.editing_symbol_index))
    }
    fn undo(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        let bitmap = match &mut target.symbols[self.editing_symbol_index] {
            flits_core::Symbol::Bitmap(bitmap) => bitmap,
            _ => panic!("Editing symbol that isn't a bitmap"),
        };
        bitmap.invalidate_cache();
        bitmap.properties = self.before.clone();

        MoviePropertiesOutput::Properties(Some(self.editing_symbol_index))
    }
}

pub struct MovieClipPropertiesEdit {
    pub editing_symbol_index: SymbolIndex,

    pub before: MovieClipProperties,
    pub after: MovieClipProperties,
}
impl MovieClipPropertiesEdit {
    fn edit(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        let movieclip = match &mut target.symbols[self.editing_symbol_index] {
            flits_core::Symbol::MovieClip(movieclip) => movieclip,
            _ => panic!("Editing symbol that isn't a movieclip"),
        };
        movieclip.properties = self.after.clone();

        MoviePropertiesOutput::Properties(Some(self.editing_symbol_index))
    }
    fn undo(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        let movieclip = match &mut target.symbols[self.editing_symbol_index] {
            flits_core::Symbol::MovieClip(movieclip) => movieclip,
            _ => panic!("Editing symbol that isn't a movieclip"),
        };
        movieclip.properties = self.before.clone();

        MoviePropertiesOutput::Properties(Some(self.editing_symbol_index))
    }
}

pub struct PlacedSymbolEdit {
    pub editing_symbol_index: SymbolIndexOrRoot,
    pub placed_symbol_index: PlacedSymbolIndex,

    pub start: PlaceSymbol,
    pub end: PlaceSymbol,
}
impl PlacedSymbolEdit {
    fn edit(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        let placed_symbols = target.get_placed_symbols_mut(self.editing_symbol_index);
        let symbol = &mut placed_symbols[self.placed_symbol_index];
        self.end.clone_into(symbol);

        MoviePropertiesOutput::PlacedSymbolProperties(
            self.editing_symbol_index,
            self.placed_symbol_index,
        )
    }

    fn undo(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        let placed_symbols = target.get_placed_symbols_mut(self.editing_symbol_index);
        let symbol = &mut placed_symbols[self.placed_symbol_index];
        self.start.clone_into(symbol);

        MoviePropertiesOutput::PlacedSymbolProperties(
            self.editing_symbol_index,
            self.placed_symbol_index,
        )
    }
}
pub struct AddPlacedSymbolEdit {
    pub editing_symbol_index: SymbolIndexOrRoot,
    pub placed_symbol: PlaceSymbol,
    pub placed_symbol_index: Option<PlacedSymbolIndex>, // for removing when undoing
}
impl AddPlacedSymbolEdit {
    fn edit(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        target
            .get_placed_symbols_mut(self.editing_symbol_index)
            .push(self.placed_symbol.clone());
        let placed_symbol_index = target.get_placed_symbols(self.editing_symbol_index).len() - 1;
        self.placed_symbol_index = Some(placed_symbol_index);

        MoviePropertiesOutput::PlacedSymbolProperties(
            self.editing_symbol_index,
            placed_symbol_index,
        )
    }

    fn undo(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        let Some(placed_symbol_index) = self.placed_symbol_index else {
            panic!("Undoing AddPlacedSymbolEdit without placed_symbol_index");
        };
        target
            .get_placed_symbols_mut(self.editing_symbol_index)
            .remove(placed_symbol_index);

        MoviePropertiesOutput::RemovedPlacedSymbol(self.editing_symbol_index)
    }
}
pub struct RemovePlacedSymbolEdit {
    pub editing_symbol_index: SymbolIndexOrRoot,
    pub placed_symbol_index: PlacedSymbolIndex,
    pub placed_symbol: PlaceSymbol, // for adding when undoing
}
impl RemovePlacedSymbolEdit {
    fn edit(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        target
            .get_placed_symbols_mut(self.editing_symbol_index)
            .remove(self.placed_symbol_index);

        MoviePropertiesOutput::RemovedPlacedSymbol(self.editing_symbol_index)
    }

    fn undo(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        target
            .get_placed_symbols_mut(self.editing_symbol_index)
            .insert(self.placed_symbol_index, self.placed_symbol.clone());

        MoviePropertiesOutput::PlacedSymbolProperties(
            self.editing_symbol_index,
            self.placed_symbol_index,
        )
    }
}
