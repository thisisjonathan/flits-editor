use std::{
    any::Any,
    path::{Path, PathBuf},
};

use image::{DynamicImage, GenericImage, ImageReader};
use serde::{Deserialize, Serialize};
use swf::{Color, Fixed16, Matrix, Twips};

use self::export::export_movie_to_swf;

mod export;
pub mod run;

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
    pub fn load(path: PathBuf) -> Result<Movie, Box<dyn std::error::Error>> {
        let directory = path.parent().unwrap();
        let file = std::fs::File::open(path.clone())?;
        let mut movie: Movie = serde_json::from_reader(file)?;
        movie.reload_assets(directory);

        Ok(movie)
    }

    pub fn reload_assets(&mut self, directory: &Path) {
        self.add_unimported_assets(directory);

        for symbol in self.symbols.iter_mut() {
            let Symbol::Bitmap(bitmap) = symbol else {
                continue;
            };
            bitmap.cache_image(directory);
        }
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
            let is_image = file_name.ends_with(".png");
            let is_font = file_name.ends_with(".ttf");
            if !is_image && !is_font {
                continue;
            }
            let existing_index = existing_assets
                .iter()
                .position(|asset| asset == file_path.as_str());
            if let Some(existing_index) = existing_index {
                // asset is in the list, remove so we don't check it for all the other ones
                existing_assets.remove(existing_index);
            } else {
                // asset doesn't exist yet, add it
                if is_image {
                    self.symbols.push(Symbol::Bitmap(Bitmap {
                        properties: BitmapProperties {
                            name: file_name,
                            path: file_path,
                            animation: None,
                        },
                        cache: BitmapCacheStatus::Uncached,
                    }));
                } else if is_font {
                    self.symbols.push(Symbol::Font(FlitsFont {
                        path: file_name,
                        characters: "1234567890".into(),
                    }))
                }
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

    pub fn export(
        &self,
        project_directory: PathBuf,
        swf_path: PathBuf,
    ) -> Result<(), Box<dyn std::error::Error>> {
        export_movie_to_swf(self, project_directory, swf_path)
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

    pub fn num_frames(&self) -> u16 {
        match self.properties.preloader {
            PreloaderType::None => 1,
            PreloaderType::StartAfterLoading => 2,
            PreloaderType::WithPlayButton => 3,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct MovieProperties {
    pub width: f64,
    pub height: f64,
    pub frame_rate: f32,
    pub background_color: EditorColor,
    pub preloader: PreloaderType,
}
impl Default for MovieProperties {
    fn default() -> Self {
        MovieProperties {
            // TODO: are these good defaults?
            width: 640.0,
            height: 360.0,
            frame_rate: 60.0,
            background_color: EditorColor {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            },
            preloader: PreloaderType::None,
        }
    }
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct EditorColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}
impl Into<Color> for EditorColor {
    fn into(self) -> Color {
        Color {
            r: self.r,
            g: self.g,
            b: self.b,
            a: self.a,
        }
    }
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum PreloaderType {
    None,
    StartAfterLoading,
    WithPlayButton,
}
impl ToString for PreloaderType {
    fn to_string(&self) -> String {
        match self {
            PreloaderType::None => "None".into(),
            PreloaderType::StartAfterLoading => "Start after loading".into(),
            PreloaderType::WithPlayButton => "With play button".into(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum Symbol {
    Bitmap(Bitmap),
    MovieClip(MovieClip),
    Font(FlitsFont),
}

impl Symbol {
    pub fn name(&self) -> String {
        match self {
            Symbol::Bitmap(bitmap) => bitmap.properties.name.clone(),
            Symbol::MovieClip(movieclip) => movieclip.properties.name.clone(),
            Symbol::Font(font) => font.path.clone(),
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
    pub fn type_name(&self) -> &str {
        match self {
            Symbol::Bitmap(_) => "Bitmap",
            Symbol::MovieClip(_) => "MovieClip",
            Symbol::Font(_) => "Font",
        }
    }
    pub fn clone_without_cache(&self) -> Self {
        match self {
            Symbol::Bitmap(bitmap) => Symbol::Bitmap(Bitmap {
                properties: bitmap.properties.clone(),
                // when undoing removing a bitmap the cache is empty because it was removed when the bitmap was removed
                // keeping removed bitmaps cached would be wasteful
                cache: BitmapCacheStatus::Uncached,
            }),
            Symbol::MovieClip(movieclip) => Symbol::MovieClip(movieclip.clone()),
            Symbol::Font(font) => Symbol::Font(font.clone()),
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
                Ok(mut image) => match &self.properties.animation {
                    None => BitmapCacheStatus::Cached(CachedBitmap {
                        image,
                        bitmap_handle: None,
                    }),
                    Some(animation) => {
                        // avoid panic if the frame count is zero
                        // (this should be handled by the input ui already, this check is just to be safe)
                        let mut sub_image_width = if animation.frame_count > 0 {
                            image.width() / animation.frame_count
                        } else {
                            1
                        };
                        // avoid crash when the amount of frames is stupidly big
                        if sub_image_width < 1 {
                            sub_image_width = 1;
                        }
                        let first_frame = image
                            .sub_image(0, 0, sub_image_width, image.height())
                            .to_image();
                        BitmapCacheStatus::Cached(CachedBitmap {
                            image: first_frame.into(),
                            bitmap_handle: None,
                        })
                    }
                },
                Err(err) => BitmapCacheStatus::Invalid(err.to_string()),
            },
            Err(err) => BitmapCacheStatus::Invalid(err.to_string()),
        };
    }
    pub fn invalidate_cache(&mut self) {
        self.cache = BitmapCacheStatus::Uncached;
    }
    pub fn size(&self) -> Option<(u32, u32)> {
        match &self.cache {
            BitmapCacheStatus::Uncached => None,
            BitmapCacheStatus::Cached(cached_bitmap) => {
                Some((cached_bitmap.image.width(), cached_bitmap.image.height()))
            }
            BitmapCacheStatus::Invalid(_) => None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct BitmapProperties {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub animation: Option<Animation>,
}
#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Animation {
    pub frame_count: u32,
    pub frame_delay: u32,
    /// empty string means no end action
    #[serde(default)]
    pub end_action: String,
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
    pub bitmap_handle: Option<Box<dyn BitmapHandle>>,
}
pub trait BitmapHandle {
    fn as_any(&self) -> &dyn Any;
}

#[derive(Serialize, Deserialize, Clone)]
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlaceSymbol {
    pub symbol_index: SymbolIndex,
    #[serde(flatten)]
    pub transform: EditorTransform,
    #[serde(default, skip_serializing_if = "is_default")]
    pub instance_name: String,
    pub text: Option<Box<TextProperties>>,
}
impl PlaceSymbol {
    pub fn from_transform(
        exisiting_place_symbol: &PlaceSymbol,
        transform: EditorTransform,
    ) -> PlaceSymbol {
        PlaceSymbol {
            symbol_index: exisiting_place_symbol.symbol_index,
            transform,
            instance_name: exisiting_place_symbol.instance_name.clone(),
            text: exisiting_place_symbol.text.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EditorTransform {
    pub x: f64,
    pub y: f64,

    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub x_scale: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub y_scale: f64,
}

impl Into<Matrix> for EditorTransform {
    fn into(self) -> Matrix {
        create_box(
            Fixed16::from_f64(self.x_scale),
            Fixed16::from_f64(self.y_scale),
            Twips::from_pixels(self.x),
            Twips::from_pixels(self.y),
        )
    }
}
// copied from ruffle_render src/matrix.rs because swf::Matrix doesn't implement it
fn create_box(
    scale_x: Fixed16,
    scale_y: Fixed16,
    translate_x: Twips,
    translate_y: Twips,
) -> Matrix {
    Matrix {
        a: scale_x,
        c: Fixed16::ZERO,
        tx: translate_x,
        b: Fixed16::ZERO,
        d: scale_y,
        ty: translate_y,
    }
}

fn one() -> f64 {
    1.0
}

fn is_one(value: &f64) -> bool {
    *value == 1.0
}

// from: https://mth.st/blog/skip-default/
fn is_default<T: Default + PartialEq>(t: &T) -> bool {
    t == &T::default()
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct FlitsFont {
    pub path: String,
    pub characters: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct TextProperties {
    pub text: String,
    pub width: f64,
    pub height: f64,
    pub size: f64,
}
