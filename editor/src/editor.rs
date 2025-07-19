use std::any::Any;
use std::path::PathBuf;

use crate::camera::Camera;
use crate::custom_event::FlitsEvent;
use crate::edit::{
    AddPlacedSymbolEdit, MovieEdit, MoviePropertiesOutput, PlacedSymbolEdit, RemovePlacedSymbolEdit,
};
use crate::menu::MENUS;
use crate::new_symbol_window::NewSymbolWindow;
use crate::properties_panel::{
    MoviePropertiesPanel, MultiSelectionPropertiesPanel, PlacedSymbolPropertiesPanel,
    PropertiesPanel, SymbolProperties, SymbolPropertiesPanel,
};
use crate::run_ui::RunUi;
use egui::Widget;
use flits_core::run::run_movie;
use flits_core::{
    BitmapCacheStatus, CachedBitmap, EditorColor, EditorTransform, Movie, PlaceSymbol,
    PlacedSymbolIndex, Symbol, SymbolIndex, SymbolIndexOrRoot, TextAlign, TextProperties,
};
use ruffle_render::bitmap::BitmapHandle;
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

pub const MENU_HEIGHT: u32 = 44;
const LIBRARY_WIDTH: u32 = 150;
pub const EDIT_EPSILON: f64 = 0.00001;
const EMPTY_CLIP_WIDTH: f64 = 16.0;
const EMPTY_CLIP_HEIGHT: f64 = 16.0;

type Renderer = Box<dyn RenderBackend>;
struct BitmapHandleWrapper(ruffle_render::bitmap::BitmapHandle);
impl flits_core::BitmapHandle for BitmapHandleWrapper {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub enum NeedsRedraw {
    Yes,
    No,
}

struct DragData {
    symbol_start_transform: EditorTransform,
    start_x: f64,
    start_y: f64,
    place_symbol_index: SymbolIndex,
}

pub struct StageSize {
    pub width: u32,
    pub height: u32,
}

pub struct Editor {
    pub(crate) movie: Movie,
    pub(crate) project_file_path: PathBuf,
    directory: PathBuf,

    camera: Camera,
    viewport_dimensions: ViewportDimensions,

    history: Record<MovieEdit>,

    editing_clip: SymbolIndexOrRoot,

    selection: Vec<SymbolIndex>,
    drag_data: Option<DragData>,
    properties_panel: PropertiesPanel,

    new_symbol_window: Option<NewSymbolWindow>,
    export_error: Option<String>,

    run_ui: Option<RunUi>,
    event_loop: EventLoopProxy<FlitsEvent>,
}

impl Editor {
    pub fn new(
        path: PathBuf,
        viewport_dimensions: ViewportDimensions,
        event_loop: EventLoopProxy<FlitsEvent>,
    ) -> Result<Editor, Box<dyn std::error::Error>> {
        let path_is_directory = path.is_dir();
        let project_file_path = if path_is_directory {
            path.join("movie.json")
        } else {
            path.clone()
        };
        let directory = if path_is_directory {
            path
        } else {
            PathBuf::from(project_file_path.parent().unwrap())
        };

        let movie = Movie::load(project_file_path.clone())?;
        let movie_properties = movie.properties.clone();
        Ok(Editor {
            movie,
            project_file_path,
            directory,

            camera: Camera::new_center_stage(&movie_properties),
            viewport_dimensions,

            history: Record::new(),

            editing_clip: None,

            selection: vec![],
            drag_data: None,
            properties_panel: PropertiesPanel::MovieProperties(MoviePropertiesPanel {
                before_edit: movie_properties.clone(),
            }),

            new_symbol_window: None,
            export_error: None,

            run_ui: None,
            event_loop,
        })
    }

    fn stage_size(&self) -> StageSize {
        Self::stage_size_from_viewport_dimensions(self.viewport_dimensions)
    }

    fn stage_size_from_viewport_dimensions(viewport_dimensions: ViewportDimensions) -> StageSize {
        StageSize {
            width: viewport_dimensions.width - LIBRARY_WIDTH,
            // we don't know the height of the properties panel, so just use an approximation
            height: viewport_dimensions.height - 65,
        }
    }

