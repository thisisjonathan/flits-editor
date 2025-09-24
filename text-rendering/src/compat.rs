use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use gc_arena::{Gc, Mutation};
use ruffle_render::{backend::RenderBackend, commands::CommandList, transform::TransformStack};
use swf::CharacterId;

use crate::{
    font::{DefaultFont, Font, FontQuery, FontType},
    font_map::FontMap,
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
    font_map: FontMap<'gc>,
    pub movie_library: MovieLibrary<'gc>,
}
impl<'gc> Library<'gc> {
    pub fn new() -> Self {
        Self {
            font_map: FontMap::default(),
            movie_library: MovieLibrary {
                fonts: HashMap::new(),
            },
        }
    }
    pub fn add_font(&mut self, id: CharacterId, font: Font<'gc>) {
        self.font_map.register(font);
        self.movie_library.fonts.insert(id, font);
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
        let query = FontQuery::new(font_type, name.to_owned(), is_bold, is_italic);
        self.font_map.find(&query)
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
    fonts: HashMap<CharacterId, Font<'gc>>,
}
impl<'gc> MovieLibrary<'gc> {
    pub fn get_font(&self, id: CharacterId) -> Option<Font<'gc>> {
        self.fonts.get(&id).copied()
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
