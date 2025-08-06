use std::path::PathBuf;

use crate::font_to_swf::font_to_swf;
use crate::font_to_swf::FontAllocator;
use crate::font_to_swf::FontSwfBuilder;
use debug::DebugFontSwfBuilder;
use swf::Tag;
use typed_arena::Arena;

mod debug;
mod font_to_swf;

/// Converts all fonts in example/assets to swf with both Flits Editor and swfmill and
/// writes an swf with all the fonts in text fields to example/output.swf
/// for visual comparison. Also prints metrics from both conversions.
fn main() {
    let allocator = DebugAllocator {
        string_arena: Arena::new(),
    };
    let characters: String = "0123456789".into();
    let directory: PathBuf = "example/assets".into();
    let fs_assets = std::fs::read_dir(directory).unwrap();
    let mut swf_builder = DebugFontSwfBuilderImpl { tags: Vec::new() };
    let mut character_id = 1;
    let mut font_character_ids = Vec::new();
    for fs_asset in fs_assets {
        let file = fs_asset.unwrap();
        let file_name = file
            .file_name()
            .into_string()
            .map_err(|original_os_string| format!("Non utf-8 filename: '{:?}'", original_os_string))
            .unwrap();
        if !file_name.ends_with(".ttf") {
            continue;
        }

        println!("Converting font: '{}'", file_name);

        let font_name: String = file_name;
        let font_path = file.path();
        font_to_swf(
            font_name.clone(),
            font_path.clone(),
            characters.clone(),
            character_id,
            &mut swf_builder,
            &allocator,
        )
        .unwrap();

        debug::compare_swfmill_font(
            font_name,
            font_path,
            characters.clone(),
            character_id + 1,
            &mut swf_builder,
            &allocator,
            Arena::new(),
        )
        .unwrap();

        font_character_ids.push((character_id, character_id + 1));
        character_id += 2;
    }
    debug::comparison_swf::create_comparision_swf(
        swf_builder.tags,
        font_character_ids,
        "example/output.swf".into(),
    )
    .unwrap();
    println!("Wrote comparison swf to {}", "example/output.swf");
}
struct DebugFontSwfBuilderImpl<'a> {
    tags: Vec<Tag<'a>>,
}
impl<'a> FontSwfBuilder<'a> for DebugFontSwfBuilderImpl<'a> {
    fn add_tag(&mut self, tag: swf::Tag<'a>) {
        self.tags.push(tag);
    }
}
impl<'a> DebugFontSwfBuilder<'a> for DebugFontSwfBuilderImpl<'a> {
    fn tags(&self) -> &Vec<swf::Tag> {
        &self.tags
    }
}
struct DebugAllocator {
    string_arena: Arena<String>,
}
impl FontAllocator for DebugAllocator {
    fn alloc_swf_string(&self, string: String) -> &swf::SwfStr {
        let str_ref = self.string_arena.alloc(string);
        swf::SwfStr::from_utf8_str(str_ref)
    }
}