    #[instrument(level = "debug", skip_all)]
    pub fn render(&mut self, renderer: &mut Renderer) {
        if !self.is_editor_visible() {
            return;
        }
        let symbols = &mut self.movie.symbols;

        let viewport_dimensions = renderer.viewport_dimensions();
        self.viewport_dimensions = viewport_dimensions;

        let mut commands = CommandList::new();

        // stage background
        let mut stage_color: Color = self.movie.properties.background_color.clone().into();
        if self.editing_clip != None {
            // when editing a clip, fade the stage background
            stage_color.a = 4;
        }
        let world_to_screen_matrix =
            self.camera
                .world_to_screen_matrix(Self::stage_size_from_viewport_dimensions(
                    viewport_dimensions,
                ));
        commands.commands.push(Command::DrawRect {
            color: stage_color,
            matrix: world_to_screen_matrix
                * Matrix::create_box(
                    self.movie.properties.width as f32,
                    self.movie.properties.height as f32,
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
                matrix: world_to_screen_matrix
                    * Matrix::create_box(
                        CROSS_SIZE,
                        1.0,
                        Twips::from_pixels(CROSS_SIZE as f64 / -2.0),
                        Twips::ZERO,
                    ),
            });
            // vertical
            commands.commands.push(Command::DrawRect {
                color: CROSS_COLOR,
                matrix: world_to_screen_matrix
                    * Matrix::create_box(
                        1.0,
                        CROSS_SIZE,
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
                _ => (),
            }
        }

        commands.commands.extend(Editor::render_placed_symbols(
            renderer,
            &self.movie,
            self.editing_clip,
            Transform {
                matrix: world_to_screen_matrix,
                color_transform: ColorTransform::IDENTITY,
            },
        ));

        commands
            .commands
            .extend(self.render_selection(world_to_screen_matrix));

        renderer.submit_frame(Color::from_rgb(0x222222, 255), commands, vec![]);
    }

    fn render_selection(&self, world_to_screen_matrix: Matrix) -> Vec<Command> {
        let mut commands = vec![];
        let placed_symbols = self.movie.get_placed_symbols(self.editing_clip);
        for i in &self.selection {
            let place_symbol = placed_symbols.get(*i).unwrap();
            let size = self.size_of_placed_symbol(place_symbol);
            if let Some(size) = size {
                let scaled_size = (
                    size.0 * place_symbol.transform.x_scale,
                    size.1 * place_symbol.transform.y_scale,
                );
                let line_size = 1.0 / self.camera.zoom_level();
                commands.extend(vec![
                    // top
                    Command::DrawRect {
                        color: Color::BLACK,
                        matrix: world_to_screen_matrix
                            * Matrix::create_box(
                                (scaled_size.0 + line_size * 2.0) as f32,
                                line_size as f32,
                                Twips::from_pixels(
                                    place_symbol.transform.x - scaled_size.0 / 2.0 - line_size,
                                ),
                                Twips::from_pixels(
                                    place_symbol.transform.y - scaled_size.1 / 2.0 - line_size,
                                ),
                            ),
                    },
                    // bottom
                    Command::DrawRect {
                        color: Color::BLACK,
                        matrix: world_to_screen_matrix
                            * Matrix::create_box(
                                (scaled_size.0 + line_size * 2.0) as f32,
                                line_size as f32,
                                Twips::from_pixels(
                                    place_symbol.transform.x - scaled_size.0 / 2.0 - line_size,
                                ),
                                Twips::from_pixels(place_symbol.transform.y + scaled_size.1 / 2.0),
                            ),
                    },
                    // left
                    Command::DrawRect {
                        color: Color::BLACK,
                        matrix: world_to_screen_matrix
                            * Matrix::create_box(
                                line_size as f32,
                                (scaled_size.1 + line_size * 2.0) as f32,
                                Twips::from_pixels(
                                    place_symbol.transform.x - scaled_size.0 / 2.0 - line_size,
                                ),
                                Twips::from_pixels(
                                    place_symbol.transform.y - scaled_size.1 / 2.0 - line_size,
                                ),
                            ),
                    },
                    // right
                    Command::DrawRect {
                        color: Color::BLACK,
                        matrix: world_to_screen_matrix
                            * Matrix::create_box(
                                line_size as f32,
                                (scaled_size.1 + line_size * 2.0) as f32,
                                Twips::from_pixels(place_symbol.transform.x + scaled_size.0 / 2.0),
                                Twips::from_pixels(
                                    place_symbol.transform.y - scaled_size.1 / 2.0 - line_size,
                                ),
                            ),
                    },
                ]);
                // fill whole selection
                /*commands.push(Command::DrawRect {
                    color: Color::BLACK,
                    matrix: world_to_screen_matrix
                        * Matrix::create_box(
                            scaled_size.0 as f32,
                            scaled_size.1 as f32,
                            0.0,
                            Twips::from_pixels(place_symbol.transform.x - scaled_size.0 / 2.0),
                            Twips::from_pixels(place_symbol.transform.y - scaled_size.1 / 2.0),
                        ),
                });*/
            }
        }
        commands
    }

    fn size_of_placed_symbol(&self, place_symbol: &PlaceSymbol) -> Option<(f64, f64)> {
        let symbol = self
            .movie
            .symbols
            .get(place_symbol.symbol_index as usize)
            .expect("Invalid symbol placed");
        match symbol {
            Symbol::Bitmap(bitmap) => match bitmap.size() {
                Some(size) => Some((size.0 as f64, size.1 as f64)),
                None => None,
            },
            Symbol::MovieClip(movieclip) => {
                if movieclip.place_symbols.len() == 0 {
                    return Some((EMPTY_CLIP_WIDTH, EMPTY_CLIP_HEIGHT));
                }
                let mut total_x_min = 0.0;
                let mut total_y_min = 0.0;
                let mut total_x_max = 0.0;
                let mut total_y_max = 0.0;
                for place_symbol in &movieclip.place_symbols {
                    let size = self.size_of_placed_symbol(place_symbol);
                    let Some(size) = size else {
                        continue;
                    };
                    let scaled_half_size = (
                        size.0 * place_symbol.transform.x_scale / 2.0,
                        size.1 * place_symbol.transform.y_scale / 2.0,
                    );
                    let x_min = place_symbol.transform.x - scaled_half_size.0;
                    if x_min < total_x_min {
                        total_x_min = x_min;
                    }
                    let y_min = place_symbol.transform.y - scaled_half_size.1;
                    if y_min < total_y_min {
                        total_y_min = y_min;
                    }
                    let x_max = place_symbol.transform.x + scaled_half_size.0;
                    if x_max > total_x_max {
                        total_x_max = x_max;
                    }
                    let y_max = place_symbol.transform.y + scaled_half_size.1;
                    if y_max > total_y_max {
                        total_y_max = y_max;
                    }
                }
                Some((total_x_max - total_x_min, total_y_max - total_y_min))
            }
            Symbol::Font(_) => {
                let text_properties = place_symbol.text.as_ref().unwrap();
                Some((text_properties.width, text_properties.height))
            }
        }
    }

    fn cache_bitmap_handle(renderer: &mut Renderer, cached_bitmap: &mut CachedBitmap) {
        cached_bitmap.bitmap_handle = Some(Box::new(BitmapHandleWrapper(
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
        )));
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
                    let bitmap_handle: &BitmapHandle =
                        match bitmap_handle.as_any().downcast_ref::<BitmapHandleWrapper>() {
                            Some(b) => &b.0,
                            None => panic!("BitmapHandle is not of the right type"),
                        };
                    let place_symbol_matrix =
                        <swf::Matrix as Into<Matrix>>::into(<EditorTransform as Into<
                            swf::Matrix,
                        >>::into(
                            place_symbol.transform.clone()
                        ));
                    commands.push(Command::RenderBitmap {
                        bitmap: bitmap_handle.clone(),
                        transform: Transform {
                            // bitmap coordinates are centered in order to make scaling and rotation easier
                            matrix: transform.matrix
                                * (place_symbol_matrix
                                    * Matrix::translate(
                                        Twips::from_pixels(
                                            cached_bitmap.image.width() as f64 / -2.0,
                                        ),
                                        Twips::from_pixels(
                                            cached_bitmap.image.height() as f64 / -2.0,
                                        ),
                                    )),
                            color_transform: transform.color_transform,
                        },
                        smoothing: false,
                        pixel_snapping: PixelSnapping::Never, // TODO: figure out a good default
                    });
                }
                Symbol::MovieClip(clip) => {
                    let place_symbol_matrix =
                        <swf::Matrix as Into<Matrix>>::into(<EditorTransform as Into<
                            swf::Matrix,
                        >>::into(
                            place_symbol.transform.clone()
                        ));
                    // draw empty clip as purple square
                    if clip.place_symbols.len() == 0 {
                        commands.push(Command::DrawRect {
                            color: Color::MAGENTA,
                            matrix: transform.matrix
                                * place_symbol_matrix
                                * Matrix::create_box(
                                    EMPTY_CLIP_WIDTH as f32,
                                    EMPTY_CLIP_HEIGHT as f32,
                                    Twips::from_pixels(EMPTY_CLIP_WIDTH / -2.0),
                                    Twips::from_pixels(EMPTY_CLIP_HEIGHT / -2.0),
                                ),
                        })
                    }

                    commands.extend(Editor::render_placed_symbols(
                        renderer,
                        movie,
                        Some(place_symbol.symbol_index as usize),
                        Transform {
                            matrix: transform.matrix * place_symbol_matrix,
                            color_transform: transform.color_transform,
                        },
                    ));
                }
                Symbol::Font(_) => {
                    let place_symbol_matrix =
                        <swf::Matrix as Into<Matrix>>::into(<EditorTransform as Into<
                            swf::Matrix,
                        >>::into(
                            place_symbol.transform.clone()
                        ));
                    let text_properties = place_symbol.text.as_ref().unwrap();
                    commands.push(Command::DrawRect {
                        color: Color::MAGENTA,
                        matrix: transform.matrix
                            * place_symbol_matrix
                            * Matrix::create_box(
                                text_properties.width as f32,
                                text_properties.height as f32,
                                Twips::from_pixels(text_properties.width / -2.0),
                                Twips::from_pixels(text_properties.height / -2.0),
                            ),
                    })
                }
            }
        }
        commands
    }

