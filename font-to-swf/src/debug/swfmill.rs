use std::{io::Write, path::PathBuf, process::Stdio};

use crate::font_to_swf::{FontAllocator, FontSwfBuilder};
use swf::{CharacterId, Font, FontFlag, SwfBuf};
use typed_arena::Arena;

pub(super) fn build_font_swfmill<'a>(
    name: String,
    path: PathBuf,
    characters: String,
    character_id: CharacterId,
    swf_builder: &mut impl FontSwfBuilder<'a>,
    allocator: &'a impl FontAllocator,
    swf_bufs: Arena<SwfBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: make this work properly
    if characters.contains('"') || characters.contains('\\') {
        return Err(format!(
            "Font \"{}\" characters contains \" or \\ which is not allowed",
            name
        )
        .into());
    }

    let dependencies_dir = std::env::current_exe()?
        .parent()
        .ok_or("Editor executable is not in a directory")?
        .join("dependencies");
    // No need to add .exe on windows, Command does that automatically
    let swfmill_path = dependencies_dir.join("swfmill");

    let mut command = std::process::Command::new(swfmill_path);
    // this is a workaround to get swfmill to work on Arch, because swfmill depends on outdated libraries
    // TODO: find a better solution
    command.env("LD_LIBRARY_PATH", dependencies_dir.join("lib"));
    command.arg("simple").arg("stdin");
    // uncomment this to write to an swf instead of stdout
    // command.arg("temp.swf");
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.current_dir(path.parent().unwrap());

    let mut child = command.spawn().map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => {
                "Could not find swfmill executable. There is supposed to be a 'dependencies' directory in the same directory as this program with the mtasc executable.".into()
            }
            _ => format!("Unable to run swfmill: {}", err),
        })?;

    let xml_input = format!(
        r##"<?xml version="1.0" encoding="iso-8859-1" ?>
    <movie width="320" height="240" framerate="12">
        <frame>
            <font id="{}" import="{}" glyphs="{}"/>
        </frame>
    </movie>"##,
        name,
        name,
        characters // can't contain " or \
    );
    let mut stdin = child.stdin.take().expect("Failed to open SwfMill stdin");
    std::thread::spawn(move || {
        stdin
            .write_all(xml_input.as_bytes())
            .expect("Failed to write to stdin");
    });

    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(format!(
            "Error with SwfMill while converting fonts: {}{}",
            std::str::from_utf8(&output.stdout)?,
            std::str::from_utf8(&output.stderr)?
        )
        .into());
    }

    let swf_buf = swf::decompress_swf(&output.stdout[..])?;
    let swf = swf::parse_swf(swf_bufs.alloc(swf_buf))?;

    for tag in &swf.tags {
        let swf::Tag::DefineFont2(define_font_tag) = tag else {
            continue;
        };

        let mut font_flags = define_font_tag.flags;
        // the swf crate doesn't look at the flags to see if it should use wide offsets but instead
        // chooses based on the glyph data.
        // TODO: we should do the same check to make sure they are in sync
        font_flags.remove(FontFlag::HAS_WIDE_OFFSETS);
        swf_builder.add_tag(swf::Tag::DefineFont2(Box::new(Font {
            version: define_font_tag.version,
            id: character_id,
            // i want the name of the file, not the font inside
            // TODO: is this the right choice?
            //name: arenas.alloc_swf_string(path.clone()),
            // new name for debugging purposes
            name: allocator.alloc_swf_string(format!("{} (swfmill)", name).into()),
            language: define_font_tag.language,
            layout: define_font_tag.layout.clone(),
            glyphs: define_font_tag.glyphs.clone(),
            flags: font_flags,
        })));
        /*swf_builder.add_tag(Tag::ExportAssets(vec![ExportedAsset {
            id: character_id,
            name: arenas.alloc_swf_string(path.clone()),
        }]));*/
    }

    Ok(())
}
