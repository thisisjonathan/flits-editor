use image::{io::Reader as ImageReader, EncodableLayout};
use std::{
    collections::HashMap,
    io::{Read, Write},
    path::PathBuf,
};

use swf::{
    avm1::types::{Action, ConstantPool, Push},
    *,
};

use super::{Bitmap, Movie, MovieClip, PlaceSymbol, Symbol, SymbolIndex, SWF_VERSION};

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
        num_frames: 1,
    };
    let mut tags = vec![Tag::SetBackgroundColor(
        movie.properties.background_color.clone().into(),
    )];
    let mut swf_builder = SwfBuilder {
        tags: vec![],
        character_id_counter: 1,
        symbol_index_to_character_id: HashMap::new(),
        symbol_index_to_tag_index: HashMap::new(),
    };
    build_library(&movie.symbols, &mut swf_builder, project_directory.clone())?;
    build_placed_symbols(&movie.root, &mut swf_builder)?;

    let mut data_storage = vec![];
    let mut string_storage: Vec<String> = vec![];
    let mut swf_string_storage: Vec<&SwfStr> = vec![];
    for i in 0..swf_builder.tags.len() {
        let builder_tag = &swf_builder.tags[i];
        if let SwfBuilderTag::Bitmap(bitmap) = builder_tag {
            data_storage.push(bitmap.data.clone());
        }
        if let SwfBuilderTag::Sound(sound) = builder_tag {
            data_storage.push(sound.data.clone());
        }
        if let SwfBuilderTag::ExportAssets(asset) = builder_tag {
            string_storage.push(asset.name.clone());
        }
    }
    for i in 0..swf_builder.tags.len() {
        let builder_tag = &swf_builder.tags[i];
        if let SwfBuilderTag::ExportAssets(_asset) = builder_tag {
            swf_string_storage.push(SwfStr::from_utf8_str(
                &string_storage[swf_string_storage.len()],
            ));
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
            SwfBuilderTag::Sound(sound) => {
                data_nr += 1;
                Tag::DefineSound(Box::new(Sound {
                    id: sound.id,
                    format: sound.format,
                    num_samples: sound.num_samples,
                    data: &data_storage[data_nr - 1],
                }))
            }
            SwfBuilderTag::ExportAssets(asset) => {
                swf_string_nr += 1;
                Tag::ExportAssets(vec![ExportedAsset {
                    id: asset.character_id,
                    name: &swf_string_storage[swf_string_nr - 1],
                }])
            }
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
    swf_builder: &mut SwfBuilder,
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
    build_audio(swf_builder, directory)?;
    Ok(())
}

fn build_audio(
    swf_builder: &mut SwfBuilder,
    directory: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let asset_dir = directory.join("assets");
    let fs_assets = std::fs::read_dir(asset_dir)?;
    for fs_asset in fs_assets {
        let file = fs_asset?;
        let file_name = file
            .file_name()
            .into_string()
            .map_err(|original_os_string| {
                format!("Non utf-8 filename: '{:?}'", original_os_string)
            })?;
        if file_name.ends_with(".mp3") {
            build_mp3(swf_builder, file, file_name.clone())
                .map_err(|err| format!("Error decoding '{}': {}", file_name, err))?;
        } else if file_name.ends_with(".wav") {
            build_wav(swf_builder, file, file_name.clone())
                .map_err(|err| format!("Error decoding '{}': {}", file_name, err))?;
        }
    }
    Ok(())
}

fn build_wav(
    swf_builder: &mut SwfBuilder,
    file: std::fs::DirEntry,
    file_name: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let reader = hound::WavReader::open(file.path())?;
    let duration = reader.duration();
    let spec = reader.spec();

    if !(spec.channels == 1 || spec.channels == 2) {
        return Err(format!(
            "Wave file should have 1 or 2 channels, has {}",
            spec.channels
        )
        .into());
    }
    if !(spec.bits_per_sample == 8 || spec.bits_per_sample == 16) {
        return Err(format!(
            "Wave file should have 8 or 16 bits per sample, has {}",
            spec.bits_per_sample
        )
        .into());
    }
    let suppored_sample_rate = match spec.sample_rate {
        5512 => true,
        11025 => true,
        22050 => true,
        44100 => true,
        _ => false,
    };
    if !suppored_sample_rate {
        return Err(format!(
            "Wave file should have a sample rate of 5512, 11025, 22050 or 44100, is {}",
            spec.sample_rate
        )
        .into());
    }

    let mut data: Vec<u8> = vec![];
    // use the underlying reader because we just want the data instead of decoding it ourselves
    reader.into_inner().read_to_end(&mut data)?;
    let character_id = swf_builder.next_character_id();
    swf_builder.tags.push(SwfBuilderTag::Sound(SwfBuilderSound {
        id: character_id,
        format: SoundFormat {
            compression: AudioCompression::Uncompressed,
            sample_rate: spec.sample_rate as u16,
            is_stereo: spec.channels == 2,
            is_16_bit: spec.bits_per_sample == 16,
        },
        num_samples: duration,
        data,
    }));
    swf_builder
        .tags
        .push(SwfBuilderTag::ExportAssets(SwfBuilderExportedAsset {
            character_id,
            name: file_name,
        }));
    Ok(())
}

fn build_mp3(
    swf_builder: &mut SwfBuilder,
    file: std::fs::DirEntry,
    file_name: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let data: Vec<u8> = std::fs::read(file.path())?;
    let (header, samples) = puremp3::read_mp3(data.as_slice())?;

    if !(header.channels.num_channels() == 1 || header.channels.num_channels() == 2) {
        return Err(format!(
            "Mp3 should have 1 or 2 channels, has {}",
            header.channels.num_channels()
        )
        .into());
    }
    let suppored_sample_rate = match header.sample_rate.hz() {
        5512 => false, // not allowed for mp3 according to the spec
        11025 => true,
        22050 => true,
        44100 => true,
        _ => false,
    };
    if !suppored_sample_rate {
        return Err(format!(
            "Mp3 should have a sample rate of 11025, 22050 or 44100, is {}",
            header.sample_rate.hz()
        )
        .into());
    }

    // TODO: this decodes the whole mp3 just to get the sample count
    // this is inefficient, it should just read the frame data
    let duration = samples.count();
    let character_id = swf_builder.next_character_id();
    swf_builder.tags.push(SwfBuilderTag::Sound(SwfBuilderSound {
        id: character_id,
        format: SoundFormat {
            compression: AudioCompression::Mp3,
            sample_rate: header.sample_rate.hz() as u16,
            is_stereo: header.channels.num_channels() == 2,
            // according to the spec, this is ignored for compressed formats like mp3 and always decoded to 16 bits
            is_16_bit: true,
        },
        num_samples: duration as u32,
        data,
    }));
    swf_builder
        .tags
        .push(SwfBuilderTag::ExportAssets(SwfBuilderExportedAsset {
            character_id,
            name: file_name,
        }));
    Ok(())
}

fn build_movieclip_outer(
    symbol_index: SymbolIndex,
    movieclip: &MovieClip,
    swf_builder: &mut SwfBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    let character_id = swf_builder.next_character_id();
    swf_builder
        .symbol_index_to_character_id
        .insert(symbol_index, character_id);
    swf_builder
        .symbol_index_to_tag_index
        .insert(symbol_index, swf_builder.tags.len());
    swf_builder
        .tags
        .push(SwfBuilderTag::Tag(Tag::DefineSprite(Sprite {
            id: character_id,
            num_frames: 1,
            tags: vec![], // these are filled in by build_movieclip_inner()
        })));
    if movieclip.properties.class_name.len() > 0 {
        // the movieclip needs to be exported to be able to add a tag to it
        swf_builder
            .tags
            .push(SwfBuilderTag::ExportAssets(SwfBuilderExportedAsset {
                character_id,
                name: movieclip.properties.name.clone(),
            }));
    }
    Ok(())
}

fn build_movieclip_inner(
    symbol_index: SymbolIndex,
    movieclip: &MovieClip,
    swf_builder: &mut SwfBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    let inner_tags = get_placed_symbols_tags(&movieclip.place_symbols, swf_builder)?;
    let tag = &mut swf_builder.tags[swf_builder.symbol_index_to_tag_index[&symbol_index]];
    let SwfBuilderTag::Tag(actual_tag) = tag else {
        return Err(format!("The tag for symbol {} is not a standard tag", symbol_index).into());
    };
    let Tag::DefineSprite(define_sprite_tag) = actual_tag else {
        return Err(format!(
            "The tag for the movieclip with symbol index {} is not a DefineSprite tag",
            symbol_index
        )
        .into());
    };
    define_sprite_tag.tags = inner_tags;
    Ok(())
}

struct SwfBuilder<'a> {
    tags: Vec<SwfBuilderTag<'a>>,
    character_id_counter: CharacterId,
    symbol_index_to_character_id: HashMap<SymbolIndex, CharacterId>,
    symbol_index_to_tag_index: HashMap<SymbolIndex, usize>,
}

