use std::{path::PathBuf, time::{Instant, Duration}};

use ruffle_render::{backend::{RenderBackend, ViewportDimensions}, commands::{CommandList, Command}, matrix::Matrix, bitmap::{Bitmap, BitmapFormat, PixelSnapping}, transform::Transform};
use swf::{Color, Twips, ColorTransform};
use tracing::instrument;
use winit::event::{MouseButton, ElementState};
use crate::editor::main::Movie;
use super::main::{Symbol, PlaceSymbol, movie_to_swf};


type Renderer = Box<dyn RenderBackend>;

struct DragData {
    symbol_start_x: f64,
    symbol_start_y: f64,
    start_x: f64,
    start_y: f64,
    place_symbol_index: usize,
}

pub struct Player {
    movie: Movie,
    directory: PathBuf,
    renderer: Renderer,
    
    selection: Vec<usize>,
    
    drag_data: Option<DragData>,
}

impl Player {
    pub fn new(renderer: Renderer, path: PathBuf) -> Player {
        let movie = crate::editor::main::load_movie(path.clone());
        Player {
            movie,
            directory: PathBuf::from(path.parent().unwrap()),
            renderer,
            
            selection: vec![],
            
            drag_data: None,
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
    
    pub fn handle_mouse_move(&mut self, mouse_x: f64, mouse_y: f64) {
        if let Some(drag_data) = &self.drag_data {
            let place_symbol = self.movie.root.get_mut(drag_data.place_symbol_index).unwrap();
            place_symbol.x = drag_data.symbol_start_x+mouse_x-drag_data.start_x;
            place_symbol.y = drag_data.symbol_start_y+mouse_y-drag_data.start_y;
        }
    }
    
    pub fn handle_mouse_input(&mut self, mouse_x: f64, mouse_y: f64, button: MouseButton, state: ElementState) {
        if button == MouseButton::Left && state == ElementState::Pressed {
            self.selection = vec![];
            // iterate from top to bottom to get the item that's on top
            for i in (0..self.movie.root.len()).rev() {
                let place_symbol = self.movie.root.get_mut(i).unwrap();
                let mut width = 32.0;
                let mut height = 32.0;
                let symbol = self.movie.symbols.get(place_symbol.symbol_id as usize).expect("Invalid symbol placed");
                // TODO: movieclip size
                if let Symbol::Bitmap(bitmap) = symbol {
                    if let Some(image) = &bitmap.image {
                        width = image.width() as f64;
                        height = image.height() as f64;
                    }
                }
                if mouse_x > place_symbol.x &&
                   mouse_y > place_symbol.y &&
                   mouse_x < place_symbol.x+width &&
                   mouse_y < place_symbol.y+height
                {
                    self.drag_data = Some(DragData {
                        symbol_start_x: place_symbol.x,
                        symbol_start_y: place_symbol.y,
                        start_x: mouse_x,
                        start_y: mouse_y,
                        place_symbol_index: i,
                    });
                    self.selection = vec![i];
                    break;
                }
            }
        }
        if button == MouseButton::Left && state == ElementState::Released {
            if let Some(drag_data) = &self.drag_data {
                let place_symbol = self.movie.root.get_mut(drag_data.place_symbol_index).unwrap();
                place_symbol.x = drag_data.symbol_start_x+mouse_x-drag_data.start_x;
                place_symbol.y = drag_data.symbol_start_y+mouse_y-drag_data.start_y;
                self.drag_data = None;
            }
        }
    }
    
    pub fn do_ui(&mut self, egui_ctx: &egui::Context) -> bool {
        let mut has_mutated = false;
        
        if self.selection.len() > 0 && egui_ctx.input_mut(|input| {
                input.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Delete))
        }) {
            self.delete_selection();
        }
        
        egui::SidePanel::right("library")
            .resizable(false) // resizing causes glitches
            .min_width(150.0)
            .show(egui_ctx, |ui| {
            ui.heading("Library");
            for i in 0..self.movie.symbols.len() {
                let symbol = self.movie.symbols.get(i).unwrap();
                let response = ui.selectable_label(false, symbol.name());
                let response = response.interact(egui::Sense::drag());
                
                if response.drag_released() {
                    let mouse_pos = response.interact_pointer_pos().unwrap();
                    // TODO: don't hardcode the menu height
                    self.movie.root.push(PlaceSymbol { symbol_id: i as u16, x: mouse_pos.x as f64, y: mouse_pos.y as f64 - 24.0 });
                    has_mutated = true;
                }
            }
        });
        
        egui::TopBottomPanel::bottom("properties").show(egui_ctx, |ui| {
            if self.selection.len() == 0 {
                ui.heading("Movie properties");
                self.show_movie_properties(ui);
            } else if self.selection.len() == 1 {
                ui.heading("Placed symbol properties");
                self.show_placed_symbol_properties(ui, *self.selection.get(0).unwrap());
            } else {
                ui.label("Multiple items selected");
            }
        });
        
        has_mutated
    }
    
    fn delete_selection(&mut self) {
        // because the list is sorted and we are traversing from the end to the beginning
        // we can safely remove placed items without changing the indices of the rest of the selection
        self.selection.sort();
        for i in (0..self.selection.len()).rev() {
            let placed_symbol_index = *self.selection.get(i).unwrap();
            self.movie.root.remove(placed_symbol_index);
        }
        self.selection = vec![];
    }
    
    fn show_movie_properties(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("movie_properties_grid").show(ui, |ui| {
            ui.label("Width:");
            ui.add(egui::DragValue::new(&mut self.movie.width));
            ui.end_row();
            
            ui.label("Height:");
            ui.add(egui::DragValue::new(&mut self.movie.height));
            ui.end_row();
        });
    }
    
    fn show_placed_symbol_properties(&mut self, ui: &mut egui::Ui, placed_symbol_index: usize) {
        let placed_symbol = self.movie.root.get_mut(placed_symbol_index).unwrap();
        egui::Grid::new(format!("placed_symbol_{placed_symbol_index}_properties_grid")).show(ui, |ui| {
            ui.label("x");
            ui.add(egui::DragValue::new(&mut placed_symbol.x));
            ui.end_row();
            
            ui.label("y");
            ui.add(egui::DragValue::new(&mut placed_symbol.y));
            ui.end_row();
        });
    }
    
    pub fn renderer_mut(&mut self) -> &mut Renderer {
        &mut self.renderer
    }
    
    pub fn set_viewport_dimensions(&mut self, dimensions: ViewportDimensions) {
        self.renderer.set_viewport_dimensions(dimensions);
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