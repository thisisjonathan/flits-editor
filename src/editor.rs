use std::path::PathBuf;

use self::edit::{
    AddPlacedSymbolEdit, MovePlacedSymbolEdit, MovieEdit, MoviePropertiesOutput,
    RemovePlacedSymbolEdit,
};
use self::menu::MENUS;
use self::properties_panel::{
    MoviePropertiesPanel, MultiSelectionPropertiesPanel, PlacedSymbolPropertiesPanel,
    PropertiesPanel, SymbolProperties, SymbolPropertiesPanel,
};
use crate::core::{
    BitmapCacheStatus, CachedBitmap, Movie, MovieClip, MovieClipProperties, MovieProperties,
    PlaceSymbol, PlacedSymbolIndex, Symbol, SymbolIndex, SymbolIndexOrRoot,
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
const LIBRARY_WIDTH: u32 = 150;
pub const EDIT_EPSILON: f64 = 0.00001;

type Renderer = Box<dyn RenderBackend>;

struct DragData {
    symbol_start_matrix: Matrix,
    start_x: f64,
    start_y: f64,
    place_symbol_index: SymbolIndex,
}
struct CameraDragData {
    previous_x: f64,
    previous_y: f64,
}

pub struct Editor {
    movie: Movie,
    project_file_path: PathBuf,
    directory: PathBuf,
    renderer: Renderer,

    camera: Matrix, // center of the screen
    camera_drag_data: Option<CameraDragData>,

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
        let movie_properties = movie.properties.clone();
        Editor {
            movie,
            project_file_path: path.clone(),
            directory: PathBuf::from(path.parent().unwrap()),
            renderer,

            camera: Self::center_stage_camera_matrix(movie_properties.clone()),
            camera_drag_data: None,

            history: Record::new(),

            editing_clip: None,

            selection: vec![],
            drag_data: None,
            properties_panel: PropertiesPanel::MovieProperties(MoviePropertiesPanel {
                before_edit: movie_properties,
            }),

            new_symbol_window: None,
        }
    }

    fn center_stage_camera_matrix(movie_properties: MovieProperties) -> Matrix {
        Matrix::translate(
            Twips::from_pixels(movie_properties.width / -2.0),
            Twips::from_pixels(movie_properties.height / -2.0),
        )
    }

    #[instrument(level = "debug", skip_all)]
    pub fn render(&mut self) {
        let symbols = &mut self.movie.symbols;
        let renderer = &mut self.renderer;

        let viewport_dimensions = renderer.viewport_dimensions();

        let mut commands = CommandList::new();

        // stage background
        let mut stage_color = Color::from_rgba(0xFFFFFFFF);
        if self.editing_clip != None {
            // when editing a clip, fade the stage background
            stage_color.a = 4;
        }
        commands.commands.push(Command::DrawRect {
            color: stage_color,
            matrix: self.camera
                * Self::camera_to_pixel_matrix(viewport_dimensions)
                * Matrix::create_box(
                    self.movie.properties.width as f32,
                    self.movie.properties.height as f32,
                    0.0,
                    Twips::ZERO,
                    Twips::ZERO,
                ),
        });

        if self.editing_clip != None {
            // when editing a movieclip
            // draw a cross to indicate the origin
            const CROSS_COLOR: Color = Color::from_rgba(0xFF888888);
            const CROSS_SIZE: f32 = 32.0;
            // horizontal
            commands.commands.push(Command::DrawRect {
                color: CROSS_COLOR,
                matrix: self.camera
                    * Self::camera_to_pixel_matrix(viewport_dimensions)
                    * Matrix::create_box(
                        CROSS_SIZE,
                        1.0,
                        0.0,
                        Twips::from_pixels(CROSS_SIZE as f64 / -2.0),
                        Twips::ZERO,
                    ),
            });
            // vertical
            commands.commands.push(Command::DrawRect {
                color: CROSS_COLOR,
                matrix: self.camera
                    * Self::camera_to_pixel_matrix(viewport_dimensions)
                    * Matrix::create_box(
                        1.0,
                        CROSS_SIZE,
                        0.0,
                        Twips::ZERO,
                        Twips::from_pixels(CROSS_SIZE as f64 / -2.0),
                    ),
            });
        }

        // set bitmap handles for bitmaps
        for i in 0..symbols.len() {
            let symbol = symbols.get_mut(i).unwrap();
            match symbol {
                Symbol::Bitmap(bitmap) => match &mut bitmap.cache {
                    BitmapCacheStatus::Uncached => {
                        bitmap.cache_image(&self.directory);
                        // if the caching is succesful
                        if let BitmapCacheStatus::Cached(cached_bitmap) = &mut bitmap.cache {
                            Self::cache_bitmap_handle(renderer, cached_bitmap);
                        }
                    }
                    BitmapCacheStatus::Cached(cached_bitmap) => {
                        if cached_bitmap.bitmap_handle.is_none() {
                            Self::cache_bitmap_handle(renderer, cached_bitmap);
                        }
                    }
                    BitmapCacheStatus::Invalid(_) => (),
                },
                Symbol::MovieClip(_) => (),
            }
        }

        commands.commands.extend(Editor::render_placed_symbols(
            renderer,
            &self.movie,
            self.editing_clip,
            Transform {
                matrix: self.camera * Self::camera_to_pixel_matrix(viewport_dimensions),
                color_transform: ColorTransform::IDENTITY,
            },
        ));
        self.renderer
            .submit_frame(Color::from_rgb(0x222222, 255), commands, vec![]);
    }

    fn camera_to_pixel_matrix(viewport_dimensions: ViewportDimensions) -> Matrix {
        Matrix::translate(
            Twips::from_pixels((viewport_dimensions.width - LIBRARY_WIDTH) as f64 / 2.0),
            // we don't know the height of the properties panel, so just use an approximation
            Twips::from_pixels((viewport_dimensions.height - 75) as f64 / 2.0),
        )
    }

    fn cache_bitmap_handle(renderer: &mut Renderer, cached_bitmap: &mut CachedBitmap) {
        cached_bitmap.bitmap_handle = Some(
            renderer
                .register_bitmap(Bitmap::new(
                    cached_bitmap.image.width(),
                    cached_bitmap.image.height(),
                    BitmapFormat::Rgba,
                    cached_bitmap
                        .image
                        .as_rgba8()
                        .expect("Unable to convert image to rgba")
                        .to_vec(),
                ))
                .expect("Unable to register bitmap"),
        );
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
                    let BitmapCacheStatus::Cached(cached_bitmap) = &bitmap.cache else {
                        break;
                    };
                    let Some(bitmap_handle) = &cached_bitmap.bitmap_handle else {
                        break;
                    };
                    commands.push(Command::RenderBitmap {
                        bitmap: bitmap_handle.clone(),
                        transform: Transform {
                            matrix: transform.matrix * place_symbol.transform.matrix,
                            color_transform: transform.color_transform
                                * place_symbol.transform.color_transform,
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
                            matrix: transform.matrix * place_symbol.transform.matrix,
                            color_transform: transform.color_transform
                                * place_symbol.transform.color_transform,
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
            place_symbol.transform.matrix = drag_data.symbol_start_matrix
                * Matrix::translate(
                    Twips::from_pixels(mouse_x - drag_data.start_x),
                    Twips::from_pixels(mouse_y - drag_data.start_y),
                );
        }

        if let Some(camera_drag_data) = &self.camera_drag_data {
            self.camera *= Matrix::translate(
                Twips::from_pixels(mouse_x - camera_drag_data.previous_x),
                Twips::from_pixels(mouse_y - camera_drag_data.previous_y),
            );
            self.camera_drag_data = Some(CameraDragData {
                previous_x: mouse_x,
                previous_y: mouse_y,
            });
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
                    symbol_start_matrix: place_symbol.transform.matrix.clone(),
                    start_x: mouse_x,
                    start_y: mouse_y,
                    place_symbol_index: symbol_index,
                });
                self.set_selection(vec![symbol_index]);
            }
        }
        if button == MouseButton::Left && state == ElementState::Released {
            if let Some(drag_data) = &self.drag_data {
                let end = Matrix::translate(
                    Twips::from_pixels(
                        drag_data.symbol_start_matrix.tx.to_pixels() + mouse_x - drag_data.start_x,
                    ),
                    Twips::from_pixels(
                        drag_data.symbol_start_matrix.ty.to_pixels() + mouse_y - drag_data.start_y,
                    ),
                );
                // only insert an edit if you actually moved the placed symbol
                if f64::abs(drag_data.symbol_start_matrix.tx.to_pixels() - end.tx.to_pixels())
                    > EDIT_EPSILON
                    || f64::abs(drag_data.symbol_start_matrix.ty.to_pixels() - end.ty.to_pixels())
                        > EDIT_EPSILON
                {
                    self.do_edit(MovieEdit::MovePlacedSymbol(MovePlacedSymbolEdit {
                        editing_symbol_index: self.editing_clip,
                        placed_symbol_index: drag_data.place_symbol_index,
                        start: drag_data.symbol_start_matrix,
                        end,
                    }));
                }
                self.drag_data = None;
            }
        }
        if button == MouseButton::Middle && state == ElementState::Pressed {
            self.camera_drag_data = Some(CameraDragData {
                previous_x: mouse_x,
                previous_y: mouse_y,
            });
        }
        if button == MouseButton::Middle && state == ElementState::Released {
            self.camera_drag_data = None;
        }
    }

    fn do_edit(&mut self, edit: MovieEdit) {
        let result = self.history.edit(&mut self.movie, edit);
        self.change_view_after_edit(result);
    }

    fn do_undo(&mut self) {
        let result = self.history.undo(&mut self.movie);
        if let Some(editing_clip) = result {
            self.change_view_after_edit(editing_clip);
        }
    }

    fn do_redo(&mut self) {
        let result = self.history.redo(&mut self.movie);
        if let Some(editing_clip) = result {
            self.change_view_after_edit(editing_clip);
        }
    }

    fn change_view_after_edit(&mut self, output: MoviePropertiesOutput) {
        match output {
            MoviePropertiesOutput::Stage(editing_clip) => {
                self.change_editing_clip_without_resetting_selection(editing_clip);
                if self.selection.len() == 1 {
                    self.properties_panel =
                        PropertiesPanel::PlacedSymbolProperties(PlacedSymbolPropertiesPanel {
                            before_edit: self.movie.get_placed_symbols(self.editing_clip)
                                [self.selection[0]]
                                .clone(),
                        });
                }
            }
            MoviePropertiesOutput::Properties(editing_clip) => {
                if let Some(symbol_index) = editing_clip {
                    self.properties_panel = Self::create_symbol_propeties_panel(
                        symbol_index,
                        &self.movie.symbols[symbol_index],
                    );
                } else {
                    // root
                    self.properties_panel =
                        PropertiesPanel::MovieProperties(MoviePropertiesPanel {
                            before_edit: self.movie.properties.clone(),
                        });
                }
            }
        }
    }

    fn get_placed_symbol_at_position(
        &self,
        x: f64,
        y: f64,
        symbol_index: SymbolIndexOrRoot,
    ) -> SymbolIndexOrRoot {
        let world_space_matrix = Matrix::translate(Twips::from_pixels(x), Twips::from_pixels(y))
            * (self.camera * Self::camera_to_pixel_matrix(self.renderer.viewport_dimensions()))
                .inverse()
                .unwrap_or(Matrix::IDENTITY);

        self.get_placed_symbol_at_position_world_space(
            world_space_matrix.tx.to_pixels(),
            world_space_matrix.ty.to_pixels(),
            symbol_index,
        )
    }
    fn get_placed_symbol_at_position_world_space(
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
            let place_symbol_x = place_symbol.transform.matrix.tx.to_pixels();
            let place_symbol_y = place_symbol.transform.matrix.ty.to_pixels();
            match symbol {
                Symbol::Bitmap(bitmap) => {
                    if let BitmapCacheStatus::Cached(cached_bitmap) = &bitmap.cache {
                        let width = cached_bitmap.image.width() as f64;
                        let height = cached_bitmap.image.height() as f64;
                        if x > place_symbol_x
                            && y > place_symbol_y
                            && x < place_symbol_x + width
                            && y < place_symbol_y + height
                        {
                            return Some(i);
                        }
                    }
                }
                Symbol::MovieClip(_) => {
                    if let Some(_) = self.get_placed_symbol_at_position_world_space(
                        x - place_symbol_x,
                        y - place_symbol_y,
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
            .min_width(LIBRARY_WIDTH as f32)
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
                            let checked = match &self.properties_panel {
                                PropertiesPanel::SymbolProperties(panel) => panel.symbol_index == i,
                                _ => false,
                            };
                            let mut text = egui::RichText::new(symbol.name());
                            if symbol.is_invalid() {
                                text = text.color(ui.style().visuals.error_fg_color);
                            }
                            let response = ui.selectable_label(checked, text);
                            let response = response.interact(egui::Sense::drag());

                            if response.double_clicked() {
                                self.change_editing_clip(Some(i));
                                has_mutated = true;
                            } else if response.clicked() {
                                self.properties_panel =
                                    Self::create_symbol_propeties_panel(i, symbol);
                                has_mutated = true;
                            } else if response.drag_released() {
                                // TODO: handle drag that doesn't end on stage
                                let mouse_pos = response.interact_pointer_pos().unwrap();
                                self.do_edit(MovieEdit::AddPlacedSymbol(AddPlacedSymbolEdit {
                                    editing_symbol_index: self.editing_clip,
                                    placed_symbol: PlaceSymbol {
                                        symbol_index: i,
                                        transform: Transform {
                                            matrix: Matrix::translate(
                                                Twips::from_pixels(mouse_pos.x as f64),
                                                Twips::from_pixels(
                                                    // TODO: don't hardcode the menu height
                                                    mouse_pos.y as f64 - MENU_HEIGHT as f64,
                                                ),
                                            ),
                                            color_transform: ColorTransform::IDENTITY,
                                        },
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
                PropertiesPanel::SymbolProperties(panel) => panel.do_ui(&mut self.movie, ui),
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
                has_mutated = true; // some edits cause cascading effects (for example changing the path of a bitmap)
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

    fn create_symbol_propeties_panel(
        symbol_index: SymbolIndex,
        symbol: &Symbol,
    ) -> PropertiesPanel {
        match symbol {
            Symbol::Bitmap(bitmap) => PropertiesPanel::SymbolProperties(SymbolPropertiesPanel {
                symbol_index,
                before_edit: SymbolProperties::Bitmap(bitmap.properties.clone()),
            }),
            Symbol::MovieClip(movieclip) => {
                PropertiesPanel::SymbolProperties(SymbolPropertiesPanel {
                    symbol_index,
                    before_edit: SymbolProperties::MovieClip(movieclip.properties.clone()),
                })
            }
        }
    }

    fn change_editing_clip(&mut self, symbol_index: SymbolIndexOrRoot) {
        self.change_editing_clip_without_resetting_selection(symbol_index);
        self.set_selection(vec![]);
    }
    fn change_editing_clip_without_resetting_selection(&mut self, symbol_index: SymbolIndexOrRoot) {
        // if switching to the same symbol, do nothing
        if symbol_index == self.editing_clip {
            return;
        }
        
        if let Some(symbol_index) = symbol_index {
            let Symbol::MovieClip(_) = self.movie.symbols[symbol_index] else {
                // only select movieclips
                return;
            };
            // center the camera on the origin when you open a movieclip
            self.camera = Matrix::IDENTITY;
        } else {
            // center the camera on the stage when you open root
            self.camera = Self::center_stage_camera_matrix(self.movie.properties.clone());
        }

        self.editing_clip = symbol_index;
    }

    fn set_selection(&mut self, selection: Vec<PlacedSymbolIndex>) {
        self.selection = selection;
        self.update_selection();
    }
    fn update_selection(&mut self) {
        match self.selection.len() {
            0 => {
                if let Some(editing_clip) = self.editing_clip {
                    self.properties_panel =
                        PropertiesPanel::SymbolProperties(SymbolPropertiesPanel {
                            symbol_index: editing_clip,
                            before_edit: match &self.movie.symbols[editing_clip] {
                                Symbol::Bitmap(bitmap) => {
                                    SymbolProperties::Bitmap(bitmap.properties.clone())
                                }
                                Symbol::MovieClip(movieclip) => {
                                    SymbolProperties::MovieClip(movieclip.properties.clone())
                                }
                            },
                        });
                } else {
                    self.properties_panel =
                        PropertiesPanel::MovieProperties(MoviePropertiesPanel {
                            before_edit: self.movie.properties.clone(),
                        });
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
        let mut selection = self.selection.clone();

        // because the list is sorted and we are traversing from the end to the beginning
        // we can safely remove placed items without changing the indices of the rest of the selection
        selection.sort();

        // reset selection before doing edits because otherwise you can delete something while it's still selected
        self.set_selection(vec![]);
        for i in (0..selection.len()).rev() {
            let placed_symbol_index = *selection.get(i).unwrap();
            self.do_edit(MovieEdit::RemovePlacedSymbol(RemovePlacedSymbolEdit {
                editing_symbol_index: self.editing_clip,
                placed_symbol_index,
                placed_symbol: self.movie.get_placed_symbols(self.editing_clip)
                    [placed_symbol_index]
                    .clone(),
            }));
        }
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
                            properties: MovieClipProperties {
                                name: self.name.clone(),
                                class_name: "".to_string(),
                            },
                            place_symbols: vec![],
                        }));
                    }
                    ui.end_row();
                });
            });
        result
    }
}
