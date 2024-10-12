#![deny(clippy::unwrap_used)]
// By default, Windows creates an additional console window for our program.
//
//
// This is silently ignored on non-windows systems.
// See https://docs.microsoft.com/en-us/cpp/build/reference/subsystem?view=msvc-160 for details.
#![windows_subsystem = "windows"]

mod core;
mod desktop;
mod editor;

use anyhow::Error;
use clap::Parser;
use desktop::app::App;
use desktop::cli::Opt;
use rfd::MessageDialogResult;
use std::cell::RefCell;
use std::panic::PanicInfo;
use url::Url;

thread_local! {
    static RENDER_INFO: RefCell<Option<String>> = RefCell::default();
    static SWF_INFO: RefCell<Option<String>> = RefCell::default();
}

#[cfg(feature = "tracy")]
#[global_allocator]
static GLOBAL: tracing_tracy::client::ProfiledAllocator<std::alloc::System> =
    tracing_tracy::client::ProfiledAllocator::new(std::alloc::System, 0);

static RUFFLE_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "-",
    env!("CFG_RELEASE_CHANNEL"),
    " (",
    env!("VERGEN_GIT_SHA"),
    " ",
    env!("VERGEN_GIT_COMMIT_DATE"),
    ")"
);

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

    let subscriber = tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .finish();
    #[cfg(feature = "tracy")]
    let subscriber = {
        use tracing_subscriber::layer::SubscriberExt;
        let tracy_subscriber = tracing_tracy::TracyLayer::new();
        subscriber.with(tracy_subscriber)
    };
    tracing::subscriber::set_global_default(subscriber).expect("Couldn't set up global subscriber");
}

fn panic_hook(info: &PanicInfo) {
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
    /*if rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("Ruffle")
        .set_description(&format!(
            "Ruffle has encountered a fatal error, this is a bug.\n\n\
            {message}\n\n\
            Please report this to us so that we can fix it. Thank you!\n\
            Pressing Yes will open a browser window."
        ))
        .set_buttons(rfd::MessageButtons::YesNo)
        .show()
        == MessageDialogResult::Yes
    {
        let mut params = vec![
            ("panic_text", info.to_string()),
            ("platform", "Desktop app".to_string()),
            ("operating_system", os_info::get().to_string()),
            ("ruffle_version", RUFFLE_VERSION.to_string()),
        ];
        let mut extra_info = vec![];
        SWF_INFO.with(|i| {
            if let Some(swf_name) = i.take() {
                extra_info.push(format!("Filename: {swf_name}\n"));
                params.push(("title", format!("Crash on {swf_name}")));
            }
        });
        RENDER_INFO.with(|i| {
            if let Some(render_info) = i.take() {
                extra_info.push(format!("### Render Info\n{render_info}\n"));
            }
        });
        if !extra_info.is_empty() {
            params.push(("extra_info", extra_info.join("\n")));
        }
        if let Ok(url) = Url::parse_with_params("https://github.com/ruffle-rs/ruffle/issues/new?assignees=&labels=bug&template=crash_report.yml", &params) {
            let _ = webbrowser::open(url.as_str());
        }
    }*/
}

fn shutdown() {
    // Without explicitly detaching the console cmd won't redraw it's prompt.
    #[cfg(windows)]
    unsafe {
        winapi::um::wincon::FreeConsole();
    }
}

fn main() {
    init();
    let opt = Opt::parse();
    App::new(opt).and_then(|app| app.run()).unwrap();
    /*#[cfg(windows)]
    if let Err(error) = &result {
        eprintln!("{:?}", error)
    }*/
    shutdown();
}
