use flits_core::SymbolIndexOrRoot;

pub enum EditorMessage {
    ChangeSelectedSymbol(SymbolIndexOrRoot),
}
