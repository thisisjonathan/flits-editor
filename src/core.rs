use std::{collections::HashMap, io::Write, path::Path, path::PathBuf};

use image::{io::Reader as ImageReader, DynamicImage, EncodableLayout};
use ruffle_render::{bitmap::BitmapHandle, transform::Transform};
use serde::{Deserialize, Serialize};
use swf::{
    avm1::types::{Action, ConstantPool, Push},
    *,
};

pub type SymbolIndex = usize;
pub type SymbolIndexOrRoot = Option<SymbolIndex>;
pub type PlacedSymbolIndex = usize;

// this is hardcoded because otherwise the entire application needs to understand
// what features are available in what version
// TODO: switch to latest as2 version instead of latest version?
const SWF_VERSION: u8 = 43; // latest flash player version

#[derive(Serialize, Deserialize)]
pub struct Movie {
    pub properties: MovieProperties,

    pub symbols: Vec<Symbol>,
    pub root: Vec<PlaceSymbol>,
}
impl Default for Movie {
    fn default() -> Self {
        Movie::from_properties(MovieProperties::default())
    }
}
impl Movie {
    pub fn from_properties(properties: MovieProperties) -> Self {
        Movie {
            properties,
            symbols: vec![],
            root: vec![],
        }
    }
    pub fn load(path: PathBuf) -> Movie {
        let directory = path.parent().unwrap();
        let file = std::fs::File::open(path.clone()).expect("Unable to load file");
        let mut movie: Movie = serde_json::from_reader(file).expect("Unable to load file");

        movie.add_unimported_assets(directory);

        for symbol in movie.symbols.iter_mut() {
            let Symbol::Bitmap(bitmap) = symbol else {
                continue;
            };
            bitmap.cache_image(directory);
        }

        movie
    }

    fn add_unimported_assets(&mut self, directory: &Path) {
        let asset_dir = directory.join("assets");
        std::fs::create_dir_all(asset_dir.clone()).unwrap();

        let mut existing_assets: Vec<String> = self
            .symbols
            .iter()
            .filter_map(|symbol| match symbol {
                Symbol::Bitmap(bitmap) => Some(bitmap.properties.path.clone()),
                _ => None,
            })
            .collect();

        let fs_assets = std::fs::read_dir(asset_dir).unwrap();
        for fs_asset in fs_assets {
            let file = fs_asset.unwrap();
            let file_name = file.file_name().into_string().unwrap();
            let file_path = format!("assets/{}", file_name);
            let existing_index = existing_assets
                .iter()
                .position(|asset| asset == file_path.as_str());
            if let Some(existing_index) = existing_index {
                // asset is in the list, remove so we don't check it for all the other ones
                existing_assets.remove(existing_index);
            } else {
                // asset doesn't exist yet, add it
                self.symbols.push(Symbol::Bitmap(Bitmap {
                    properties: BitmapProperties {
                        name: file_name,
                        path: file_path,
                    },
                    cache: BitmapCacheStatus::Uncached,
                }));
            }
        }
    }

    pub fn save(&self, path: &Path) {
        let file = std::fs::File::options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .unwrap();
        serde_json::to_writer(file, self).unwrap();
    }

    pub fn export(&self, project_directory: PathBuf, swf_path: PathBuf) {
        movie_to_swf(self, project_directory, swf_path);
    }

    pub fn run(swf_path: &PathBuf) {
        // No need to add .exe on windows, Command does that automatically
        let ruffle_path = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .join("dependencies/ruffle");
        std::process::Command::new(ruffle_path)
            .arg(swf_path)
            .spawn()
            .unwrap();
    }

    pub fn get_placed_symbols(&self, symbol_index: SymbolIndexOrRoot) -> &Vec<PlaceSymbol> {
        if let Some(symbol_index) = symbol_index {
            if let Symbol::MovieClip(movieclip) = &self.symbols[symbol_index] {
                &movieclip.place_symbols
            } else {
                &self.root
            }
        } else {
            &self.root
        }
    }