    pub fn handle_mouse_move(&mut self, mouse_x: f64, mouse_y: f64) {
        if !self.is_editor_visible() {
            return;
        }
        let world_space_mouse_position = self.camera.screen_to_world_matrix(self.stage_size())
            * Matrix::translate(Twips::from_pixels(mouse_x), Twips::from_pixels(mouse_y));
        let placed_symbols = self.movie.get_placed_symbols_mut(self.editing_clip);
        if let Some(drag_data) = &self.drag_data {
            let place_symbol = placed_symbols
                .get_mut(drag_data.place_symbol_index)
                .unwrap();
            place_symbol.transform.x = drag_data.symbol_start_transform.x
                + world_space_mouse_position.tx.to_pixels()
                - drag_data.start_x;
            place_symbol.transform.y = drag_data.symbol_start_transform.y
                + world_space_mouse_position.ty.to_pixels()
                - drag_data.start_y;
        }

        self.camera.update_drag(mouse_x, mouse_y);
    }

    pub fn handle_mouse_input(
        &mut self,
        mouse_x: f64,
        mouse_y: f64,
        button: MouseButton,
        state: ElementState,
    ) {
        if !self.is_editor_visible() {
            return;
        }
        let world_space_mouse_position = self.camera.screen_to_world_matrix(self.stage_size())
            * Matrix::translate(Twips::from_pixels(mouse_x), Twips::from_pixels(mouse_y));
        if button == MouseButton::Left && state == ElementState::Pressed {
            self.set_selection(vec![]);
            let symbol_index =
                self.get_placed_symbol_at_position(mouse_x, mouse_y, self.editing_clip);
            if let Some(symbol_index) = symbol_index {
                let place_symbol = &self.movie.get_placed_symbols(self.editing_clip)[symbol_index];
                self.drag_data = Some(DragData {
                    symbol_start_transform: place_symbol.transform.clone(),
                    start_x: world_space_mouse_position.tx.to_pixels(),
                    start_y: world_space_mouse_position.ty.to_pixels(),
                    place_symbol_index: symbol_index,
                });
                self.set_selection(vec![symbol_index]);
            }
        }
        if button == MouseButton::Left && state == ElementState::Released {
            if let Some(drag_data) = &self.drag_data {
                let end = EditorTransform {
                    x: drag_data.symbol_start_transform.x
                        + world_space_mouse_position.tx.to_pixels()
                        - drag_data.start_x,
                    y: drag_data.symbol_start_transform.y
                        + world_space_mouse_position.ty.to_pixels()
                        - drag_data.start_y,
                    x_scale: self.movie.get_placed_symbols(self.editing_clip)
                        [drag_data.place_symbol_index]
                        .transform
                        .x_scale,
                    y_scale: self.movie.get_placed_symbols(self.editing_clip)
                        [drag_data.place_symbol_index]
                        .transform
                        .y_scale,
                };

                // only insert an edit if you actually moved the placed symbol
                if f64::abs(drag_data.symbol_start_transform.x - end.x) > EDIT_EPSILON
                    || f64::abs(drag_data.symbol_start_transform.y - end.y) > EDIT_EPSILON
                {
                    self.do_edit(MovieEdit::EditPlacedSymbol(PlacedSymbolEdit {
                        editing_symbol_index: self.editing_clip,
                        placed_symbol_index: drag_data.place_symbol_index,
                        start: PlaceSymbol::from_transform(
                            &self.movie.get_placed_symbols(self.editing_clip)
                                [drag_data.place_symbol_index],
                            drag_data.symbol_start_transform.clone(),
                        ),
                        end: PlaceSymbol::from_transform(
                            &self.movie.get_placed_symbols(self.editing_clip)
                                [drag_data.place_symbol_index],
                            end,
                        ),
                    }));
                }
                self.drag_data = None;
            }
        }
        if button == MouseButton::Middle && state == ElementState::Pressed {
            self.camera.start_drag(mouse_x, mouse_y)
        }
        if button == MouseButton::Middle && state == ElementState::Released {
            self.camera.stop_drag();
        }
    }

