use std::path::PathBuf;

use crate::core::{Movie, PlaceSymbol, Symbol, MovieClip};
use crate::desktop::custom_event::RuffleEvent;
use egui::{Widget, Vec2};
use ruffle_render::{
    backend::{RenderBackend, ViewportDimensions},
    bitmap::{Bitmap, BitmapFormat, PixelSnapping},
    commands::{Command, CommandList},
    matrix::Matrix,
    transform::Transform,
};
use swf::{Color, ColorTransform, Twips};
use tracing::instrument;
use winit::{
    event::{ElementState, MouseButton},
    event_loop::EventLoopProxy,
};

// also defined in desktop/gui.rs
pub const MENU_HEIGHT: u32 = 48;

type Renderer = Box<dyn RenderBackend>;

struct DragData {
    symbol_start_x: f64,
    symbol_start_y: f64,
    start_x: f64,
    start_y: f64,
    place_symbol_index: usize,
}

pub struct Editor {
    movie: Movie,
    project_file_path: PathBuf,
    directory: PathBuf,
    renderer: Renderer,
    
    editing_clip: Option<usize>,

    selection: Vec<usize>,
    drag_data: Option<DragData>,
    
    new_symbol_window: Option<NewSymbolWindow>,
}

struct Menu<'a> {
    name: &'a str,
    items: &'a [MenuItem<'a>],
}

struct MenuItem<'a> {
    name: &'a str,
    keyboard_shortcut: Option<egui::KeyboardShortcut>,
    action: fn(player: &mut Editor, event_loop: &EventLoopProxy<RuffleEvent>),
}

const MENUS: &[Menu] = &[Menu {
    name: "File",
    items: &[MenuItem {
        name: "Open...",
        keyboard_shortcut: Some(egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND,
            egui::Key::O,
        )),
        action: open_project,
    },MenuItem {
        name: "Save",
        keyboard_shortcut: Some(egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND,
            egui::Key::S,
        )),
        action: save_project,
    },MenuItem {
        name: "Export",
        keyboard_shortcut: Some(egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND,
            egui::Key::E,
        )),
        action: export_swf,
    },MenuItem {
        name: "Close",
        keyboard_shortcut: Some(egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND,
            egui::Key::W,
        )),
        action: close_project,
    },MenuItem {
        name: "Exit",
        keyboard_shortcut: Some(egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND,
            egui::Key::Q,
        )),
        action: request_exit,
    }],
},Menu {
    name: "Edit",
    items: &[MenuItem {
        name: "Delete",
        keyboard_shortcut: Some(egui::KeyboardShortcut::new(
            egui::Modifiers::NONE,
            egui::Key::Delete,
        )),
        action: delete_selection
    }]
},Menu {
    name: "Control",
    items: &[MenuItem {
        name: "Test Movie",
        keyboard_shortcut: Some(egui::KeyboardShortcut::new(
            egui::Modifiers::CTRL,
            egui::Key::Enter,
        )),
        action: run_project
    }]
},Menu {
    name: "Help",
    items: &[MenuItem {
        name: "About...",
        keyboard_shortcut: None,
        action: show_about_screen
    }]
}];

fn open_project(_player: &mut Editor, event_loop: &EventLoopProxy<RuffleEvent>) {
    let _ = event_loop.send_event(RuffleEvent::OpenFile);
}

fn save_project(player: &mut Editor, _event_loop: &EventLoopProxy<RuffleEvent>) {
    player.movie.save(&player.project_file_path);
}

fn export_swf(player: &mut Editor, _event_loop: &EventLoopProxy<RuffleEvent>) {
    player.export_swf();
}
    
fn close_project(_player: &mut Editor, event_loop: &EventLoopProxy<RuffleEvent>) {
    let _ = event_loop.send_event(RuffleEvent::CloseFile);
}

fn request_exit(_player: &mut Editor, event_loop: &EventLoopProxy<RuffleEvent>) {
    let _ = event_loop.send_event(RuffleEvent::ExitRequested);
}

fn run_project(player: &mut Editor, _event_loop: &EventLoopProxy<RuffleEvent>) {
    player.export_swf();
    Movie::run(&player.directory.join("output.swf"));
}

fn show_about_screen(_player: &mut Editor, event_loop: &EventLoopProxy<RuffleEvent>) {
    let _ = event_loop.send_event(RuffleEvent::About);
}

fn delete_selection(player: &mut Editor, _event_loop: &EventLoopProxy<RuffleEvent>) {
    player.delete_selection();
}

