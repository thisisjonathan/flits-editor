use std::path::PathBuf;

use font_to_swf::{FontAllocator, FontSwfBuilder};

use super::{Arenas, SwfBuilder};
use crate::{FlitsFont, SymbolIndex};

// adapted from: https://github.com/djcsdy/swfmill/blob/53d769029adc9d817972e1ccd648b7b335bf78b7/src/swft/swft_import_ttf.cpp#L289
pub(super) fn build_font<'a>(
    symbol_index: SymbolIndex,
    font: &FlitsFont,
    swf_builder: &mut SwfBuilder<'a>,
    arenas: &'a Arenas,
    directory: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let character_id = swf_builder.next_character_id();
    swf_builder
        .symbol_index_to_character_id
        .insert(symbol_index, character_id);
    font_to_swf::font_to_swf(
        // i want the name of the file, not the font inside
        // this only gets used for ExportAssets, the name of the font is set to the font family
        // even when referencing it in AS you still need the family name, not the name in ExportAssets
        font.path.clone(),
        directory.join("assets").join(font.path.clone()),
        font.characters.characters(),
        character_id,
        swf_builder,
        arenas,
    )?;
    // TODO: check for 2 fonts with the same family and bold+italic?

    Ok(())
}
impl<'a> FontSwfBuilder<'a> for SwfBuilder<'a> {
    fn add_tag(&mut self, tag: swf::Tag<'a>) {
        self.tags.push(tag);
    }
}
impl FontAllocator for Arenas {
    fn alloc_swf_string(&self, string: String) -> &swf::SwfStr {
        self.alloc_swf_string(string)
    }
}
