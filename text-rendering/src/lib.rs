use std::{marker::PhantomData, ops::DerefMut, sync::Arc};

use compat::{Library, MovieLibrary, RenderContext, UiBackendImpl, UpdateContext};
use edit_text::EditText;
use font::Font;
use gc_arena::{Arena, Mutation, Rootable};
use ruffle_render::{backend::RenderBackend, commands::CommandList, transform::TransformStack};
use tag_utils::SwfMovie;

mod compat;
mod edit_text;
mod font;
mod html;
mod sandbox;
mod string;
mod tag_utils;

struct Temp<'gc> {
    gc_context: &'gc Mutation<'gc>,
}

pub fn render_text(
    swf_font: swf::Font,
    swf_edit_text: swf::EditText,
    renderer: &mut Box<dyn RenderBackend>,
) -> CommandList {
    let mut commands = None;
    Arena::<Rootable![()]>::new(|gc_context| {
        let ui = UiBackendImpl {};
        let font = Font::from_swf_tag(
            gc_context,
            renderer.deref_mut(),
            swf_font,
            swf::UTF_8, // TODO: is this correct?
            font::FontType::Embedded,
        );
        let mut context = UpdateContext {
            gc_context: gc_context,
            library: Library::from_font(font),
            //renderer: renderer.deref_mut(),
            // TODO: don't do this, or do this only once
            // (or maybe it doesn't matter because the struct is empty?)
            ui: Box::leak(Box::new(ui)),
        };
        let edit_text = EditText::from_swf_tag(
            &mut context,
            Arc::new(SwfMovie::empty(
                43, // TODO: is this the right version? is this even used?
                None,
            )),
            swf_edit_text,
        );

        let mut transform_stack = TransformStack::new();
        let mut render_context = RenderContext {
            renderer: renderer.deref_mut(),
            commands: CommandList::new(),
            transform_stack: &mut transform_stack,
        };
        edit_text.render_self(&mut render_context);

        commands = Some(render_context.commands);

        ()
    });

    commands.unwrap()
}
