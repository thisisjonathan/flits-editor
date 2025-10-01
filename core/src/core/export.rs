use std::{collections::HashMap, path::PathBuf};

use font::build_font;
use swf::*;
use text_field::build_text_field;
use typed_arena::Arena;

use self::{
    as2::compile_as2,
    audio::build_audio,
    bitmap::build_bitmap,
    movieclip::{build_movieclip_inner, build_movieclip_outer},
    preloader::build_preloader,
};

use super::{
    FlitsFont, Movie, PlaceSymbol, PreloaderType, Symbol, SymbolIndex, TextProperties, SWF_VERSION,
};

mod as2;
mod audio;
mod bitmap;
mod font;
mod movieclip;
mod preloader;
mod text_field;

pub fn export_movie_to_swf<'a>(
    movie: &Movie,
    project_directory: PathBuf,
    swf_path: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let header = Header {
        compression: Compression::Zlib,
        version: SWF_VERSION,
        stage_size: Rectangle {
            x_min: Twips::from_pixels(0.0),
            x_max: Twips::from_pixels(movie.properties.width),
            y_min: Twips::from_pixels(0.0),
            y_max: Twips::from_pixels(movie.properties.height),
        },
        frame_rate: Fixed8::from_f32(movie.properties.frame_rate),
        num_frames: movie.num_frames(),
    };
    let mut tags = vec![Tag::SetBackgroundColor(
        movie.properties.background_color.clone().into(),
    )];

    let mut swf_builder = SwfBuilder::new();
    let arenas = Arenas::new();
    if movie.properties.preloader != PreloaderType::None {
        build_preloader(
            movie.properties.preloader.clone(),
            &mut swf_builder,
            &arenas,
            movie.properties.width,
            movie.properties.height,
        )?;
    }
    build_library(
        &movie.symbols,
        &mut swf_builder,
        &arenas,
        project_directory.clone(),
    )?;
    build_placed_symbols_of_root(&movie.root, &mut swf_builder, &arenas)?;

    for tag in swf_builder.tags {
        tags.push(tag);
    }

    let file = std::fs::File::create(swf_path.clone())?;
    let writer = std::io::BufWriter::new(file);
    swf::write_swf(&header, &tags, writer)?;

    compile_as2(
        &movie,
        &swf_builder.state.symbol_index_to_character_id,
        project_directory,
        swf_path,
    )?;

    Ok(())
}

fn build_library<'a>(
    symbols: &Vec<Symbol>,
    swf_builder: &mut SwfBuilder<'a>,
    arenas: &'a Arenas,
    directory: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut symbol_index: SymbolIndex = 0;
    for symbol in symbols {
        match symbol {
            Symbol::Bitmap(bitmap) => {
                build_bitmap(symbol_index, bitmap, swf_builder, arenas, directory.clone())?
            }
            Symbol::MovieClip(movieclip) => {
                build_movieclip_outer(symbol_index, movieclip, swf_builder)?
            }
            Symbol::Font(font) => {
                build_font(symbol_index, font, swf_builder, arenas, directory.clone())?
            }
        }
        symbol_index += 1;
    }

    // create the inner tags of movieclips after we've assigned all the character ids to make sure
    // the character ids for all the symbols exist
    symbol_index = 0;
    for symbol in symbols {
        match symbol {
            Symbol::MovieClip(movieclip) => {
                build_movieclip_inner(symbol_index, movieclip, swf_builder, arenas)?
            }
            _ => {}
        }
        symbol_index += 1;
    }
    build_audio(swf_builder, &arenas, directory)?;
    Ok(())
}

struct SwfBuilder<'a> {
    tags: Vec<Tag<'a>>,
    state: SwfBuilderState,
}
#[derive(Clone)]
struct SwfBuilderState {
    character_id_counter: CharacterId,
    symbol_index_to_character_id: HashMap<SymbolIndex, CharacterId>,
    symbol_index_to_tag_index: HashMap<SymbolIndex, usize>,
}