    fn do_edit(&mut self, edit: MovieEdit) {
        let result = self.history.edit(&mut self.movie, edit);
        self.change_view_after_edit(result);
        self.update_title();
    }

    pub(crate) fn do_undo(&mut self) {
        let result = self.history.undo(&mut self.movie);
        if let Some(editing_clip) = result {
            self.change_view_after_edit(editing_clip);
        }
        self.update_title();
    }

    pub(crate) fn do_redo(&mut self) {
        let result = self.history.redo(&mut self.movie);
        if let Some(editing_clip) = result {
            self.change_view_after_edit(editing_clip);
        }
        self.update_title();
    }

    pub(crate) fn save(&mut self) {
        self.movie.save(&self.project_file_path);
        self.history.set_saved(true);
        self.update_title();
    }

    fn update_title(&self) {
        self.event_loop
            .send_event(FlitsEvent::UpdateTitle)
            .unwrap_or_else(|err| {
                eprintln!("Unable to send command output event: {}", err);
            });
    }

    fn change_view_after_edit(&mut self, output: MoviePropertiesOutput) {
        match output {
            MoviePropertiesOutput::Stage(editing_clip) => {
                self.change_editing_clip(editing_clip);
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
            MoviePropertiesOutput::PlacedSymbolProperties(editing_clip, placed_symbol_index) => {
                self.change_editing_clip(editing_clip);
                self.set_selection(vec![placed_symbol_index]);
            }
            MoviePropertiesOutput::RemovedPlacedSymbol(editing_clip) => {
                // reset the selection to make sure the removed placed symbol isn't selected anymore
                self.set_selection(vec![]);
                self.change_editing_clip(editing_clip);
            }
        }
    }

    fn get_placed_symbol_at_position(
        &self,
        x: f64,
        y: f64,
        symbol_index: SymbolIndexOrRoot,
    ) -> SymbolIndexOrRoot {
        let world_space_position = self.camera.screen_to_world_matrix(self.stage_size())
            * Matrix::translate(Twips::from_pixels(x), Twips::from_pixels(y));

        self.get_placed_symbol_at_position_local_space(
            world_space_position.tx.to_pixels(),
            world_space_position.ty.to_pixels(),
            symbol_index,
        )
    }
    fn get_placed_symbol_at_position_local_space(
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
            let place_symbol_x = place_symbol.transform.x;
            let place_symbol_y = place_symbol.transform.y;
            match symbol {
                Symbol::Bitmap(bitmap) => {
                    if let BitmapCacheStatus::Cached(cached_bitmap) = &bitmap.cache {
                        let half_width = cached_bitmap.image.width() as f64
                            * place_symbol.transform.x_scale
                            / 2.0;
                        let half_height = cached_bitmap.image.height() as f64
                            * place_symbol.transform.y_scale
                            / 2.0;
                        if x > place_symbol_x - half_width
                            && y > place_symbol_y - half_height
                            && x < place_symbol_x + half_width
                            && y < place_symbol_y + half_height
                        {
                            return Some(i);
                        }
                    }
                }
                Symbol::MovieClip(clip) => {
                    if clip.place_symbols.len() == 0 {
                        let half_width = EMPTY_CLIP_WIDTH / 2.0;
                        let half_height = EMPTY_CLIP_HEIGHT / 2.0;
                        if x > place_symbol_x - half_width
                            && y > place_symbol_y - half_height
                            && x < place_symbol_x + half_width
                            && y < place_symbol_y + half_height
                        {
                            return Some(i);
                        }
                    }
                    if let Some(_) = self.get_placed_symbol_at_position_local_space(
                        (x - place_symbol_x) / place_symbol.transform.x_scale,
                        (y - place_symbol_y) / place_symbol.transform.y_scale,
                        Some(place_symbol.symbol_index as usize),
                    ) {
                        return Some(i);
                    }
                }
                Symbol::Font(_) => {
                    let text_properties = place_symbol.text.as_ref().unwrap();
                    let half_width = text_properties.width * place_symbol.transform.x_scale / 2.0;
                    let half_height = text_properties.height * place_symbol.transform.y_scale / 2.0;
                    if x > place_symbol_x - half_width
                        && y > place_symbol_y - half_height
                        && x < place_symbol_x + half_width
                        && y < place_symbol_y + half_height
                    {
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
        event_loop: &EventLoopProxy<FlitsEvent>,
    ) -> NeedsRedraw {
        if let Some(run_ui) = &mut self.run_ui {
            run_ui.do_ui(egui_ctx);
        }
        // don't show the editor ui when you have selected a different tab in the run ui
        if !self.is_editor_visible() {
            return NeedsRedraw::No;
        }
        let mut needs_redraw = NeedsRedraw::No;
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
                            needs_redraw = NeedsRedraw::Yes;
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

                            if response.clicked() {
                                match self.movie.symbols[i] {
                                    Symbol::MovieClip(_) => {
                                        self.change_editing_clip(Some(i));
                                    }
                                    _ => {
                                        self.properties_panel =
                                            Self::create_symbol_propeties_panel(i, symbol);
                                    }
                                }

                                needs_redraw = NeedsRedraw::Yes;
                            } else if response.drag_stopped() {
                                // TODO: handle drag that doesn't end on stage
                                let mouse_pos = response.interact_pointer_pos().unwrap();
                                let mut matrix =
                                    self.camera.screen_to_world_matrix(self.stage_size())
                                        * Matrix::translate(
                                            Twips::from_pixels(mouse_pos.x as f64),
                                            Twips::from_pixels(
                                                // TODO: don't hardcode the menu height
                                                mouse_pos.y as f64 - MENU_HEIGHT as f64,
                                            ),
                                        );
                                // reset zoom (otherwise when you are zoomed in the symbol becomes smaller)
                                matrix.a = Matrix::IDENTITY.a;
                                matrix.b = Matrix::IDENTITY.b;
                                matrix.c = Matrix::IDENTITY.c;
                                matrix.d = Matrix::IDENTITY.d;
                                self.do_edit(MovieEdit::AddPlacedSymbol(AddPlacedSymbolEdit {
                                    editing_symbol_index: self.editing_clip,
                                    placed_symbol: PlaceSymbol {
                                        symbol_index: i,
                                        transform: EditorTransform {
                                            x: matrix.tx.to_pixels(),
                                            y: matrix.ty.to_pixels(),
                                            x_scale: 1.0,
                                            y_scale: 1.0,
                                        },
                                        instance_name: "".into(),
                                        text: match &self.movie.symbols[i] {
                                            Symbol::Font(_) => Some(Box::new(TextProperties {
                                                text: "123456".into(),
                                                width: 200.0,
                                                height: 50.0,
                                                size: 50.0,
                                                color: EditorColor::BLACK,
                                                align: TextAlign::Left,
                                            })),
                                            _ => None,
                                        },
                                    },
                                    placed_symbol_index: None,
                                }));
                                needs_redraw = NeedsRedraw::Yes;
                            }
                        }
                    });
            });
        egui::TopBottomPanel::top("breadcrumb_bar").show(egui_ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(editing_clip) = self.editing_clip {
                    if ui.selectable_label(false, "Scene").clicked() {
                        self.change_editing_clip(None);
                        needs_redraw = NeedsRedraw::Yes;
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
                needs_redraw = NeedsRedraw::Yes; // some edits cause cascading effects (for example changing the path of a bitmap)
            }
        });