    pub fn get_placed_symbols_mut(
        &mut self,
        symbol_index: SymbolIndexOrRoot,
    ) -> &mut Vec<PlaceSymbol> {
        if let Some(symbol_index) = symbol_index {
            if let Symbol::MovieClip(movieclip) = &mut self.symbols[symbol_index] {
                &mut movieclip.place_symbols
            } else {
                &mut self.root
            }
        } else {
            &mut self.root
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct MovieProperties {
    pub width: f64,
    pub height: f64,
    pub frame_rate: f32,
}
impl Default for MovieProperties {
    fn default() -> Self {
        MovieProperties {
            // TODO: are these good defaults?
            width: 640.0,
            height: 360.0,
            frame_rate: 60.0,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum Symbol {
    Bitmap(Bitmap),
    MovieClip(MovieClip),
}

impl Symbol {
    pub fn name(&self) -> String {
        match self {
            Symbol::Bitmap(bitmap) => bitmap.properties.name.clone(),
            Symbol::MovieClip(movieclip) => movieclip.properties.name.clone(),
        }
    }
    pub fn is_invalid(&self) -> bool {
        match self {
            Symbol::Bitmap(bitmap) => match bitmap.cache {
                BitmapCacheStatus::Invalid(_) => true,
                _ => false,
            },
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Bitmap {
    pub properties: BitmapProperties,
    #[serde(skip)]
    pub cache: BitmapCacheStatus,
}
impl Bitmap {
    pub fn cache_image(&mut self, directory: &Path) {
        self.cache = match ImageReader::open(directory.join(self.properties.path.clone())) {
            Ok(reader) => match reader.decode() {
                Ok(image) => BitmapCacheStatus::Cached(CachedBitmap {
                    image,
                    bitmap_handle: None,
                }),
                Err(err) => BitmapCacheStatus::Invalid(err.to_string()),
            },
            Err(err) => BitmapCacheStatus::Invalid(err.to_string()),
        };
    }
    pub fn invalidate_cache(&mut self) {
        self.cache = BitmapCacheStatus::Uncached;
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct BitmapProperties {
    pub name: String,
    pub path: String,
}

#[derive(Default)]
pub enum BitmapCacheStatus {
    #[default]
    Uncached,
    Cached(CachedBitmap),
    Invalid(String),
}
pub struct CachedBitmap {
    pub image: DynamicImage,
    pub bitmap_handle: Option<BitmapHandle>,
}

#[derive(Serialize, Deserialize)]
pub struct MovieClip {
    pub properties: MovieClipProperties,
    pub place_symbols: Vec<PlaceSymbol>,
}
#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct MovieClipProperties {
    pub name: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub class_name: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PlaceSymbol {
    pub symbol_index: SymbolIndex,
    #[serde(with = "TransformDef")]
    pub transform: Transform,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Transform")]
struct TransformDef {
    #[serde(with = "MatrixDef")]
    pub matrix: ruffle_render::matrix::Matrix,
    #[serde(with = "color_transform_def")]
    #[serde(default, skip_serializing_if = "is_default")]
    pub color_transform: ColorTransform,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "ruffle_render::matrix::Matrix")]
struct MatrixDef {
    /// Serialized as `scale_x` in SWF files
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub a: f32,

    /// Serialized as `rotate_skew_0` in SWF files
    #[serde(default, skip_serializing_if = "is_default")]
    pub b: f32,

    /// Serialized as `rotate_skew_1` in SWF files
    #[serde(default, skip_serializing_if = "is_default")]
    pub c: f32,

    /// Serialized as `scale_y` in SWF files
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub d: f32,

    /// The X translation in twips. Labeled `TranslateX` in SWF19.
    #[serde(with = "twips_def")]
    pub tx: Twips,

    /// The Y translation in twips. Labeled `TranslateX` in SWF19.
    #[serde(with = "twips_def")]
    pub ty: Twips,
}

fn one() -> f32 {
    1.0
}

fn is_one(value: &f32) -> bool {
    *value == 1.0
}

// from: https://mth.st/blog/skip-default/
fn is_default<T: Default + PartialEq>(t: &T) -> bool {
    t == &T::default()
}

mod twips_def {
    use serde::{de::Visitor, Deserializer, Serializer};
    use swf::Twips;

    pub fn serialize<S>(twips: &Twips, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(twips.to_pixels())
    }
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Twips, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_f64(TwipsVisitor)
    }

    struct TwipsVisitor;

    impl<'de> Visitor<'de> for TwipsVisitor {
        type Value = Twips;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("twips")
        }

        fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Twips::from_pixels(v))
        }

        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Twips::from_pixels(v as f64))
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Twips::from_pixels(v as f64))
        }
    }
}

mod color_transform_def {
    use serde::{Deserializer, Serializer};
    use swf::ColorTransform;

    pub fn serialize<S>(_color_transform: &ColorTransform, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // TODO
        serializer.serialize_none()
    }

    pub fn deserialize<'de, D>(_deserializer: D) -> Result<ColorTransform, D::Error>
    where
        D: Deserializer<'de>,
    {
        // TODO
        Ok(ColorTransform::IDENTITY)
    }
}

fn movie_to_swf<'a>(movie: &Movie, project_directory: PathBuf, swf_path: PathBuf) {
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
    let mut tags = vec![Tag::SetBackgroundColor(Color {
        r: 0,
        g: 0,
        b: 255,
        a: 255,
    })];
    let mut swf_builder = SwfBuilder {
        tags: vec![],
        character_id_counter: 1,
        symbol_index_to_character_id: HashMap::new(),
    };
    build_library(&movie.symbols, &mut swf_builder, project_directory.clone());
    build_placed_symbols(&movie.root, &mut swf_builder);

    let mut data_storage = vec![];
    let mut string_storage: Vec<String> = vec![];
    let mut swf_string_storage: Vec<&SwfStr> = vec![];
    for i in 0..swf_builder.tags.len() {
        let builder_tag = &swf_builder.tags[i];
        if let SwfBuilderTag::Bitmap(bitmap) = builder_tag {
            data_storage.push(bitmap.data.clone());
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

    let mut bitmap_nr = 0;
    let mut swf_string_nr = 0;
    for builder_tag in swf_builder.tags {
        let tag: Tag = match builder_tag {
            SwfBuilderTag::Tag(tag) => tag,
            SwfBuilderTag::Bitmap(bitmap) => {
                bitmap_nr += 1;
                Tag::DefineBitsLossless(DefineBitsLossless {
                    version: 2,
                    id: bitmap.character_id,
                    format: BitmapFormat::Rgb32,
                    width: bitmap.width as u16,
                    height: bitmap.height as u16,
                    data: &data_storage[bitmap_nr - 1],
                })
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

    let file = std::fs::File::create(swf_path.clone()).unwrap();
    let writer = std::io::BufWriter::new(file);
    swf::write_swf(&header, &tags, writer).unwrap();

    compile_as2(
        &movie,
        &swf_builder.symbol_index_to_character_id,
        project_directory,
        swf_path,
    );
}

fn build_library<'a>(symbols: &Vec<Symbol>, swf_builder: &mut SwfBuilder, directory: PathBuf) {
    let mut symbol_index: SymbolIndex = 0;
    for symbol in symbols {
        match symbol {
            Symbol::Bitmap(bitmap) => {
                build_bitmap(symbol_index, bitmap, swf_builder, directory.clone())
            }
            Symbol::MovieClip(movieclip) => build_movieclip(symbol_index, movieclip, swf_builder),
        }
        symbol_index += 1;
    }
}

fn build_movieclip(symbol_index: SymbolIndex, movieclip: &MovieClip, swf_builder: &mut SwfBuilder) {
    let character_id = swf_builder.next_character_id();
    swf_builder
        .symbol_index_to_character_id
        .insert(symbol_index, character_id);
    swf_builder
        .tags
        .push(SwfBuilderTag::Tag(Tag::DefineSprite(Sprite {
            id: character_id,
            num_frames: 1,
            tags: get_placed_symbols_tags(&movieclip.place_symbols, swf_builder),
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
}

struct SwfBuilder<'a> {
    tags: Vec<SwfBuilderTag<'a>>,
    character_id_counter: CharacterId,
    symbol_index_to_character_id: HashMap<SymbolIndex, CharacterId>,
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
    Bitmap(SwfBuilderBitmap),
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
struct SwfBuilderExportedAsset {
    character_id: CharacterId,
    name: String,
}

fn build_bitmap<'a>(
    symbol_index: SymbolIndex,
    bitmap: &Bitmap,
    swf_builder: &mut SwfBuilder,
    directory: PathBuf,
) {
    // TODO: the images are probably already loaded when exporting a movie you are editing, maybe reuse that?
    let img = ImageReader::open(directory.join(bitmap.properties.path.clone()))
        .expect("Unable to read image")
        .decode()
        .expect("Unable to decode image");
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
    encoder.write(image_data).expect("Unable to compress image");
    let compressed_image_data = encoder.finish().unwrap();

    let bitmap_id = swf_builder.next_character_id();
    let shape_id = swf_builder.next_character_id();
    swf_builder
        .symbol_index_to_character_id
        .insert(symbol_index, shape_id);
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
    ])
}

fn build_placed_symbols(placed_symbols: &Vec<PlaceSymbol>, swf_builder: &mut SwfBuilder) {
    let mut tags = vec![];
    for tag in get_placed_symbols_tags(placed_symbols, swf_builder) {
        tags.push(SwfBuilderTag::Tag(tag));
    }
    swf_builder.tags.extend(tags);
}
fn get_placed_symbols_tags<'a>(
    placed_symbols: &Vec<PlaceSymbol>,
    swf_builder: &SwfBuilder,
) -> Vec<Tag<'a>> {
    let mut i = 0;
    let mut tags = vec![];
    for place_symbol in placed_symbols {
        tags.push(Tag::PlaceObject(Box::new(PlaceObject {
            version: 2,
            action: PlaceObjectAction::Place(
                *swf_builder
                    .symbol_index_to_character_id
                    .get(&place_symbol.symbol_index)
                    .unwrap_or_else(|| {
                        panic!(
                            "No character id for symbol id {}",
                            place_symbol.symbol_index
                        )
                    }),
            ),
            depth: (i as u16) + 1,
            matrix: Some(place_symbol.transform.matrix.into()),
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

    tags
}

fn compile_as2(
    movie: &Movie,
    symbol_index_to_character_id: &HashMap<SymbolIndex, CharacterId>,
    project_directory: PathBuf,
    swf_path: PathBuf,
) {
    let dependencies_dir = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("dependencies");
    // No need to add .exe on windows, Command does that automatically
    let mtasc_path = dependencies_dir.join("mtasc");

    let mut command = std::process::Command::new(mtasc_path);
    command.arg("-swf").arg(swf_path.clone());
    command.arg("-version").arg("8"); // use newer as2 standard library
    command.arg("-cp").arg(dependencies_dir.join("std")); // set class path
    command.arg("-cp").arg(dependencies_dir.join("std8")); // set class path for version 8

    let mut at_least_one_file = false;
    let src_dir = project_directory.join("src");
    std::fs::create_dir_all(src_dir.clone()).unwrap();
    // TODO: subdirectories
    for src_file in src_dir.read_dir().unwrap() {
        command.arg(src_file.unwrap().path());
        at_least_one_file = true;
    }

    if at_least_one_file {
        let output = command.output().unwrap();
        println!("mtasc status: {}", output.status);
        std::io::stdout().write_all(&output.stdout).unwrap();
        std::io::stderr().write_all(&output.stderr).unwrap();
        // TODO: error handling

        // put placeobject after the class definitions, otherwise it won't work
        let file = std::fs::File::open(swf_path.clone()).unwrap();
        let reader = std::io::BufReader::new(file);
        let swf_buf = swf::decompress_swf(reader).unwrap();
        let mut swf = swf::parse_swf(&swf_buf).unwrap();

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
                    action_writer.write_action(&action).unwrap();
                    let action = Action::Push(Push {
                        values: vec![swf::avm1::types::Value::ConstantPool(3)],
                    });
                    action_writer.write_action(&action).unwrap();
                    let action = Action::GetVariable;
                    action_writer.write_action(&action).unwrap();
                    let action = Action::Push(Push {
                        values: vec![
                            swf::avm1::types::Value::ConstantPool(2),
                            swf::avm1::types::Value::Int(2),
                            swf::avm1::types::Value::ConstantPool(0),
                        ],
                    });
                    action_writer.write_action(&action).unwrap();
                    let action = Action::GetVariable;
                    action_writer.write_action(&action).unwrap();
                    let action = Action::Push(Push {
                        values: vec![swf::avm1::types::Value::ConstantPool(1)],
                    });
                    action_writer.write_action(&action).unwrap();
                    let action = Action::CallMethod;
                    action_writer.write_action(&action).unwrap();
                    let action = Action::Pop;
                    action_writer.write_action(&action).unwrap();
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
                    let character_id = *symbol_index_to_character_id.get(&symbol_index).unwrap();
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
        let file = std::fs::File::create(swf_path).unwrap();
        let writer = std::io::BufWriter::new(file);
        swf::write_swf(&swf.header.swf_header(), &swf.tags, writer).unwrap();
    }
}