impl<'a> SwfBuilder<'a> {
    fn new() -> SwfBuilder<'a> {
        SwfBuilder {
            tags: vec![],
            state: SwfBuilderState {
                character_id_counter: 1,
                symbol_index_to_character_id: HashMap::new(),
                symbol_index_to_tag_index: HashMap::new(),
            },
        }
    }
    fn from_state(state: SwfBuilderState) -> Self {
        SwfBuilder {
            tags: vec![],
            state,
        }
    }
    fn next_character_id(&mut self) -> CharacterId {
        let character_id = self.state.character_id_counter;
        self.state.character_id_counter += 1;
        character_id
    }
}

struct Arenas {
    data: Arena<Vec<u8>>,
    strings: Arena<String>,
}
impl Arenas {
    fn new() -> Arenas {
        Arenas {
            data: Arena::new(),
            strings: Arena::new(),
        }
    }
    fn alloc_swf_string(&self, str: String) -> &SwfStr {
        let str_ref = self.strings.alloc(str);
        SwfStr::from_utf8_str(str_ref)
    }
}

fn stop_action<'a>(arenas: &'a Arenas) -> Tag<'a> {
    // hardcode the bytes because creating a whole writer just to write these two bytes is a lot of work
    // and it's not like these bytes are ever going to change
    Tag::DoAction(arenas.data.alloc(vec![
        0x07, // stop
        0x00, // end action
    ]))
}

fn build_placed_symbols_of_root<'a>(
    placed_symbols: &Vec<PlaceSymbol>,
    swf_builder: &mut SwfBuilder<'a>,
    arenas: &'a Arenas,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut tags = vec![];
    for tag in get_placed_symbols_tags(placed_symbols, swf_builder, arenas)? {
        tags.push(tag);
    }
    swf_builder.tags.extend(tags);
    swf_builder.tags.push(stop_action(arenas));
    swf_builder.tags.push(Tag::ShowFrame);
    Ok(())
}
fn get_placed_symbols_tags<'a>(
    placed_symbols: &Vec<PlaceSymbol>,
    swf_builder: &mut SwfBuilder<'a>,
    arenas: &'a Arenas,
) -> Result<Vec<Tag<'a>>, Box<dyn std::error::Error>> {
    let mut i = 0;
    let mut tags = vec![];
    for place_symbol in placed_symbols {
        let mut matrix: Matrix = place_symbol.transform.clone().into();
        let tag_index = swf_builder
            .state
            .symbol_index_to_tag_index
            .get(&place_symbol.symbol_index);

        if let Some(tag_index) = tag_index {
            let tag = &swf_builder.tags[*tag_index];
            // TODO: this is a hacky to solve this
            // use the coordinates as the center of bitmaps instead of the top left
            if let Tag::DefineBitsLossless(define_bits) = tag {
                matrix = matrix
                    * Matrix::translate(
                        Twips::from_pixels(define_bits.width as f64 / -2.0),
                        Twips::from_pixels(define_bits.height as f64 / -2.0),
                    );
            }
        }

        let mut character_id = *swf_builder
            .state
            .symbol_index_to_character_id
            .get(&place_symbol.symbol_index)
            .ok_or_else(|| {
                format!(
                    "No character id for symbol id {}",
                    place_symbol.symbol_index
                )
            })?;

        if let Some(text) = &place_symbol.text {
            // change the character id to the text field instead of the font
            character_id = build_text_field(character_id, text, swf_builder, arenas);

            // use the coordinates as the center of text fields instead of the top left
            // we can't just set the bounds right because Ruffle selections get glitchy when
            // the x_min is negative (Flash is fine with it)
            matrix = matrix
                * Matrix::translate(
                    Twips::from_pixels(text.width / -2.0),
                    Twips::from_pixels(text.height / -2.0),
                );
        }

        tags.push(Tag::PlaceObject(Box::new(PlaceObject {
            version: 2,
            action: PlaceObjectAction::Place(character_id),
            depth: (i as u16) + 1,
            matrix: Some(matrix.into()),
            color_transform: None,
            ratio: None,
            name: if place_symbol.instance_name != "" {
                Some(arenas.alloc_swf_string(place_symbol.instance_name.clone()))
            } else {
                None
            },
            clip_depth: None,
            class_name: None,
            filters: None,
            background_color: None,
            blend_mode: None,
            clip_actions: None,
            has_image: true,
            is_bitmap_cached: None,
            is_visible: Some(true),
            amf_data: None,
        })));
        i += 1;
    }

    Ok(tags)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedFont {
    pub version: u8,
    pub id: CharacterId,
    pub name: String,
    pub language: Language,
    pub layout: Option<FontLayout>,
    pub glyphs: Vec<Glyph>,
    pub flags: FontFlag,
}

