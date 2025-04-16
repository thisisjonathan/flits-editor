use std::{collections::HashMap, path::PathBuf};

use swf::{
    avm1::types::{Action, Push},
    *,
};
use typed_arena::Arena;

use self::{
    as2::compile_as2,
    audio::build_audio,
    bitmap::build_bitmap,
    movieclip::{build_movieclip_inner, build_movieclip_outer},
    preloader::build_preloader,
};

use super::{Movie, PlaceSymbol, PreloaderType, Symbol, SymbolIndex, SWF_VERSION};

mod as2;
mod audio;
mod bitmap;
mod movieclip;
mod preloader;

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

    let mut data_storage = vec![];
    let mut string_storage: Vec<String> = vec![];
    let mut swf_string_storage: Vec<&SwfStr> = vec![];
    // separate lists to make getting the index easier
    let mut action_data_storage = vec![];
    let mut action_string_storage: Vec<String> = vec![];
    let mut action_swf_string_storage: Vec<&SwfStr> = vec![];
    for i in 0..swf_builder.tags.len() {
        let builder_tag = &swf_builder.tags[i];
        match builder_tag {
            SwfBuilderTag::Tag(_) => (), // normal case, no data stored
            SwfBuilderTag::Bitmap(bitmap) => {
                data_storage.push(bitmap.data.clone());
            }
            SwfBuilderTag::ExportAssets(asset) => {
                string_storage.push(asset.name.clone());
            }
            SwfBuilderTag::DefineButton2(button) => {
                for action in &button.actions {
                    data_storage.push(action.action_data.clone());
                }
            }
            SwfBuilderTag::DefineSpriteWithEndAction(_, action_str) => {
                action_string_storage.push(action_str.clone());
            }
        }
    }
    let mut action_string_index = 0;
    for i in 0..swf_builder.tags.len() {
        let builder_tag = &mut swf_builder.tags[i];
        if let SwfBuilderTag::ExportAssets(_asset) = builder_tag {
            swf_string_storage.push(SwfStr::from_utf8_str(
                &string_storage[swf_string_storage.len()],
            ));
        }
        if let SwfBuilderTag::DefineSpriteWithEndAction(_, _) = builder_tag {
            {
                let mut action_data: Vec<u8> = vec![];
                let mut action_writer =
                    swf::avm1::write::Writer::new(&mut action_data, SWF_VERSION);
                action_swf_string_storage.push(SwfStr::from_utf8_str(
                    &action_string_storage[action_string_index],
                ));
                action_string_index += 1;
                let action = Action::Push(Push {
                    values: vec![
                        swf::avm1::types::Value::Double(0.0), // amount of arguments
                        swf::avm1::types::Value::Str(action_swf_string_storage.last().unwrap()),
                    ],
                });
                action_writer.write_action(&action)?;
                let action = Action::CallFunction;
                action_writer.write_action(&action)?;
                let action = Action::Pop;
                action_writer.write_action(&action)?;
                let action = Action::End;
                action_writer.write_action(&action)?;
                action_data_storage.push(action_data);
            }
        }
    }
    let mut action_data_nr = 0;
    for i in 0..swf_builder.tags.len() {
        let builder_tag = &mut swf_builder.tags[i];
        if let SwfBuilderTag::DefineSpriteWithEndAction(sprite, _) = builder_tag {
            sprite.tags.pop();
            sprite
                .tags
                .push(Tag::DoAction(&action_data_storage[action_data_nr]));
            action_data_nr += 1;
            sprite.tags.push(Tag::ShowFrame);
        }
    }

    let mut data_nr = 0;
    let mut swf_string_nr = 0;
    for builder_tag in swf_builder.tags {
        let tag: Tag = match builder_tag {
            SwfBuilderTag::Tag(tag) => tag,
            SwfBuilderTag::Bitmap(bitmap) => {
                data_nr += 1;
                Tag::DefineBitsLossless(DefineBitsLossless {
                    version: 2,
                    id: bitmap.character_id,
                    format: BitmapFormat::Rgb32,
                    width: bitmap.width as u16,
                    height: bitmap.height as u16,
                    data: std::borrow::Cow::from(&data_storage[data_nr - 1]),
                })
            }
            SwfBuilderTag::ExportAssets(asset) => {
                swf_string_nr += 1;
                Tag::ExportAssets(vec![ExportedAsset {
                    id: asset.character_id,
                    name: &swf_string_storage[swf_string_nr - 1],
                }])
            }
            SwfBuilderTag::DefineButton2(button) => {
                let mut actions = vec![];
                for action in button.actions {
                    data_nr += 1;
                    actions.push(ButtonAction {
                        conditions: action.conditions,
                        action_data: &data_storage[data_nr - 1],
                    });
                }
                Tag::DefineButton2(Box::new(Button {
                    id: button.id,
                    is_track_as_menu: button.is_track_as_menu,
                    records: button.records,
                    actions,
                }))
            }
            SwfBuilderTag::DefineSpriteWithEndAction(sprite, _) => Tag::DefineSprite(sprite),
        };
        tags.push(tag);
    }

    let file = std::fs::File::create(swf_path.clone())?;
    let writer = std::io::BufWriter::new(file);
    swf::write_swf(&header, &tags, writer)?;

    compile_as2(
        &movie,
        &swf_builder.symbol_index_to_character_id,
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
                build_bitmap(symbol_index, bitmap, swf_builder, directory.clone())?
            }
            Symbol::MovieClip(movieclip) => {
                build_movieclip_outer(symbol_index, movieclip, swf_builder)?
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
                build_movieclip_inner(symbol_index, movieclip, swf_builder)?
            }
            _ => {}
        }
        symbol_index += 1;
    }
    build_audio(swf_builder, &arenas, directory)?;
    Ok(())
}

