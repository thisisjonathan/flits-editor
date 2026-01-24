use flits_core::SymbolIndexOrRoot;

use crate::{edit::MovieEdit, FlitsEvent};

pub enum EditorMessage {
    ChangeSelectedSymbol(SymbolIndexOrRoot),
    Edit(MovieEdit),
    Undo,
    Redo,
    Event(FlitsEvent),
    TODO,
}
