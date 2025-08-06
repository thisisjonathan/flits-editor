use crate::font_to_swf::font_to_swf;
use crate::font_to_swf::FontAllocator;
use crate::font_to_swf::FontSwfBuilder;
use debug::DebugFontSwfBuilder;
use swf::Tag;
use typed_arena::Arena;

mod debug;
mod font_to_swf;

fn main() {
    let mut swf_builder = DebugFontSwfBuilderImpl { tags: Vec::new() };
    let allocator = DebugAllocator {
        string_arena: Arena::new(),
    };
    font_to_swf(
        "PixelifySans-Regular.ttf".into(),
        "0123456789".into(),
        "example".into(),
        1,
        &mut swf_builder,
        &allocator,
    )
    .unwrap();
    debug::compare_swfmill_font(
        "PixelifySans-Regular.ttf".into(),
        "0123456789".into(),
        "example".into(),
        2,
        &mut swf_builder,
        &allocator,
        Arena::new(),
    )
    .unwrap();
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
