use std::env;
use std::error::Error;
use vergen::EmitBuilder;

fn main() -> Result<(), Box<dyn Error>> {
    // Emit version info, and "rerun-if-changed" for relevant files, including build.rs
    EmitBuilder::builder()
        .build_timestamp()
        .cargo_features()
        .git_sha(false)
        .git_commit_timestamp()
        .git_commit_date()
        .emit()?;

    // Embed resource file w/ icon on windows
    // To allow for cross-compilation, this must not be behind cfg(windows)!
    // TODO: windows icon
    //println!("cargo:rerun-if-changed=assets/ruffle_desktop.rc");
    //embed_resource::compile("assets/ruffle_desktop.rc", embed_resource::NONE);

    println!("cargo:rerun-if-env-changed=CFG_RELEASE_CHANNEL");
    let channel = channel();
    if channel == "nightly" || channel == "dev" {
        println!("cargo:rustc-cfg=nightly");
    }
    println!("cargo:rustc-env=CFG_RELEASE_CHANNEL={channel}");

    Ok(())
}

fn channel() -> String {
    if let Ok(channel) = env::var("CFG_RELEASE_CHANNEL") {
        channel
    } else {
        "nightly".to_owned()
    }
}
