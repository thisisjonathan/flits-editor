//! Custom event type for desktop ruffle

/// User-defined events.
pub enum RuffleEvent {

    /// The user requested to open a new local SWF.
    OpenFile,

    /// The user requested to close the current SWF.
    CloseFile,
    
        /// THe user requested to open the about screen
    About,

    /// The user requested to exit Ruffle.
    ExitRequested,
}
