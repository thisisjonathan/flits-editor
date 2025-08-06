use std::path::PathBuf;

use crate::font_to_swf::font_to_swf;
use crate::font_to_swf::FontAllocator;
use crate::font_to_swf::FontSwfBuilder;
use debug::DebugFontSwfBuilder;
use swf::Tag;
use typed_arena::Arena;

mod debug;
mod font_to_swf;

fn main() {
    let allocator = DebugAllocator {
        string_arena: Arena::new(),
    };
    let characters: String = "0123456789".into();
    let directory: PathBuf = "example/assets".into();
    let fs_assets = std::fs::read_dir(directory).unwrap();
    for fs_asset in fs_assets {
        let mut swf_builder = DebugFontSwfBuilderImpl { tags: Vec::new() };
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
            1,
            &mut swf_builder,
            &allocator,
        )
        .unwrap();
        debug::compare_swfmill_font(
            font_name,
            font_path,
            characters.clone(),
            2,
            &mut swf_builder,
            &allocator,
            Arena::new(),
        )
        .unwrap();
    }
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
    fn tags(&'a self) -> &'a Vec<swf::Tag<'a>> {
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
