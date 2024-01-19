use std::path::PathBuf;

use self::edit::{AddPlacedSymbolEdit, MovePlacedSymbolEdit, MovieEdit, RemovePlacedSymbolEdit};
use self::menu::MENUS;
use self::properties_panel::{
    MoviePropertiesPanel, MultiSelectionPropertiesPanel, PlacedSymbolPropertiesPanel,
    PropertiesPanel, SymbolPropertiesPanel,
};
use crate::core::{
    Movie, MovieClip, PlaceSymbol, PlacedSymbolIndex, Symbol, SymbolIndex, SymbolIndexOrRoot,
};
use crate::desktop::custom_event::RuffleEvent;
use egui::{Vec2, Widget};
use ruffle_render::{
    backend::{RenderBackend, ViewportDimensions},
    bitmap::{Bitmap, BitmapFormat, PixelSnapping},
    commands::{Command, CommandList},
    matrix::Matrix,
    transform::Transform,
};
use swf::{Color, ColorTransform, Twips};
use tracing::instrument;
use undo::Record;
use winit::{
    event::{ElementState, MouseButton},
    event_loop::EventLoopProxy,
};

mod edit;
mod menu;
mod properties_panel;

pub const MENU_HEIGHT: u32 = 48; // also defined in desktop/gui.rs
pub const EDIT_EPSILON: f64 = 0.00001;

type Renderer = Box<dyn RenderBackend>;

struct DragData {
    symbol_start_x: f64,
    symbol_start_y: f64,
    start_x: f64,
    start_y: f64,
    place_symbol_index: SymbolIndex,
}

pub struct Editor {
    movie: Movie,
    project_file_path: PathBuf,
    directory: PathBuf,
    renderer: Renderer,

    history: Record<MovieEdit>,

    editing_clip: SymbolIndexOrRoot,

    selection: Vec<SymbolIndex>,
    drag_data: Option<DragData>,
    properties_panel: PropertiesPanel,

    new_symbol_window: Option<NewSymbolWindow>,
}