struct SwfBuilder<'a> {
    tags: Vec<SwfBuilderTag<'a>>,
    character_id_counter: CharacterId,
    symbol_index_to_character_id: HashMap<SymbolIndex, CharacterId>,
    symbol_index_to_tag_index: HashMap<SymbolIndex, usize>,
}

impl<'a> SwfBuilder<'a> {
    fn new() -> SwfBuilder<'a> {
        SwfBuilder {
            tags: vec![],
            character_id_counter: 1,
            symbol_index_to_character_id: HashMap::new(),
            symbol_index_to_tag_index: HashMap::new(),
        }
    }
    fn next_character_id(&mut self) -> CharacterId {
        let character_id = self.character_id_counter;
        self.character_id_counter += 1;
        character_id
    }
}

struct Arenas {
    data: Arena<Vec<u8>>,
}
impl Arenas {
    fn new() -> Arenas {
        Arenas { data: Arena::new() }
    }
}

enum SwfBuilderTag<'a> {
    Tag(Tag<'a>),
    // we need this to avoid lifetime issues with DefineBitsLossless because data is &[u8] instead of Vec<u8>
    // TODO: it uses Cow now, we might not need this anymore
    Bitmap(SwfBuilderBitmap),
    // avoid lifetime issues with &str, own it instead
    // only export one asset per tag to make the code simpler
    ExportAssets(SwfBuilderExportedAsset),
    // we need this to avoid lifetime issues because action_data is &[u8] instead of Vec<u8>
    DefineButton2(Box<SwfBuilderButton>),
    // adds a DoAction with a call to the method named by the String before the last ShowFrame
    // a more proper way to do this would be to have a list of SwfBuilderTags
    // but that got complicated with lifetimes of lists
    DefineSpriteWithEndAction(Sprite<'a>, String),
}
impl<'a> SwfBuilderTag<'a> {
    pub fn stop_action(arenas: &'a Arenas) -> SwfBuilderTag<'a> {
        // hardcode the bytes because creating a whole writer just to write these two bytes is a lot of work
        // and it's not like these bytes are ever going to change
        SwfBuilderTag::Tag(Tag::DoAction(arenas.data.alloc(vec![
            0x07, // stop
            0x00, // end action
        ])))
    }
}
struct SwfBuilderBitmap {
    character_id: CharacterId,
    width: u32,
    height: u32,
    data: Vec<u8>,
}
struct SwfBuilderExportedAsset {
    character_id: CharacterId,
    name: String,
}
struct SwfBuilderButton {
    pub id: CharacterId,
    pub is_track_as_menu: bool,
    pub records: Vec<ButtonRecord>,
    pub actions: Vec<SwfBuilderButtonAction>,
}
struct SwfBuilderButtonAction {
    pub conditions: ButtonActionCondition,
    pub action_data: Vec<u8>,
}

fn build_placed_symbols_of_root<'a>(
    placed_symbols: &Vec<PlaceSymbol>,
    swf_builder: &mut SwfBuilder<'a>,
    arenas: &'a Arenas,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut tags = vec![];
    for tag in get_placed_symbols_tags(placed_symbols, swf_builder)? {
        tags.push(SwfBuilderTag::Tag(tag));
    }
    swf_builder.tags.extend(tags);
    swf_builder.tags.push(SwfBuilderTag::stop_action(arenas));
    swf_builder.tags.push(SwfBuilderTag::Tag(Tag::ShowFrame));
    Ok(())
}
fn get_placed_symbols_tags<'a>(
    placed_symbols: &Vec<PlaceSymbol>,
    swf_builder: &SwfBuilder,
) -> Result<Vec<Tag<'a>>, Box<dyn std::error::Error>> {
    let mut i = 0;
    let mut tags = vec![];
    for place_symbol in placed_symbols {
        let mut matrix: Matrix = place_symbol.transform.clone().into();
        let tag_index = swf_builder
            .symbol_index_to_tag_index
            .get(&place_symbol.symbol_index);

        // use the coordinates as the center of bitmaps instead of the top left
        if let Some(tag_index) = tag_index {
            let tag = &swf_builder.tags[*tag_index];
            if let SwfBuilderTag::Bitmap(bitmap_tag) = tag {
                matrix = matrix
                    * Matrix::translate(
                        Twips::from_pixels(bitmap_tag.width as f64 / -2.0),
                        Twips::from_pixels(bitmap_tag.height as f64 / -2.0),
                    );
            }
        }

        tags.push(Tag::PlaceObject(Box::new(PlaceObject {
            version: 2,
            action: PlaceObjectAction::Place(
                *swf_builder
                    .symbol_index_to_character_id
                    .get(&place_symbol.symbol_index)
                    .ok_or_else(|| {
                        format!(
                            "No character id for symbol id {}",
                            place_symbol.symbol_index
                        )
                    })?,
            ),
            depth: (i as u16) + 1,
            matrix: Some(matrix.into()),
            color_transform: None,
            ratio: None,
            name: None,
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
