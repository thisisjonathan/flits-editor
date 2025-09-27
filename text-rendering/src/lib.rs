use std::{any::Any, collections::HashMap, ops::DerefMut, sync::Arc};

use compat::{Library, RenderContext, UpdateContext};
use edit_text::EditText;
use font::Font;
use gc_arena::{Arena, Rootable};
use ruffle_render::{
    backend::RenderBackend,
    commands::CommandList,
    transform::{Transform, TransformStack},
};
use tag_utils::SwfMovie;

mod compat;
mod edit_text;
mod font;
mod font_map;
mod html;
mod sandbox;
mod string;
mod tag_utils;

// these abstractions cache fonts and set up the things the code from Ruffle needs.
// it's designed so that the calling code doesn't need to worry about the lifetimes
// of the data inside.
// i got here by trail and error, there is probably a simpler way to do this

struct TextRendererWorld<'gc> {
    update_context: UpdateContext<'gc>,
    edit_texts: HashMap<usize, EditText<'gc>>,
    fonts_container: Box<dyn SwfFontsContainer<'gc> + 'gc>,
}

pub trait SwfFontsContainerBuilder {
    fn build<'a>(&self) -> Box<dyn SwfFontsContainer<'a> + 'a>;
}
// TODO: more accurate name
pub trait SwfFontsContainer<'a> {
    fn convert_fonts(&'a mut self) -> Result<(), Box<dyn std::error::Error>>;
    fn get_fonts<'b>(&'b self) -> &'b Vec<swf::Font<'b>>;

    fn convert_edit_text(
        &'a mut self,
        // this needs to be any because otherwise we need a generic through gc_arena and that doesn't seem to work
        edit_text_properties: Box<dyn Any>,
    ) -> Result<swf::EditText<'a>, Box<dyn std::error::Error>>;
}

pub struct TextRenderer {
    // TODO: do we need to run the garbage collector?
    arena: Arena<Rootable![TextRendererWorld<'_>]>,
}
impl TextRenderer {
    pub fn new(
        fonts_container_builder: Box<dyn SwfFontsContainerBuilder>,
        renderer: &mut Box<dyn RenderBackend>,
    ) -> Self {
        let mut arena = Arena::new(|gc_context| TextRendererWorld {
            update_context: UpdateContext {
                gc_context: gc_context,
                library: Library::new(),
            },
            edit_texts: HashMap::new(),
            fonts_container: fonts_container_builder.build(),
        });
        arena.mutate_root(|_, world: &mut TextRendererWorld<'_>| {
            // TODO: handle errors
            world.fonts_container.convert_fonts().unwrap();
        });
        arena.mutate_root(|gc_context, world: &mut TextRendererWorld<'_>| {
            for swf_font in world.fonts_container.get_fonts() {
                let id = swf_font.id;
                let font = Font::from_swf_tag(
                    gc_context,
                    renderer.deref_mut(),
                    swf_font.clone(),
                    swf::UTF_8, // TODO: is this correct?
                    font::FontType::Embedded,
                );
                world.update_context.library.add_font(id, font);
            }
        });
        Self { arena }
    }
    pub fn add_edit_text<T>(&mut self, edit_text_id: usize, edit_text_properties: T)
    where
        T: 'static,
    {
        self.arena.mutate_root(|_gc_context, world| {
            let swf_edit_text = world
                .fonts_container
                .convert_edit_text(Box::new(edit_text_properties))
                .unwrap();
            world.edit_texts.insert(
                edit_text_id,
                EditText::from_swf_tag(
                    &mut world.update_context,
                    Arc::new(SwfMovie::empty(
                        43, // TODO: is this the right version? is this even used?
                        None,
                    )),
                    swf_edit_text,
                ),
            );
        })
    }
    pub fn render(
        &self,
        edit_text_id: usize,
        transform: Transform,
        renderer: &mut Box<dyn RenderBackend>,
    ) -> CommandList {
        let mut transform_stack = TransformStack::new();
        let mut render_context = RenderContext {
            renderer: renderer.deref_mut(),
            commands: CommandList::new(),
            transform_stack: &mut transform_stack,
        };
        render_context.transform_stack.push(&transform);
        self.arena.mutate(|_, world| {
            world
                .edit_texts
                .get(&edit_text_id)
                .unwrap()
                .render_self(&mut render_context);
        });

        render_context.commands
    }
}
