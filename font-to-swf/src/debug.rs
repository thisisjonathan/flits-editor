use std::path::PathBuf;

use crate::font_to_swf::FontAllocator;
use crate::font_to_swf::FontSwfBuilder;
use swf::{CharacterId, SwfBuf, Tag};
use typed_arena::Arena;

pub mod comparison_swf;
mod swfmill;

pub(super) trait DebugFontSwfBuilder<'a>: FontSwfBuilder<'a> {
    fn tags(&self) -> &Vec<Tag>;
}

pub(super) fn compare_swfmill_font<'a>(
    name: String,
    path: PathBuf,
    characters: String,
    character_id: CharacterId,
    swf_builder: &mut impl DebugFontSwfBuilder<'a>,
    allocator: &'a impl FontAllocator,
    swf_bufs: Arena<SwfBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: move this call outside this function
    swfmill::build_font_swfmill(
        name,
        path,
        characters.clone(),
        character_id,
        swf_builder,
        allocator,
        swf_bufs,
    )?;
    // use -3 because the last one is at -1, skip the new font and the export tag
    let Tag::DefineFont2(flits_font) = &swf_builder.tags()[swf_builder.tags().len() - 3] else {
        return Err("Flits font is not a font".into());
    };
    let Tag::DefineFont2(swfmill_font) = swf_builder.tags().last().unwrap() else {
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
        println!("Glyph: {}", characters.chars().skip(index).next().unwrap());
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
