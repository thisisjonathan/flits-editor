//! Custom event type for desktop ruffle

use std::{path::PathBuf, time::Duration};

use crate::core::MovieProperties;

/// User-defined events.
#[derive(Debug)]
pub enum RuffleEvent {
    RedrawRequested(Duration),

    /// The user requested to create a new project.
    NewFile(NewProjectData),

    /// The user requested to open a new local SWF.
    OpenFile,

    /// The user requested to close the current SWF.
    CloseFile,

    /// The user requested to open the about screen
    About,

    /// output received from running Ruffle process
    CommandOutput(String),
    RuffleClosed,

    /// The user requested to exit Ruffle.
    ExitRequested,
}

#[derive(Debug, Default, Clone)]
pub struct NewProjectData {
    pub movie_properties: MovieProperties,
    pub path: PathBuf,
}
