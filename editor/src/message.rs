use flits_core::{SymbolIndex, SymbolIndexOrRoot};

use crate::{edit::MovieEdit, FlitsEvent};

pub enum EditorMessage {
    ChangeSelectedSymbol(SymbolIndexOrRoot),
    ChangeSelectedPlacedSymbols(Vec<SymbolIndex>),
    Edit(MovieEdit),
    Undo,
    Redo,
    Event(FlitsEvent),
    TODO,
}
