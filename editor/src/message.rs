use flits_core::{SymbolIndex, SymbolIndexOrRoot};

use crate::{edit::MovieEdit, editor::stage::StageMessage, FlitsEvent};

pub enum EditorMessage {
    Save,
    Export,
    Run,
    OpenNewSymbolWindow,
    ChangeSelectedSymbol(SymbolIndexOrRoot),
    ChangeSelectedPlacedSymbols(Vec<SymbolIndex>),
    SelectAll,
    DeleteSelection,
    ReloadAssets,
    Edit(MovieEdit),
    Undo,
    Redo,
    Stage(StageMessage),
    Event(FlitsEvent),
}