        if let Some(new_symbol_window) = &mut self.new_symbol_window {
            match new_symbol_window.do_ui(egui_ctx) {
                crate::new_symbol_window::NewSymbolWindowResult::NoAction => {}
                crate::new_symbol_window::NewSymbolWindowResult::Cancel => {
                    self.new_symbol_window = None;
                }
                crate::new_symbol_window::NewSymbolWindowResult::Confirm(edit) => {
                    self.do_edit(edit);
                    self.new_symbol_window = None;
                }
            }
        }

        if let Some(export_error) = &self.export_error {
            egui::TopBottomPanel::bottom("export_error").show(egui_ctx, |ui| {
                ui.colored_label(ui.style().visuals.error_fg_color, export_error);
            });
        }

        needs_redraw
    }

    fn is_editor_visible(&self) -> bool {
        if let Some(run_ui) = &self.run_ui {
            return run_ui.is_editor_visible();
        }
        true
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
            Symbol::Font(font) => PropertiesPanel::SymbolProperties(SymbolPropertiesPanel {
                symbol_index,
                before_edit: SymbolProperties::Font(font.clone()),
            }),
        }
    }

    fn change_editing_clip(&mut self, symbol_index: SymbolIndexOrRoot) {
        // if switching to the same symbol, just switch the properies panel
        // (because you might have selected something else)
        if symbol_index == self.editing_clip {
            self.change_view_after_edit(MoviePropertiesOutput::Properties(symbol_index));
            return;
        }

        if let Some(symbol_index) = symbol_index {
            let Symbol::MovieClip(_) = self.movie.symbols[symbol_index] else {
                // only select movieclips
                return;
            };
            // center the camera on the origin when you open a movieclip
            self.camera.reset_to_origin();
        } else {
            // center the camera on the stage when you open root
            self.camera.reset_to_center_stage(&self.movie.properties);
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
                                Symbol::Font(font) => SymbolProperties::Font(font.clone()),
                            },
                        });
                } else {
                    // only recreate the panel if it doesn't exist already
                    if !matches!(self.properties_panel, PropertiesPanel::MovieProperties(_)) {
                        self.properties_panel =
                            PropertiesPanel::MovieProperties(MoviePropertiesPanel {
                                before_edit: self.movie.properties.clone(),
                            });
                    }
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

    pub(crate) fn delete_selection(&mut self) {
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

    pub fn reload_assets(&mut self) {
        self.movie.reload_assets(&self.directory);
    }

    pub fn export_and_run(&mut self, event_loop: &EventLoopProxy<FlitsEvent>) {
        // only run the movie if the export is successful
        if self.export_swf().is_ok() {
            self.run_ui = Some(RunUi::new());
            let result = run_movie(
                &self.directory.join("output.swf"),
                event_loop.clone(),
                |line, event_loop| {
                    // TODO: debounce events
                    event_loop
                        .send_event(FlitsEvent::CommandOutput(line))
                        .unwrap_or_else(|err| {
                            eprintln!("Unable to send command output event: {}", err);
                        });
                },
                |event_loop| {
                    event_loop
                        .send_event(FlitsEvent::RuffleClosed)
                        .unwrap_or_else(|err| {
                            eprintln!("Unable to send command output event: {}", err);
                        });
                },
            );
            self.export_error = match &result {
                Ok(_) => None,
                Err(err) => Some(err.to_string()),
            };
        }
    }

    pub fn export_swf(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let directory = self.directory.clone();
        let swf_path = directory.clone().join("output.swf");
        let result = self.movie.export(directory, swf_path);
        self.export_error = match &result {
            Ok(_) => None,
            Err(err) => Some(err.to_string()),
        };
        result
    }

    // TODO: maybe just hardcode the zoom percentages: https://www.uxpin.com/studio/blog/the-strikingly-precise-zoom/
    pub fn zoom(&mut self, zoom_amount: f64) {
        self.camera.zoom(zoom_amount);
    }

    pub fn reset_zoom(&mut self) {
        self.camera.reset_zoom();
    }

    pub fn receive_command_output(&mut self, line: String) -> NeedsRedraw {
        if let Some(run_ui) = &mut self.run_ui {
            run_ui.add_line(line);
            if run_ui.needs_redraw_after_new_line() {
                return NeedsRedraw::Yes;
            }
        }
        NeedsRedraw::No
    }
    pub fn on_ruffle_closed(&mut self) {
        self.run_ui = None;
    }

    pub fn project_name(&self) -> &str {
        self.directory
            .as_path()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap_or("INVALID DIRECTORY NAME")
    }
    pub fn unsaved_changes(&self) -> bool {
        !self.history.is_saved()
    }
}
