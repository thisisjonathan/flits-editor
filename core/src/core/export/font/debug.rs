use std::{io::Write, path::PathBuf, process::Stdio};

use swf::{CharacterId, ExportedAsset, Font, FontFlag, Tag};

use crate::{FlitsFont, SymbolIndex};

use super::{Arenas, SwfBuilder};

pub(super) fn compare_swfmill_font<'a>(
    symbol_index: SymbolIndex,
    font: &FlitsFont,
    swf_builder: &mut SwfBuilder<'a>,
    arenas: &'a Arenas,
    directory: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    build_font_swfmill(symbol_index, font, swf_builder, arenas, directory)?;
    // use -3 because the last one is at -1, skip the new font and the export tag
    let Tag::DefineFont2(flits_font) = &swf_builder.tags[swf_builder.tags.len() - 3] else {
        return Err("Flits font is not a font".into());
    };
    let Tag::DefineFont2(swfmill_font) = swf_builder.tags.last().unwrap() else {
        return Err("SWFMill font is not a font".into());
    };

    println!("Debugging Flits font vs SWFMill font");
    println!(
        "Flits glyphs: {} SWFMill glyps: {}",
        flits_font.glyphs.len(),
        swfmill_font.glyphs.len()
    );
    if flits_font.glyphs.len() != swfmill_font.glyphs.len() {
        return Err("SWFMill and Flits fonts do not have the same number of glyphs".into());
    }

    println!(
        "Layout: Flits: {:?} SWFMill: {:?}",
        flits_font.layout, swfmill_font.layout
    );

    for index in 0..flits_font.glyphs.len() {
        println!(
            "Glyph: {}",
            font.characters.chars().skip(index).next().unwrap()
        );
        let flits_glyph = &flits_font.glyphs[index];
        let swfmill_glyph = &swfmill_font.glyphs[index];
        println!(
            "Advance: Flits: {:?} SWFMill: {:?}",
            flits_glyph.advance, swfmill_glyph.advance
        );
        println!(
            "Bounds: Flits: {} SWFMill: {}",
            debug_twips_rect(flits_glyph.bounds),
            debug_twips_rect(swfmill_glyph.bounds)
        )
    }

    Ok(())
}
/// Display in pixels instead of twips
fn debug_twips_rect(rect: Option<swf::Rectangle<swf::Twips>>) -> String {
    match rect {
        Some(rect) => format!(
            "x_min: {} x_max: {} y_min: {}, y_max: {}",
            rect.x_min.to_pixels(),
            rect.x_max.to_pixels(),
            rect.y_min.to_pixels(),
            rect.y_max.to_pixels()
        ),
        None => "None".into(),
    }
}
fn build_font_swfmill<'a>(
    _symbol_index: SymbolIndex,
    font: &FlitsFont,
    swf_builder: &mut SwfBuilder<'a>,
    arenas: &'a Arenas,
    directory: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: make this work properly
    if font.characters.contains('"') || font.characters.contains('\\') {
        return Err(format!(
            "Font \"{}\" characters contains \" or \\ which is not allowed",
            font.path
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
    command.current_dir(directory.join("assets"));

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
        font.path,
        font.path,
        font.characters // can't contain " or \
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
    let swf = swf::parse_swf(arenas.swf_bufs.alloc(swf_buf))?;

    let character_id = swf_builder.next_character_id();
    for tag in &swf.tags {
        let swf::Tag::DefineFont2(define_font_tag) = tag else {
            continue;
        };

        /*swf_builder
        .symbol_index_to_character_id
        .insert(symbol_index, character_id);*/
        let mut font_flags = define_font_tag.flags;
        // the swf crate doesn't look at the flags to see if it should use wide offsets but instead
        // chooses based on the glyph data.
        // TODO: we should do the same check to make sure they are in sync
        font_flags.remove(FontFlag::HAS_WIDE_OFFSETS);
        swf_builder.tags.push(swf::Tag::DefineFont2(Box::new(Font {
            version: define_font_tag.version,
            id: character_id,
            // i want the name of the file, not the font inside
            // TODO: is this the right choice?
            //name: arenas.alloc_swf_string(font.path.clone()),
            // new name for debugging purposes
            name: arenas.alloc_swf_string("TEST_SWFMILL_FONT".into()),
            language: define_font_tag.language,
            layout: define_font_tag.layout.clone(),
            glyphs: define_font_tag.glyphs.clone(),
            flags: font_flags,
        })));
        /*swf_builder.tags.push(Tag::ExportAssets(vec![ExportedAsset {
            id: character_id,
            name: arenas.alloc_swf_string(font.path.clone()),
        }]));*/
    }

    Ok(())
}