impl<'a> SwfBuilder<'a> {
    fn next_character_id(&mut self) -> CharacterId {
        let character_id = self.character_id_counter;
        self.character_id_counter += 1;
        character_id
    }
}

enum SwfBuilderTag<'a> {
    Tag(Tag<'a>),
    // we need this to avoid lifetime issues with DefineBitsLossless because data is &[u8] instead of Vec<u8>
    // TODO: it uses Cow now, we might not need this anymore
    Bitmap(SwfBuilderBitmap),
    // we need this to avoid lifetime issues with DefineSound because data is &[u8] instead of Vec<u8>
    Sound(SwfBuilderSound),
    // avoid lifetime issues with &str, own it instead
    // only export one asset per tag to make the code simpler
    ExportAssets(SwfBuilderExportedAsset),
}
struct SwfBuilderBitmap {
    character_id: CharacterId,
    width: u32,
    height: u32,
    data: Vec<u8>,
}
struct SwfBuilderSound {
    id: CharacterId,
    format: SoundFormat,
    num_samples: u32,
    data: Vec<u8>,
}
struct SwfBuilderExportedAsset {
    character_id: CharacterId,
    name: String,
}

fn build_bitmap<'a>(
    symbol_index: SymbolIndex,
    bitmap: &Bitmap,
    swf_builder: &mut SwfBuilder,
    directory: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: the images are probably already loaded when exporting a movie you are editing, maybe reuse that?
    let img = ImageReader::open(directory.join(bitmap.properties.path.clone()))
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => {
                format!("File not found: '{}'", bitmap.properties.path.clone())
            }
            _ => format!(
                "Unable to open file: '{}' Reason: {}",
                bitmap.properties.path.clone(),
                err
            ),
        })?
        .decode()
        .map_err(|err| {
            format!(
                "Error decoding '{}': {}",
                bitmap.properties.path.clone(),
                err
            )
        })?;
    let image_width = img.width();
    let image_height = img.height();
    let rgba8 = img.into_rgba8();
    let image_data = &mut rgba8.as_bytes().to_owned();
    // convert to argb
    for i in 0..image_width {
        for j in 0..image_height {
            let index: usize = ((i + j * image_width) * 4) as usize;
            let r = image_data[index];
            let g = image_data[index + 1];
            let b = image_data[index + 2];
            let a = image_data[index + 3];
            image_data[index] = a;
            image_data[index + 1] = r;
            image_data[index + 2] = g;
            image_data[index + 3] = b;
        }
    }
    let compressed_image_data_buffer = Vec::new();
    let mut encoder =
        flate2::write::ZlibEncoder::new(compressed_image_data_buffer, flate2::Compression::best());
    encoder.write_all(image_data)?;
    let compressed_image_data = encoder.finish()?;

    let bitmap_id = swf_builder.next_character_id();
    let shape_id = swf_builder.next_character_id();
    swf_builder
        .symbol_index_to_character_id
        .insert(symbol_index, shape_id);
    swf_builder
        .symbol_index_to_tag_index
        .insert(symbol_index, swf_builder.tags.len());
    swf_builder.tags.extend(vec![
        SwfBuilderTag::Bitmap(SwfBuilderBitmap {
            character_id: bitmap_id,
            width: image_width,
            height: image_height,
            data: compressed_image_data,
        }),
        SwfBuilderTag::Tag(Tag::DefineShape(Shape {
            version: 1,
            id: shape_id,
            shape_bounds: Rectangle {
                x_min: Twips::from_pixels(0.0),
                y_min: Twips::from_pixels(0.0),
                x_max: Twips::from_pixels(image_width as f64),
                y_max: Twips::from_pixels(image_height as f64),
            },
            edge_bounds: Rectangle {
                x_min: Twips::from_pixels(0.0),
                y_min: Twips::from_pixels(0.0),
                x_max: Twips::from_pixels(image_width as f64),
                y_max: Twips::from_pixels(image_height as f64),
            },
            flags: ShapeFlag::empty(),
            styles: ShapeStyles {
                /*fill_styles: vec![FillStyle::Color(Color {
                    r: 0,
                    g: 0,
                    b: 255,
                    a: 255,
                })],*/
                fill_styles: vec![FillStyle::Bitmap {
                    id: bitmap_id,
                    matrix: Matrix::scale(Fixed16::from_f64(20.0), Fixed16::from_f64(20.0)),
                    is_repeating: false,
                    is_smoothed: false,
                }],
                line_styles: vec![],
            },
            shape: vec![
                ShapeRecord::StyleChange(Box::new(StyleChangeData {
                    move_to: Some(Point::new(
                        Twips::from_pixels(image_width as f64),
                        Twips::from_pixels(image_height as f64),
                    )),
                    fill_style_0: None,
                    fill_style_1: Some(1),
                    line_style: None,
                    new_styles: None,
                })),
                ShapeRecord::StraightEdge {
                    delta: PointDelta {
                        dx: Twips::from_pixels(-(image_width as f64)),
                        dy: Twips::from_pixels(0.0),
                    },
                },
                ShapeRecord::StraightEdge {
                    delta: PointDelta {
                        dx: Twips::from_pixels(0.0),
                        dy: Twips::from_pixels(-(image_height as f64)),
                    },
                },
                ShapeRecord::StraightEdge {
                    delta: PointDelta {
                        dx: Twips::from_pixels(image_width as f64),
                        dy: Twips::from_pixels(0.0),
                    },
                },
                ShapeRecord::StraightEdge {
                    delta: PointDelta {
                        dx: Twips::from_pixels(0.0),
                        dy: Twips::from_pixels(image_height as f64),
                    },
                },
            ],
        })),
    ]);
    Ok(())
}

