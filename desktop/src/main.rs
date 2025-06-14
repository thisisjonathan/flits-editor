// error handling code copied from Ruffle
#![deny(clippy::unwrap_used)]
// By default, Windows creates an additional console window for our program.
//
//
// This is silently ignored on non-windows systems.
// See https://docs.microsoft.com/en-us/cpp/build/reference/subsystem?view=msvc-160 for details.
#![windows_subsystem = "windows"]

mod app;
mod cli;
mod player;
mod welcome;

use std::panic::PanicHookInfo;

use anyhow::{Context, Error};
use app::App;
use cli::parse_command_line_arguments;
use flits_editor_lib::FlitsEvent;
use winit::event_loop::EventLoop;

fn init() {
    // When linked with the windows subsystem windows won't automatically attach
    // to the console of the parent process, so we do it explicitly. This fails
    // silently if the parent has no console.
    #[cfg(windows)]
    unsafe {
        use winapi::um::wincon::{AttachConsole, ATTACH_PARENT_PROCESS};
        AttachConsole(ATTACH_PARENT_PROCESS);
    }

    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        prev_hook(info);
        panic_hook(info);
    }));
}

fn panic_hook(info: &PanicHookInfo) {
    // [NA] Let me just point out that PanicInfo::message() exists but isn't stable and that sucks.
    let panic_text = info.to_string();
    let message = if let Some(text) = panic_text.strip_prefix("panicked at '") {
        let location = info.location().map(|l| l.to_string()).unwrap_or_default();
        if let Some(text) = text.strip_suffix(&format!("', {location}")) {
            text.trim()
        } else {
            text.trim()
        }
    } else {
        panic_text.trim()
    };
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("Flits Editor")
        .set_description(&format!(
            "Flits Editor has encountered an error:\n\n\
            {message}\n\n"
        ))
        .show();
}

fn shutdown() {
    // Without explicitly detaching the console cmd won't redraw it's prompt.
    #[cfg(windows)]
    unsafe {
        winapi::um::wincon::FreeConsole();
    }
}

fn start_app() -> Result<(), Error> {
    let cli_params = parse_command_line_arguments();
    let event_loop: EventLoop<FlitsEvent> = EventLoop::with_user_event().build()?;
    let mut app = App::new(event_loop.create_proxy(), cli_params);
    event_loop.run_app(&mut app).context("Event loop failure")
}

fn main() -> Result<(), Error> {
    init();
    let result = start_app();
    #[cfg(windows)]
    if let Err(error) = &result {
        eprintln!("{:?}", error)
    }
    shutdown();
    result
}
