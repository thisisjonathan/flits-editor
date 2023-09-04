use std::path::PathBuf;

use ruffle_render::{backend::RenderBackend, commands::{CommandList, Command}, matrix::Matrix, bitmap::{Bitmap, BitmapFormat, BitmapHandle, PixelSnapping}, transform::Transform};
use swf::{Color, Twips, ColorTransform};
use tracing::instrument;
use crate::editor::main::Movie;
use super::main::{Symbol, PlaceSymbol, movie_to_swf};


type Renderer = Box<dyn RenderBackend>;


pub struct Player {
    movie: Movie,
    directory: PathBuf,
    renderer: Renderer,
}

impl Player {
    pub fn new(renderer: Renderer, path: PathBuf) -> Player {
        let movie = crate::editor::main::load_movie(path.clone());
        Player {
            movie,
            directory: PathBuf::from(path.parent().unwrap()),
            renderer
        }
    }
    #[instrument(level = "debug", skip_all)]
    pub fn render(&mut self) {
        let mut commands = CommandList::new();
        // stage background
        commands.commands.push(Command::DrawRect {
            color: Color::from_rgba(0xFFFFFFFF),
            matrix: Matrix::create_box(
                self.movie.width as f32,
                self.movie.height as f32,
                0.0, 
                Twips::from_pixels(0.0),
                Twips::from_pixels(0.0)
            )
        });
        let placed_symbols = &self.movie.root;
        let symbols = &mut self.movie.symbols;
        let renderer = &mut self.renderer;
        
        // set bitmap handles for bitmaps
        for i in 0..symbols.len() {
            let symbol = symbols.get_mut(i).unwrap();
            match symbol {
                Symbol::Bitmap(bitmap) => {
                    let image = bitmap.image.as_ref().expect("Image or symbol not loaded");
                    if bitmap.bitmap_handle.is_none() {
                            let bitmap_handle = renderer.register_bitmap(Bitmap::new(
                                image.width(),
                                image.height(),
                                BitmapFormat::Rgba,
                                image.as_rgba8().expect("Unable to convert image to rgba").to_vec(),
                            )).expect("Unable to register bitmap");
                            bitmap.bitmap_handle = Some(bitmap_handle);
                    }
                }
                Symbol::MovieClip(_) => ()
            }
        }
        
        commands.commands.extend(Player::render_placed_symbols(
            renderer,
            symbols,
            placed_symbols,
            Transform::default()
        ));
        self.renderer.submit_frame(Color::from_rgb(0x222222, 255), commands, vec![]);
    }
    
    fn render_placed_symbols(
        renderer: &mut Box<dyn RenderBackend>,
        symbols: &Vec<Symbol>,
        placed_symbols: &Vec<PlaceSymbol>,
        transform: Transform,
    ) -> Vec<Command> {
        let mut commands = vec![];
        for i in 0..placed_symbols.len() {
            let place_symbol = placed_symbols.get(i).unwrap();
            let symbol = symbols.get(place_symbol.symbol_id as usize).expect("Invalid symbol placed");
            match symbol {
                Symbol::Bitmap(bitmap) => {
                    let bitmap_handle = bitmap.bitmap_handle.as_ref().unwrap();
                    commands.push(Command::RenderBitmap {
                        bitmap: bitmap_handle.clone(),
                        transform: Transform {
                            matrix: transform.matrix*Matrix::translate(Twips::from_pixels(place_symbol.x), Twips::from_pixels(place_symbol.y)),
                            color_transform: ColorTransform::IDENTITY
                        },
                        smoothing: false,
                        pixel_snapping: PixelSnapping::Never, // TODO: figure out a good default
                    });
                }
                Symbol::MovieClip(movieclip) => {
                    commands.extend(Player::render_placed_symbols(
                        renderer,
                        symbols,
                        &movieclip.place_symbols,
                        Transform {
                            matrix: transform.matrix*Matrix::translate(
                                Twips::from_pixels(place_symbol.x),
                                Twips::from_pixels(place_symbol.y),
                            ),
                            color_transform: transform.color_transform
                        }
                    ));
                }
            }
        }
        commands
    }
    
    pub fn renderer_mut(&mut self) -> &mut Renderer {
        &mut self.renderer
    }
    
    pub fn export_swf(&self) {
        let directory = self.directory.clone();
        let swf_path = directory.clone().join("output.swf");
        movie_to_swf(
            &self.movie,
            directory, 
            swf_path
        );
    }
}