impl Editor {
    pub fn new(renderer: Renderer, path: PathBuf) -> Editor {
        let movie = Movie::load(path.clone());
        Editor {
            movie,
            project_file_path: path.clone(),
            directory: PathBuf::from(path.parent().unwrap()),
            renderer,

            history: Record::new(),

            editing_clip: None,

            selection: vec![],
            drag_data: None,
            properties_panel: PropertiesPanel::MovieProperties(MoviePropertiesPanel {}),

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
        symbol_index: SymbolIndexOrRoot,
        transform: Transform,
    ) -> Vec<Command> {
        let mut commands = vec![];
        let placed_symbols = movie.get_placed_symbols(symbol_index);
        for i in 0..placed_symbols.len() {
            let place_symbol = placed_symbols.get(i).unwrap();
            let symbol = movie
                .symbols
                .get(place_symbol.symbol_index as usize)
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
                        Some(place_symbol.symbol_index as usize),
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
        if button == MouseButton::Left && state == ElementState::Pressed {
            self.set_selection(vec![]);
            let symbol_index =
                self.get_placed_symbol_at_position(mouse_x, mouse_y, self.editing_clip);
            if let Some(symbol_index) = symbol_index {
                let place_symbol = &self.movie.get_placed_symbols(self.editing_clip)[symbol_index];
                self.drag_data = Some(DragData {
                    symbol_start_x: place_symbol.x,
                    symbol_start_y: place_symbol.y,
                    start_x: mouse_x,
                    start_y: mouse_y,
                    place_symbol_index: symbol_index,
                });
                self.set_selection(vec![symbol_index]);
            }
        }
        if button == MouseButton::Left && state == ElementState::Released {
            if let Some(drag_data) = &self.drag_data {
                let end_x = drag_data.symbol_start_x + mouse_x - drag_data.start_x;
                let end_y = drag_data.symbol_start_y + mouse_y - drag_data.start_y;
                // only insert an edit if you actually moved the placed symbol
                if f64::abs(drag_data.symbol_start_x - end_x) > EDIT_EPSILON
                    || f64::abs(drag_data.symbol_start_y - end_y) > EDIT_EPSILON
                {
                    self.do_edit(MovieEdit::MovePlacedSymbol(MovePlacedSymbolEdit {
                        editing_symbol_index: self.editing_clip,
                        placed_symbol_index: drag_data.place_symbol_index,
                        start_x: drag_data.symbol_start_x,
                        start_y: drag_data.symbol_start_y,
                        end_x,
                        end_y,
                    }));
                }
                self.drag_data = None;
            }
        }
    }

    fn do_edit(&mut self, edit: MovieEdit) {
        self.editing_clip = self.history.edit(&mut self.movie, edit);
        if self.selection.len() == 1 {
            self.properties_panel =
                PropertiesPanel::PlacedSymbolProperties(PlacedSymbolPropertiesPanel {
                    before_edit: self.movie.get_placed_symbols(self.editing_clip)
                        [self.selection[0]]
                        .clone(),
                });
        }
    }

    fn do_undo(&mut self) {
        let result = self.history.undo(&mut self.movie);
        if let Some(editing_clip) = result {
            self.editing_clip = editing_clip;
        } else if self.selection.len() == 1 {
            self.properties_panel =
                PropertiesPanel::PlacedSymbolProperties(PlacedSymbolPropertiesPanel {
                    before_edit: self.movie.get_placed_symbols(self.editing_clip)
                        [self.selection[0]]
                        .clone(),
                });
        }
    }

    fn do_redo(&mut self) {
        let result = self.history.redo(&mut self.movie);
        if let Some(editing_clip) = result {
            self.editing_clip = editing_clip;
        } else if self.selection.len() == 1 {
            self.properties_panel =
                PropertiesPanel::PlacedSymbolProperties(PlacedSymbolPropertiesPanel {
                    before_edit: self.movie.get_placed_symbols(self.editing_clip)
                        [self.selection[0]]
                        .clone(),
                });
        }
    }

    fn get_placed_symbol_at_position(
        &self,
        x: f64,
        y: f64,
        symbol_index: SymbolIndexOrRoot,
    ) -> SymbolIndexOrRoot {
        let placed_symbols = self.movie.get_placed_symbols(symbol_index);
        // iterate from top to bottom to get the item that's on top
        for i in (0..placed_symbols.len()).rev() {
            let place_symbol = &placed_symbols[i];
            let symbol = self
                .movie
                .symbols
                .get(place_symbol.symbol_index as usize)
                .expect("Invalid symbol placed");
            match symbol {
                Symbol::Bitmap(bitmap) => {
                    if let Some(image) = &bitmap.image {
                        let width = image.width() as f64;
                        let height = image.height() as f64;
                        if x > place_symbol.x
                            && y > place_symbol.y
                            && x < place_symbol.x + width
                            && y < place_symbol.y + height
                        {
                            return Some(i);
                        }
                    }
                }
                Symbol::MovieClip(_) => {
                    if let Some(_) = self.get_placed_symbol_at_position(
                        x - place_symbol.x,
                        y - place_symbol.y,
                        Some(place_symbol.symbol_index as usize),
                    ) {
                        return Some(i);
                    }
                }
            }
        }
        None
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
                            has_mutated = true;
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
                                button = button
                                    .shortcut_text(ui.ctx().format_shortcut(&keyboard_shortcut));
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
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for i in 0..self.movie.symbols.len() {
                            let symbol = self.movie.symbols.get(i).unwrap();
                            let checked = if let Some(editing_clip) = self.editing_clip {
                                editing_clip == i
                            } else {
                                false
                            };
                            let response = ui.selectable_label(checked, symbol.name());
                            let response = response.interact(egui::Sense::drag());

                            if response.clicked() {
                                self.change_editing_clip(Some(i));
                                has_mutated = true;
                            } else if response.drag_released() {
                                // TODO: handle drag that doesn't end on stage
                                let mouse_pos = response.interact_pointer_pos().unwrap();
                                self.do_edit(MovieEdit::AddPlacedSymbol(AddPlacedSymbolEdit {
                                    editing_symbol_index: self.editing_clip,
                                    placed_symbol: PlaceSymbol {
                                        symbol_index: i,
                                        x: mouse_pos.x as f64,
                                        // TODO: don't hardcode the menu height
                                        y: mouse_pos.y as f64 - MENU_HEIGHT as f64,
                                    },
                                    placed_symbol_index: None,
                                }));
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
            let edit = match &mut self.properties_panel {
                PropertiesPanel::MovieProperties(panel) => panel.do_ui(&mut self.movie, ui),
                PropertiesPanel::SymbolProperties(panel) => panel.do_ui(
                    &mut self.movie,
                    ui,
                    self.editing_clip
                        .expect("Showing symbol properties while no symbol is selected"),
                ),
                PropertiesPanel::PlacedSymbolProperties(panel) => {
                    if self.selection.len() != 1 {
                        panic!(
                            "Showing placed symbol properties while selection has length {}",
                            self.selection.len()
                        );
                    }
                    panel.do_ui(
                        &mut self.movie,
                        ui,
                        self.editing_clip,
                        *self.selection.get(0).unwrap(),
                    )
                }
                PropertiesPanel::MultiSelectionProperties(panel) => panel.do_ui(ui),
            };
            if let Some(edit) = edit {
                self.do_edit(edit);
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

    fn change_editing_clip(&mut self, symbol_index: SymbolIndexOrRoot) {
        if let Some(symbol_index) = symbol_index {
            let Symbol::MovieClip(_) = self.movie.symbols[symbol_index] else {
                // only select movieclips
                return;
            };
        }
        self.editing_clip = symbol_index;
        self.set_selection(vec![]);
    }

    fn set_selection(&mut self, selection: Vec<PlacedSymbolIndex>) {
        self.selection = selection;
        self.update_selection();
    }
    fn update_selection(&mut self) {
        match self.selection.len() {
            0 => {
                if self.editing_clip.is_some() {
                    self.properties_panel =
                        PropertiesPanel::SymbolProperties(SymbolPropertiesPanel {});
                } else {
                    self.properties_panel =
                        PropertiesPanel::MovieProperties(MoviePropertiesPanel {});
                }
            }
            1 => {
                let placed_symbol_index = self.selection[0];
                let place_symbol =
                    &self.movie.get_placed_symbols(self.editing_clip)[placed_symbol_index];
                self.properties_panel =
                    PropertiesPanel::PlacedSymbolProperties(PlacedSymbolPropertiesPanel {
                        before_edit: place_symbol.clone(),
                    });
            }
            _ => {
                self.properties_panel =
                    PropertiesPanel::MultiSelectionProperties(MultiSelectionPropertiesPanel {});
            }
        }
    }

    fn delete_selection(&mut self) {
        // because the list is sorted and we are traversing from the end to the beginning
        // we can safely remove placed items without changing the indices of the rest of the selection
        self.selection.sort();
        for i in (0..self.selection.len()).rev() {
            let placed_symbol_index = *self.selection.get(i).unwrap();
            self.do_edit(MovieEdit::RemovePlacedSymbol(RemovePlacedSymbolEdit {
                editing_symbol_index: self.editing_clip,
                placed_symbol_index,
                placed_symbol: self.movie.get_placed_symbols(self.editing_clip)
                    [placed_symbol_index]
                    .clone(),
            }));
        }
        self.set_selection(vec![]);
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
    pub fn do_ui(&mut self, egui_ctx: &egui::Context) -> Option<Symbol> {
        let mut result = None;
        // title says new movieclip because there are no other options yet
        egui::Window::new("New movieclip")
            .resizable(false)
            .show(egui_ctx, |ui| {
                egui::Grid::new("symbol_properties_grid").show(ui, |ui| {
                    ui.label("Name:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.name).min_size(Vec2::new(200.0, 0.0)),
                    );
                    ui.end_row();

                    if ui
                        .add_enabled(!self.name.is_empty(), egui::Button::new("Create"))
                        .clicked()
                    {
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
