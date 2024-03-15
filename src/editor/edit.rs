use ruffle_render::matrix::Matrix;
use undo::Edit;

use crate::core::{Movie, PlaceSymbol, PlacedSymbolIndex, SymbolIndexOrRoot, MovieProperties, MovieClipProperties, SymbolIndex, BitmapProperties};

pub enum MovieEdit {
    EditMovieProperties(MoviePropertiesEdit),
    
    EditBitmapProperties(BitmapPropertiesEdit),
    EditMovieClipProperties(MovieClipPropertiesEdit),
    
    MovePlacedSymbol(MovePlacedSymbolEdit),
    AddPlacedSymbol(AddPlacedSymbolEdit),
    RemovePlacedSymbol(RemovePlacedSymbolEdit),
}
impl Edit for MovieEdit {
    type Target = Movie;
    type Output = MoviePropertiesOutput; // the symbol that has changed so the editor can show it

    fn edit(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        match self {
            MovieEdit::EditMovieProperties(edit) => edit.edit(target),
            MovieEdit::EditBitmapProperties(edit) => edit.edit(target),
            MovieEdit::EditMovieClipProperties(edit) => edit.edit(target),
            MovieEdit::MovePlacedSymbol(edit) => edit.edit(target),
            MovieEdit::AddPlacedSymbol(edit) => edit.edit(target),
            MovieEdit::RemovePlacedSymbol(edit) => edit.edit(target),
        }
    }

    fn undo(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        match self {
            MovieEdit::EditMovieProperties(edit) => edit.undo(target),
            MovieEdit::EditBitmapProperties(edit) => edit.undo(target),
            MovieEdit::EditMovieClipProperties(edit) => edit.undo(target),
            MovieEdit::MovePlacedSymbol(edit) => edit.undo(target),
            MovieEdit::AddPlacedSymbol(edit) => edit.undo(target),
            MovieEdit::RemovePlacedSymbol(edit) => edit.undo(target),
        }
    }
}
pub enum MoviePropertiesOutput {
    Stage(SymbolIndexOrRoot),
    Properties(SymbolIndexOrRoot),
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
            crate::core::Symbol::Bitmap(bitmap) => bitmap,
            _ => panic!("Editing symbol that isn't a bitmap")
        };
        bitmap.invalidate_cache();
        bitmap.properties = self.after.clone();
        
        MoviePropertiesOutput::Properties(Some(self.editing_symbol_index))
    }
    fn undo(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        let bitmap = match &mut target.symbols[self.editing_symbol_index] {
            crate::core::Symbol::Bitmap(bitmap) => bitmap,
            _ => panic!("Editing symbol that isn't a bitmap")
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
            crate::core::Symbol::MovieClip(movieclip) => movieclip,
            _ => panic!("Editing symbol that isn't a movieclip")
        };
        movieclip.properties = self.after.clone();
        
        MoviePropertiesOutput::Properties(Some(self.editing_symbol_index))
    }
    fn undo(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        let movieclip = match &mut target.symbols[self.editing_symbol_index] {
            crate::core::Symbol::MovieClip(movieclip) => movieclip,
            _ => panic!("Editing symbol that isn't a movieclip")
        };
        movieclip.properties = self.before.clone();
        
        MoviePropertiesOutput::Properties(Some(self.editing_symbol_index))
    }
}


pub struct MovePlacedSymbolEdit {
    pub editing_symbol_index: SymbolIndexOrRoot,
    pub placed_symbol_index: PlacedSymbolIndex,

    pub start: Matrix,
    pub end: Matrix,
}
impl MovePlacedSymbolEdit {
    fn edit(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        let placed_symbols = target.get_placed_symbols_mut(self.editing_symbol_index);
        let symbol = &mut placed_symbols[self.placed_symbol_index];
        symbol.transform.matrix = self.end.clone();
        
        MoviePropertiesOutput::Stage(self.editing_symbol_index)
    }

    fn undo(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        let placed_symbols = target.get_placed_symbols_mut(self.editing_symbol_index);
        let symbol = &mut placed_symbols[self.placed_symbol_index];
        symbol.transform.matrix = self.start.clone();
        
        MoviePropertiesOutput::Stage(self.editing_symbol_index)
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
        self.placed_symbol_index =
            Some(target.get_placed_symbols(self.editing_symbol_index).len() - 1);
        
        MoviePropertiesOutput::Stage(self.editing_symbol_index)
    }

    fn undo(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        let Some(placed_symbol_index) = self.placed_symbol_index else {
            panic!("Undoing AddPlacedSymbolEdit without placed_symbol_index");
        };
        target
            .get_placed_symbols_mut(self.editing_symbol_index)
            .remove(placed_symbol_index);
        
        MoviePropertiesOutput::Stage(self.editing_symbol_index)
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
        
        MoviePropertiesOutput::Stage(self.editing_symbol_index)
    }

    fn undo(&mut self, target: &mut Movie) -> MoviePropertiesOutput {
        target
            .get_placed_symbols_mut(self.editing_symbol_index)
            .insert(self.placed_symbol_index, self.placed_symbol.clone());
        
        MoviePropertiesOutput::Stage(self.editing_symbol_index)
    }
}
