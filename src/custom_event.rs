//! Custom event type for desktop ruffle

use std::path::PathBuf;

use crate::editor::main::Movie;

/// User-defined events.
pub enum RuffleEvent {
    /// The user requested to create a new project.
    NewFile(NewProjectData),
    
    /// The user requested to open a new local SWF.
    OpenFile,

    /// The user requested to close the current SWF.
    CloseFile,
    
        /// THe user requested to open the about screen
    About,

    /// The user requested to exit Ruffle.
    ExitRequested,
}

pub struct NewProjectData {
    pub movie: Movie,
    pub path: PathBuf,
}