impl<'a> From<&Box<swf::Font<'a>>> for OwnedFont {
    fn from(value: &Box<swf::Font<'a>>) -> Self {
        OwnedFont {
            version: value.version,
            id: value.id,
            name: value.name.to_string_lossy(swf::UTF_8),
            language: value.language,
            layout: value.layout.clone(),
            glyphs: value.glyphs.clone(),
            flags: value.flags,
        }
    }
}
pub struct FontContainer {
    swf_builder_state: SwfBuilderState,
    arenas: Arenas,
    owned_swf_fonts: Option<Vec<OwnedFont>>,
}
impl FontContainer {
    pub fn new() -> Self {
        Self {
            swf_builder_state: SwfBuilder::new().state,
            arenas: Arenas::new(),
            owned_swf_fonts: None,
        }
    }
    pub fn convert_fonts(
        &mut self,
        fonts: &Vec<(usize, FlitsFont)>,
        directory: PathBuf,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut swf_fonts = Vec::new();

        let mut swf_builder = SwfBuilder::from_state(self.swf_builder_state.clone());

        for (font_symbol_index, font) in fonts {
            build_font(
                *font_symbol_index,
                &font,
                &mut swf_builder,
                &self.arenas,
                directory.clone(),
            )?;
            if let swf::Tag::DefineFont2(font) = &swf_builder.tags[swf_builder.tags.len() - 2] {
                swf_fonts.push(OwnedFont::from(font));
            }
        }

        self.owned_swf_fonts = Some(swf_fonts);

        self.swf_builder_state = swf_builder.state;

        Ok(())
    }

    // this leaks memory and should only be called once per FontContainer
    pub fn fonts<'a>(&'a self) -> Vec<swf::Font<'a>> {
        self.owned_swf_fonts
            .as_ref()
            .expect("Fonts not converted")
            .iter()
            .map(|owned_font| swf::Font {
                version: owned_font.version,
                id: owned_font.id,
                name: self.arenas.alloc_swf_string(owned_font.name.clone()),
                language: owned_font.language,
                layout: owned_font.layout.clone(),
                glyphs: owned_font.glyphs.clone(),
                flags: owned_font.flags,
            })
            .collect()
    }

    pub fn convert_text_field<'a>(
        &'a mut self,
        font_symbol_index: usize,
        text: TextProperties,
    ) -> Result<EditText<'a>, Box<dyn std::error::Error>> {
        let mut swf_builder = SwfBuilder::from_state(self.swf_builder_state.clone());

        let font_character_id = swf_builder.state.symbol_index_to_character_id[&font_symbol_index];
        build_text_field(font_character_id, &text, &mut swf_builder, &self.arenas);
        if let swf::Tag::DefineEditText(edit_text) = swf_builder.tags.last().unwrap() {
            self.swf_builder_state = swf_builder.state;
            return Ok(*edit_text.clone());
        }

        self.swf_builder_state = swf_builder.state;

        Err("EditText is not the last tag".into())
    }
}