impl Editor {
    pub fn new(renderer: Renderer, path: PathBuf) -> Editor {
        let movie = Movie::load(path.clone());
        Editor {
            movie,
            project_file_path: path.clone(),
            directory: PathBuf::from(path.parent().unwrap()),
            renderer,
            
            editing_clip: None,

            selection: vec![],
            drag_data: None,
            
            new_symbol_window: None,
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
                Twips::from_pixels(0.0),
            ),
        });
        
        let symbols = &mut self.movie.symbols;
        let renderer = &mut self.renderer;

        // set bitmap handles for bitmaps
        for i in 0..symbols.len() {
            let symbol = symbols.get_mut(i).unwrap();
            match symbol {
                Symbol::Bitmap(bitmap) => {
                    let image = bitmap.image.as_ref().expect("Image or symbol not loaded");
                    if bitmap.bitmap_handle.is_none() {
                        let bitmap_handle = renderer
                            .register_bitmap(Bitmap::new(
                                image.width(),
                                image.height(),
                                BitmapFormat::Rgba,
                                image
                                    .as_rgba8()
                                    .expect("Unable to convert image to rgba")
                                    .to_vec(),
                            ))
                            .expect("Unable to register bitmap");
                        bitmap.bitmap_handle = Some(bitmap_handle);
                    }
                }
                Symbol::MovieClip(_) => (),
            }
        }

        commands.commands.extend(Editor::render_placed_symbols(
            renderer,
            &self.movie,
            self.editing_clip,
            Transform::default(),
        ));
        self.renderer
            .submit_frame(Color::from_rgb(0x222222, 255), commands, vec![]);
    }

    fn render_placed_symbols(
        renderer: &mut Box<dyn RenderBackend>,
        movie: &Movie,
        symbol_index: Option<usize>,
        transform: Transform,
    ) -> Vec<Command> {
        let mut commands = vec![];
        let placed_symbols = movie.get_placed_symbols(symbol_index);
        for i in 0..placed_symbols.len() {
            let place_symbol = placed_symbols.get(i).unwrap();
            let symbol = movie.symbols
                .get(place_symbol.symbol_id as usize)
                .expect("Invalid symbol placed");
            match symbol {
                Symbol::Bitmap(bitmap) => {
                    let bitmap_handle = bitmap.bitmap_handle.as_ref().unwrap();
                    commands.push(Command::RenderBitmap {
                        bitmap: bitmap_handle.clone(),
                        transform: Transform {
                            matrix: transform.matrix
                                * Matrix::translate(
                                    Twips::from_pixels(place_symbol.x),
                                    Twips::from_pixels(place_symbol.y),
                                ),
                            color_transform: ColorTransform::IDENTITY,
                        },
                        smoothing: false,
                        pixel_snapping: PixelSnapping::Never, // TODO: figure out a good default
                    });
                }
                Symbol::MovieClip(_) => {
                    commands.extend(Editor::render_placed_symbols(
                        renderer,
                        movie,
                        Some(place_symbol.symbol_id as usize),
                        Transform {
                            matrix: transform.matrix
                                * Matrix::translate(
                                    Twips::from_pixels(place_symbol.x),
                                    Twips::from_pixels(place_symbol.y),
                                ),
                            color_transform: transform.color_transform,
                        },
                    ));
                }
            }
        }
        commands
    }

    pub fn handle_mouse_move(&mut self, mouse_x: f64, mouse_y: f64) {
        let placed_symbols = self.movie.get_placed_symbols_mut(self.editing_clip);
        if let Some(drag_data) = &self.drag_data {
            let place_symbol = placed_symbols
                .get_mut(drag_data.place_symbol_index)
                .unwrap();
            place_symbol.x = drag_data.symbol_start_x + mouse_x - drag_data.start_x;
            place_symbol.y = drag_data.symbol_start_y + mouse_y - drag_data.start_y;
        }
    }

    pub fn handle_mouse_input(
        &mut self,
        mouse_x: f64,
        mouse_y: f64,
        button: MouseButton,
        state: ElementState,
    ) {
        let placed_symbols = self.movie.get_placed_symbols(self.editing_clip);
        if button == MouseButton::Left && state == ElementState::Pressed {
            self.selection = vec![];
            // iterate from top to bottom to get the item that's on top
            for i in (0..placed_symbols.len()).rev() {
                let place_symbol = &placed_symbols[i];
                let mut width = 32.0;
                let mut height = 32.0;
                let symbol = self
                    .movie
                    .symbols
                    .get(place_symbol.symbol_id as usize)
                    .expect("Invalid symbol placed");
                // TODO: movieclip size
                if let Symbol::Bitmap(bitmap) = symbol {
                    if let Some(image) = &bitmap.image {
                        width = image.width() as f64;
                        height = image.height() as f64;
                    }
                }
                if mouse_x > place_symbol.x
                    && mouse_y > place_symbol.y
                    && mouse_x < place_symbol.x + width
                    && mouse_y < place_symbol.y + height
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
                let place_symbol = self.movie.get_placed_symbols_mut(self.editing_clip)
                    .get_mut(drag_data.place_symbol_index)
                    .unwrap();
                place_symbol.x = drag_data.symbol_start_x + mouse_x - drag_data.start_x;
                place_symbol.y = drag_data.symbol_start_y + mouse_y - drag_data.start_y;
                self.drag_data = None;
            }
        }
    }

    pub fn do_ui(
        &mut self,
        egui_ctx: &egui::Context,
        event_loop: &EventLoopProxy<RuffleEvent>,
    ) -> bool {
        let mut has_mutated = false;
        egui::TopBottomPanel::top("menu_bar").show(egui_ctx, |ui| {
            for menu in MENUS {
                for item in menu.items {
                    if let Some(keyboard_shortcut) = item.keyboard_shortcut {
                        if ui
                            .ctx()
                            .input_mut(|input| input.consume_shortcut(&keyboard_shortcut))
                        {
                            (item.action)(self, event_loop);
                            ui.close_menu();
                        }
                    }
                }
            }
            egui::menu::bar(ui, |ui| {
                for menu in MENUS {
                    egui::menu::menu_button(ui, menu.name, |ui| {
                        for item in menu.items {
                            let mut button = egui::Button::new(item.name);
                            if let Some(keyboard_shortcut) = item.keyboard_shortcut {
                                button =
                                    button.shortcut_text(ui.ctx().format_shortcut(&keyboard_shortcut));
                            }
                            if button.ui(ui).clicked() {
                                (item.action)(self, event_loop);
                                ui.close_menu();
                            }
                        }
                    });
                }
            });
        });
        egui::SidePanel::right("library")
            .resizable(false) // resizing causes glitches
            .min_width(150.0)
            .show(egui_ctx, |ui| {
                ui.heading("Library");
                if ui.button("Add MovieClip...").clicked() {
                    self.new_symbol_window = Some(NewSymbolWindow::default());
                }
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for i in 0..self.movie.symbols.len() {
                        let symbol = self.movie.symbols.get(i).unwrap();
                        let checked = if let Some(editing_clip) = self.editing_clip {editing_clip == i} else {false};
                        let response = ui.selectable_label(checked, symbol.name());
                        let response = response.interact(egui::Sense::drag());
                        
                        if response.clicked() {
                            self.change_editing_clip(Some(i));
                            has_mutated = true;
                        } else if response.drag_released() {  // TODO: handle drag that doesn't end on stage
                            let mouse_pos = response.interact_pointer_pos().unwrap();
                            // TODO: don't hardcode the menu height
                            self.movie.get_placed_symbols_mut(self.editing_clip).push(PlaceSymbol {
                                symbol_id: i as u16,
                                x: mouse_pos.x as f64,
                                y: mouse_pos.y as f64 - MENU_HEIGHT as f64,
                            });
                            has_mutated = true; 
                        }
                    }
                });
            });
        egui::TopBottomPanel::top("breadcrumb_bar").show(egui_ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(editing_clip) = self.editing_clip {
                    if ui.selectable_label(false, "Scene").clicked() {
                        self.change_editing_clip(None);
                        has_mutated = true;
                    }
                    let _ = ui.selectable_label(true, self.movie.symbols[editing_clip].name());
                } else {
                    let _ = ui.selectable_label(true, "Scene");
                }
            });
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
        
        if let Some(new_symbol_window) = &mut self.new_symbol_window {
            if let Some(symbol) = new_symbol_window.do_ui(egui_ctx) {
                self.movie.symbols.push(symbol);
                self.new_symbol_window = None;
            }
        }

        has_mutated
    }
    
    fn change_editing_clip(&mut self, symbol_index: Option<usize>) {
        if let Some(symbol_index) = symbol_index {
            let Symbol::MovieClip(_) = self.movie.symbols[symbol_index] else {
                // only select movieclips
                return;
            };   
        }
        self.selection = vec![];
        self.editing_clip = symbol_index;
    }

    fn delete_selection(&mut self) {
        // because the list is sorted and we are traversing from the end to the beginning
        // we can safely remove placed items without changing the indices of the rest of the selection
        self.selection.sort();
        for i in (0..self.selection.len()).rev() {
            let placed_symbol_index = *self.selection.get(i).unwrap();
            self.movie.get_placed_symbols_mut(self.editing_clip).remove(placed_symbol_index);
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
        let placed_symbol = self.movie.get_placed_symbols_mut(self.editing_clip).get_mut(placed_symbol_index).unwrap();
        egui::Grid::new(format!(
            "placed_symbol_{placed_symbol_index}_properties_grid"
        ))
        .show(ui, |ui| {
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
        self.movie.export(directory, swf_path);
    }
}

#[derive(Default)]
struct NewSymbolWindow {
    name: String,
}
impl NewSymbolWindow {
    pub fn do_ui(
        &mut self,
        egui_ctx: &egui::Context,
    ) -> Option<Symbol> {
        let mut result = None;
        // title says new movieclip because there are no other options yet
        egui::Window::new("New movieclip").resizable(false).show(egui_ctx, |ui| {
            egui::Grid::new("symbol_properties_grid").show(ui, |ui| {                        
                        ui.label("Name:");
                        ui.add(egui::TextEdit::singleline(&mut self.name).min_size(Vec2::new(200.0, 0.0)));
                        ui.end_row();

                        if ui.add_enabled(!self.name.is_empty(), egui::Button::new("Create")).clicked() {
                            result = Some(Symbol::MovieClip(MovieClip {
                                name: self.name.clone(),
                                class_name: "".to_string(),
                                place_symbols: vec![],
                            }));
                        }
                        ui.end_row();
                    });
        });
        result
    }
}   