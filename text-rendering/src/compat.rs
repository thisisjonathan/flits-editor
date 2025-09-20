use std::{marker::PhantomData, sync::Arc};

use gc_arena::Mutation;
use ruffle_render::{backend::RenderBackend, commands::CommandList, transform::TransformStack};
use swf::CharacterId;

use crate::{
    font::{DefaultFont, Font, FontType},
    tag_utils::SwfMovie,
};

// for compatibility with Ruffle to make the html module compile
pub struct UpdateContext<'gc> {
    /// The mutation context to allocate and mutate `Gc` pointers.
    ///
    /// NOTE: This is redundant with `strings.gc()`, but is used by
    /// too much code to remove.
    pub gc_context: &'gc Mutation<'gc>,

    pub library: Library<'gc>,

    /// The renderer, used by the display objects to draw themselves.
    //pub renderer: &'gc mut dyn RenderBackend,

    /// The UI backend, used to detect user interactions.
    pub ui: &'gc mut dyn UiBackend,
}
impl<'gc> UpdateContext<'gc> {
    /// Convenience method to retrieve the current GC context. Note that explicitly writing
    /// `self.gc_context` can be sometimes necessary to satisfy the borrow checker.
    #[inline(always)]
    pub fn gc(&self) -> &'gc Mutation<'gc> {
        self.gc_context
    }
}
pub struct Library<'gc> {
    font: Font<'gc>,
    pub movie_library: MovieLibrary<'gc>,
}
impl<'gc> Library<'gc> {
    pub fn from_font(font: Font<'gc>) -> Self {
        Self {
            font,
            movie_library: MovieLibrary {
                phantom: PhantomData::default(),
            },
        }
    }
    pub fn library_for_movie_mut(&mut self, swf_movie: Arc<SwfMovie>) -> &mut MovieLibrary<'gc> {
        &mut self.movie_library
    }
    pub fn get_embedded_font_by_name(
        &self,
        name: &str,
        font_type: FontType,
        is_bold: bool,
        is_italic: bool,
        movie: Option<Arc<SwfMovie>>,
    ) -> Option<Font<'gc>> {
        Some(self.font)
    }

    /// Returns the default Font implementations behind the built in names (ie `_sans`)
    pub fn default_font(
        &mut self,
        name: DefaultFont,
        is_bold: bool,
        is_italic: bool,
        ui: &dyn UiBackend,
        renderer: &mut dyn RenderBackend,
        gc_context: &Mutation<'gc>,
    ) -> Vec<Font<'gc>> {
        todo!()
    }
}

pub trait UiBackend {}
pub struct UiBackendImpl {}
impl UiBackend for UiBackendImpl {}

pub struct MovieLibrary<'gc> {
    pub phantom: PhantomData<&'gc ()>,
}
impl<'gc> MovieLibrary<'gc> {
    pub fn get_font(&self, id: CharacterId) -> Option<Font<'gc>> {
        None // TODO
    }
}

#[derive(Debug, Clone)]
pub struct Drawing {}

pub struct RenderContext<'a> {
    /// The renderer, used by the display objects to register themselves.
    pub renderer: &'a mut dyn RenderBackend,
    /// The command list, used by the display objects to draw themselves.
    pub commands: CommandList,
    /// The transform stack controls the matrix and color transform as we traverse the display hierarchy.
    pub transform_stack: &'a mut TransformStack,
}
