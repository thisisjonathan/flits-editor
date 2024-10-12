use std::{path::Path, path::PathBuf};

use image::{io::Reader as ImageReader, DynamicImage};
use ruffle_render::bitmap::BitmapHandle;
use serde::{Deserialize, Serialize};
use swf::{Color, Matrix, Twips};

use self::export::export_movie_to_swf;

mod export;

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
        movie.reload_assets(directory);

        movie
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
            if !file_name.ends_with(".png") {
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

    pub fn export(
        &self,
        project_directory: PathBuf,
        swf_path: PathBuf,
    ) -> Result<(), Box<dyn std::error::Error>> {
        export_movie_to_swf(self, project_directory, swf_path)
    }

    pub fn run(swf_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        // No need to add .exe on windows, Command does that automatically
        let ruffle_path = std::env::current_exe()?
            .parent()
            .ok_or("Editor executable is not in a directory")?
            .join("dependencies/ruffle");
        std::process::Command::new(ruffle_path)
            .arg(swf_path)
            .spawn()
            .map_err(|err| match err.kind() {
                std::io::ErrorKind::NotFound => {
                    "Could not find ruffle executable. There is supposed to be a 'dependencies' directory in the same directory as this program with the ruffle executable.".into()
                }
                _ => format!("Unable to run ruffle: {}", err),
            })?;
        Ok(())
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

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct MovieProperties {
    pub width: f64,
    pub height: f64,
    pub frame_rate: f32,
    pub background_color: EditorColor,
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
        <EditorTransform as Into<ruffle_render::matrix::Matrix>>::into(self).into()
    }
}
impl Into<ruffle_render::matrix::Matrix> for EditorTransform {
    fn into(self) -> ruffle_render::matrix::Matrix {
        ruffle_render::matrix::Matrix::create_box(
            self.x_scale as f32,
            self.y_scale as f32,
            Twips::from_pixels(self.x),
            Twips::from_pixels(self.y),
        )
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
