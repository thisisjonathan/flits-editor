use flits_core::{
    BitmapCacheStatus, BitmapProperties, Movie, MovieClip, MovieClipProperties, MovieProperties,
    PlaceSymbol, PlacedSymbolIndex, Symbol, SymbolIndex, SymbolIndexOrRoot,
};

use crate::undo::{ActionEdit, ChangeEdit};

#[derive(Debug, Clone)]
pub enum MovieChange {
    MovieProperties(MovieProperties),
    BitmapProperties(SymbolIndex, BitmapProperties),
    MovieClipProperties(SymbolIndex, MovieClipProperties),
    PlacedSymbols(Vec<PlacedSymbolChange>),
}
impl ChangeEdit for MovieChange {
    type Model = Movie;

    fn apply(&self, model: &mut Movie) {
        match self {
            MovieChange::MovieProperties(movie_properties) => {
                model.properties = movie_properties.clone();
            }
            MovieChange::BitmapProperties(symbol_index, bitmap_properties) => {
                let Symbol::Bitmap(bitmap) = &mut model.symbols[*symbol_index] else {
                    panic!("Changed symbol is not a bitmap");
                };
                // reset cache when path changes or when animation settings change
                // (because the same image will be interpreted differently)
                if bitmap.properties.path != bitmap_properties.path
                    || bitmap.properties.animation != bitmap_properties.animation
                {
                    // TODO: reuse file data when animation settings change
                    bitmap.invalidate_cache();
                }
                bitmap.properties = bitmap_properties.clone();
            }
            MovieChange::MovieClipProperties(symbol_index, movieclip_properties) => {
                let Symbol::MovieClip(movieclip) = &mut model.symbols[*symbol_index] else {
                    panic!("Changed symbol is not a movieclip");
                };
                movieclip.properties = movieclip_properties.clone();
            }
            MovieChange::PlacedSymbols(changes) => {
                for change in changes {
                    let placed_symbols = model.get_placed_symbols_mut(change.editing_symbol_index);
                    let symbol = &mut placed_symbols[change.placed_symbol_index];
                    change.placed_symbol.clone_into(symbol);
                }
            }
        }
    }

    fn existing_value(&self, model: &Movie) -> Self {
        match self {
            MovieChange::MovieProperties(_) => {
                MovieChange::MovieProperties(model.properties.clone())
            }
            MovieChange::BitmapProperties(symbol_index, _) => {
                let Symbol::Bitmap(bitmap) = &model.symbols[*symbol_index] else {
                    panic!("Existing symbol is not a bitmap");
                };
                MovieChange::BitmapProperties(*symbol_index, bitmap.properties.clone())
            }
            MovieChange::MovieClipProperties(symbol_index, _) => {
                let Symbol::MovieClip(movieclip) = &model.symbols[*symbol_index] else {
                    panic!("Existing symbol is not a movieclip");
                };
                MovieChange::MovieClipProperties(*symbol_index, movieclip.properties.clone())
            }
            MovieChange::PlacedSymbols(changes) => {
                let mut existing_changes = Vec::with_capacity(changes.len());
                for change in changes {
                    let placed_symbols = model.get_placed_symbols(change.editing_symbol_index);
                    let symbol = &placed_symbols[change.placed_symbol_index];
                    existing_changes.push(PlacedSymbolChange {
                        editing_symbol_index: change.editing_symbol_index,
                        placed_symbol_index: change.placed_symbol_index,
                        placed_symbol: symbol.clone(),
                    });
                }
                MovieChange::PlacedSymbols(existing_changes)
            }
        }
    }
}
#[derive(Debug, Clone)]
pub struct PlacedSymbolChange {
    pub editing_symbol_index: SymbolIndexOrRoot,
    pub placed_symbol_index: PlacedSymbolIndex,

    pub placed_symbol: PlaceSymbol,
}
#[derive(Debug, Clone)]
pub enum MovieAction {
    AddMovieClip(String),
    // can only be used as the inverse of AddMovieClip, because we know that it isn't used anywhere yet
    RemoveNewestMovieClip,

    AddPlacedSymbols(Vec<PlacedSymbolAction>),
    RemovePlacedSymbols(Vec<PlacedSymbolAction>),
}
impl ActionEdit for MovieAction {
    type Model = Movie;

    fn apply(&self, model: &mut Movie) {
        match self {
            MovieAction::AddMovieClip(name) => {
                model.symbols.push(Symbol::MovieClip(MovieClip {
                    properties: MovieClipProperties {
                        name: name.clone(),
                        class_name: "".to_string(),
                    },
                    place_symbols: vec![],
                }));
            }
            MovieAction::RemoveNewestMovieClip => {
                model.symbols.pop();
            }
            MovieAction::AddPlacedSymbols(actions) => {
                for action in actions {
                    model
                        .get_placed_symbols_mut(action.editing_symbol_index)
                        .insert(action.placed_symbol_index, action.placed_symbol.clone());
                }
            }
            MovieAction::RemovePlacedSymbols(actions) => {
                // iterate in reverse to make sure the indexes don't change while iterating
                // this assumes the placed symbol indexes are sorted
                for action in actions.iter().rev() {
                    model
                        .get_placed_symbols_mut(action.editing_symbol_index)
                        .remove(action.placed_symbol_index);
                }
            }
        }
    }

    fn invert(self) -> Self {
        match self {
            MovieAction::AddMovieClip(_) => MovieAction::RemoveNewestMovieClip,
            MovieAction::RemoveNewestMovieClip => unreachable!("RemoveNewestMovieClip should never be inverted, it only exists to be the inverse of AddMovieClip"),
            MovieAction::AddPlacedSymbols(actions) => MovieAction::RemovePlacedSymbols(actions),
            MovieAction::RemovePlacedSymbols(actions) => MovieAction::AddPlacedSymbols(actions),
        }
    }
}
#[derive(Debug, Clone)]
pub struct PlacedSymbolAction {
    pub editing_symbol_index: SymbolIndexOrRoot,
    pub placed_symbol: PlaceSymbol,
    pub placed_symbol_index: PlacedSymbolIndex,
}