fn build_placed_symbols(
    placed_symbols: &Vec<PlaceSymbol>,
    swf_builder: &mut SwfBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut tags = vec![];
    for tag in get_placed_symbols_tags(placed_symbols, swf_builder)? {
        tags.push(SwfBuilderTag::Tag(tag));
    }
    swf_builder.tags.extend(tags);
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
    tags.push(Tag::ShowFrame);

    Ok(tags)
}

fn compile_as2(
    movie: &Movie,
    symbol_index_to_character_id: &HashMap<SymbolIndex, CharacterId>,
    project_directory: PathBuf,
    swf_path: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let dependencies_dir = std::env::current_exe()?
        .parent()
        .ok_or("Editor executable is not in a directory")?
        .join("dependencies");
    // No need to add .exe on windows, Command does that automatically
    let mtasc_path = dependencies_dir.join("mtasc");

    let mut command = std::process::Command::new(mtasc_path);
    // TODO: add -infer?
    command.arg("-swf").arg(swf_path.clone());
    command.arg("-version").arg("8"); // use newer as2 standard library
    command.arg("-cp").arg(dependencies_dir.join("std")); // set class path
    command.arg("-cp").arg(dependencies_dir.join("std8")); // set class path for version 8

    let mut at_least_one_file = false;
    let src_dir = project_directory.join("src");
    std::fs::create_dir_all(src_dir.clone())?;
    // TODO: subdirectories
    for src_file in src_dir.read_dir()? {
        command.arg(src_file?.path());
        at_least_one_file = true;
    }

    if at_least_one_file {
        let output = command.output().map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => {
                "Could not find mtasc executable. There is supposed to be a 'dependencies' directory in the same directory as this program with the mtasc executable.".into()
            }
            _ => format!("Unable to run mtasc (as2 compiler): {}", err),
        })?;

        if !output.status.success() {
            return Err(format!(
                "{}{}",
                std::str::from_utf8(&output.stdout)?,
                std::str::from_utf8(&output.stderr)?
            )
            .into());
        }

        // put placeobject after the class definitions, otherwise it won't work
        let file = std::fs::File::open(swf_path.clone())?;
        let reader = std::io::BufReader::new(file);
        let swf_buf = swf::decompress_swf(reader)?;
        let mut swf = swf::parse_swf(&swf_buf)?;

        // add actions to call Object.registerClass for each movieclip with a class
        let mut symbol_index = 0;
        let mut action_datas = vec![];
        for symbol in &movie.symbols {
            if let Symbol::MovieClip(movieclip) = symbol {
                if movieclip.properties.class_name.len() > 0 {
                    let mut action_data: Vec<u8> = vec![];
                    let mut action_writer =
                        swf::avm1::write::Writer::new(&mut action_data, swf.header.version());
                    let action = Action::ConstantPool(ConstantPool {
                        strings: vec![
                            SwfStr::from_utf8_str("Object"),
                            SwfStr::from_utf8_str("registerClass"),
                            SwfStr::from_utf8_str(&movieclip.properties.name),
                            SwfStr::from_utf8_str(&movieclip.properties.class_name),
                        ],
                    });
                    action_writer.write_action(&action)?;
                    let action = Action::Push(Push {
                        values: vec![swf::avm1::types::Value::ConstantPool(3)],
                    });
                    action_writer.write_action(&action)?;
                    let action = Action::GetVariable;
                    action_writer.write_action(&action)?;
                    let action = Action::Push(Push {
                        values: vec![
                            swf::avm1::types::Value::ConstantPool(2),
                            swf::avm1::types::Value::Int(2),
                            swf::avm1::types::Value::ConstantPool(0),
                        ],
                    });
                    action_writer.write_action(&action)?;
                    let action = Action::GetVariable;
                    action_writer.write_action(&action)?;
                    let action = Action::Push(Push {
                        values: vec![swf::avm1::types::Value::ConstantPool(1)],
                    });
                    action_writer.write_action(&action)?;
                    let action = Action::CallMethod;
                    action_writer.write_action(&action)?;
                    let action = Action::Pop;
                    action_writer.write_action(&action)?;
                    action_datas.push(action_data);
                }
            }
            symbol_index += 1;
        }
        symbol_index = 0;
        let mut action_nr = 0;
        for symbol in &movie.symbols {
            if let Symbol::MovieClip(movieclip) = symbol {
                if movieclip.properties.class_name.len() > 0 {
                    let character_id = *symbol_index_to_character_id
                        .get(&symbol_index)
                        .ok_or("MovieClip with unknown character id")?;
                    // -1 because of ShowFrame
                    swf.tags.insert(
                        swf.tags.len() - 1,
                        Tag::DoInitAction {
                            id: character_id,
                            action_data: &action_datas[action_nr],
                        },
                    );
                    action_nr += 1;
                }
            }
            symbol_index += 1;
        }

        let mut tags_to_place_at_end = vec![];
        let mut index = 0;
        for tag in &swf.tags {
            if matches!(tag, Tag::PlaceObject(_)) {
                tags_to_place_at_end.push(index);
            }
            index += 1;
        }

        // iterate in reverse order to make sure placing the tag at the end doesn't change the index of the other tags
        for index_reference in tags_to_place_at_end.iter().rev() {
            let index = *index_reference;
            // length minus 2 because it swaps with the next one and ShowFrame still needs to be last
            for swap_index in index..swf.tags.len() - 2 {
                swf.tags.swap(swap_index, swap_index + 1);
            }
        }

        // write the new version
        let file = std::fs::File::create(swf_path)?;
        let writer = std::io::BufWriter::new(file);
        swf::write_swf(&swf.header.swf_header(), &swf.tags, writer)?;
    }
    Ok(())
}
