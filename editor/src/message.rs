use flits_core::SymbolIndexOrRoot;

use crate::FlitsEvent;

pub enum EditorMessage {
    ChangeSelectedSymbol(SymbolIndexOrRoot),
    Event(FlitsEvent),
    TODO,
}
