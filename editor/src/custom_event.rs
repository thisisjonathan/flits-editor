use std::path::PathBuf;

use flits_core::MovieProperties;

pub enum FlitsEvent {
    NewFile(NewProjectData),
    OpenFile,
    CloseFile,
    About,
    ExitRequested,

    /// output received from running Ruffle process
    CommandOutput(String),
    RuffleClosed,
}

#[derive(Debug, Default, Clone)]
pub struct NewProjectData {
    pub movie_properties: MovieProperties,
    pub path: PathBuf,
}
