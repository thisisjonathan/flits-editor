use std::path::PathBuf;

use flits_core::{
    BitmapCacheStatus, CachedBitmap, EditorTransform, Movie, MovieProperties, PlaceSymbol,
    PlacedSymbolIndex, Symbol, SymbolIndex, SymbolIndexOrRoot,
};
use flits_text_rendering::TextRenderer;
use ruffle_render::{
    backend::ViewportDimensions,
    bitmap::{Bitmap, BitmapFormat, BitmapHandle, PixelSnapping},
    commands::{Command, CommandList},
    matrix::Matrix,
    transform::Transform,
};
use swf::{Color, ColorTransform, Twips};
use winit::event::{ElementState, MouseButton};

use crate::{
    camera::Camera,
    edit::{MovieEdit, MultiEdit, MultiEditEdit, PlacedSymbolEdit},
    editor::{
        BitmapHandleWrapper, Context, MutableContext, RenderContext, Renderer, StageSize,
        EDIT_EPSILON, EMPTY_CLIP_HEIGHT, EMPTY_CLIP_WIDTH, LIBRARY_WIDTH,
    },
    message::EditorMessage,
    text_rendering::FontsConverterBuilder,
};

#[derive(Clone, Copy)]
struct Bounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}
impl Bounds {
    fn contains(&self, other: &Self) -> bool {
        other.min_x >= self.min_x
            && other.min_y >= self.min_y
            && other.max_x <= self.max_x
            && other.max_y <= self.max_y
    }
    fn from_points(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Bounds {
            min_x: x1.min(x2),
            min_y: y1.min(y2),
            max_x: x1.max(x2),
            max_y: y1.max(y2),
        }
    }
}

struct BoxSelection {
    start_x: f64,
    start_y: f64,
    bounds: Bounds,
    // indexes of placed symbols
    // we need to store the items of the box selection specifically to not
    // deselect the existing selection when holding shift
    items: Vec<usize>,
}

#[derive(Clone)]
struct DragData {
    symbol_start_transform: EditorTransform,
    start_x: f64,
    start_y: f64,
    place_symbol_index: SymbolIndex,
}

pub struct Stage {
    camera: Camera,
    // Option because we need the renderer to intialize it
    text_renderer: Option<TextRenderer>,

    directory: PathBuf,
    box_selection: Option<BoxSelection>,

    // one DragData per selected PlacedSymbol
    drag_datas: Option<Vec<DragData>>,
}
impl Stage {
    pub fn new(movie_properties: &MovieProperties, directory: PathBuf) -> Self {
        Stage {
            camera: Camera::new_center_stage(movie_properties),
            text_renderer: None,
            directory,
            box_selection: None,
            drag_datas: None,
        }
    }
    pub fn render(&mut self, ctx: &mut RenderContext) {
        let symbols = &mut ctx.movie.symbols;

        if self.text_renderer.is_none() {
            let flits_fonts = symbols
                .iter()
                .enumerate()
                .filter_map(|index_and_symbol| {
                    let (symbol_index, symbol) = index_and_symbol;
                    match symbol {
                        Symbol::Font(flits_font) => Some((symbol_index, flits_font.clone())),
                        _ => None,
                    }
                })
                .collect();
            let text_renderer = TextRenderer::new(
                Box::new(FontsConverterBuilder::new(
                    flits_fonts,
                    self.directory.clone(),
                )),
                ctx.renderer,
            );
            self.text_renderer = Some(text_renderer);
        }

        let viewport_dimensions = ctx.renderer.viewport_dimensions();

        let mut commands = CommandList::new();

        // stage background
        let mut stage_color: Color = ctx.movie.properties.background_color.clone().into();
        if ctx.selection.stage_symbol_index != None {
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
                    ctx.movie.properties.width as f32,
                    ctx.movie.properties.height as f32,
                    Twips::ZERO,
                    Twips::ZERO,
                ),
        });

        if ctx.selection.stage_symbol_index != None {
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
                            Self::cache_bitmap_handle(ctx.renderer, cached_bitmap);
                        }
                    }
                    BitmapCacheStatus::Cached(cached_bitmap) => {
                        if cached_bitmap.bitmap_handle.is_none() {
                            Self::cache_bitmap_handle(ctx.renderer, cached_bitmap);
                        }
                    }
                    BitmapCacheStatus::Invalid(_) => (),
                },
                _ => (),
            }
        }

        commands.commands.extend(Self::render_placed_symbols(
            ctx.renderer,
            self.text_renderer.as_mut().unwrap(), // we initialized this above
            ctx.movie,
            ctx.selection.stage_symbol_index,
            Transform {
                matrix: world_to_screen_matrix,
                color_transform: ColorTransform::IDENTITY,
            },
            &self.directory,
        ));

        commands
            .commands
            .extend(self.render_selection(ctx, world_to_screen_matrix));

        ctx.renderer
            .submit_frame(Color::from_rgb(0x222222, 255), commands, vec![]);

        // we created this earlier in this function
        self.text_renderer.as_mut().unwrap().finish_frame();
    }

    fn render_selection(
        &self,
        ctx: &mut RenderContext,
        world_to_screen_matrix: Matrix,
    ) -> Vec<Command> {
        let mut commands = vec![];
        let placed_symbols = ctx
            .movie
            .get_placed_symbols(ctx.selection.stage_symbol_index);
        for i in &ctx.selection.placed_symbols {
            let place_symbol = placed_symbols.get(*i).unwrap();
            let bounds = self.bounds_of_placed_symbol(ctx.movie, place_symbol);
            if let Some(bounds) = bounds {
                let mut rect = self.render_selection_rectangle(world_to_screen_matrix, bounds);
                commands.append(&mut rect);
            }
        }

        // render box selection
        if let Some(box_selection) = &self.box_selection {
            let mut rect =
                self.render_selection_rectangle(world_to_screen_matrix, box_selection.bounds);
            commands.append(&mut rect);
        }

        commands
    }

    fn render_selection_rectangle(
        &self,
        world_to_screen_matrix: Matrix,
        bounds: Bounds,
    ) -> Vec<Command> {
        let mut commands = vec![];
        let line_size = 1.0 / self.camera.zoom_level();
        let scaled_size = (bounds.max_x - bounds.min_x, bounds.max_y - bounds.min_y);
        let x = bounds.min_x + scaled_size.0 / 2.0;
        let y = bounds.min_y + scaled_size.1 / 2.0;
        commands.extend(vec![
            // top
            Command::DrawRect {
                color: Color::BLACK,
                matrix: world_to_screen_matrix
                    * Matrix::create_box(
                        (scaled_size.0 + line_size * 2.0) as f32,
                        line_size as f32,
                        Twips::from_pixels(x - scaled_size.0 / 2.0 - line_size),
                        Twips::from_pixels(y - scaled_size.1 / 2.0 - line_size),
                    ),
            },
            // bottom
            Command::DrawRect {
                color: Color::BLACK,
                matrix: world_to_screen_matrix
                    * Matrix::create_box(
                        (scaled_size.0 + line_size * 2.0) as f32,
                        line_size as f32,
                        Twips::from_pixels(x - scaled_size.0 / 2.0 - line_size),
                        Twips::from_pixels(y + scaled_size.1 / 2.0),
                    ),
            },
            // left
            Command::DrawRect {
                color: Color::BLACK,
                matrix: world_to_screen_matrix
                    * Matrix::create_box(
                        line_size as f32,
                        (scaled_size.1 + line_size * 2.0) as f32,
                        Twips::from_pixels(x - scaled_size.0 / 2.0 - line_size),
                        Twips::from_pixels(y - scaled_size.1 / 2.0 - line_size),
                    ),
            },
            // right
            Command::DrawRect {
                color: Color::BLACK,
                matrix: world_to_screen_matrix
                    * Matrix::create_box(
                        line_size as f32,
                        (scaled_size.1 + line_size * 2.0) as f32,
                        Twips::from_pixels(x + scaled_size.0 / 2.0),
                        Twips::from_pixels(y - scaled_size.1 / 2.0 - line_size),
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

        commands
    }

    fn bounds_of_placed_symbol(&self, movie: &Movie, place_symbol: &PlaceSymbol) -> Option<Bounds> {
        let local_bounds = self.local_bounds_of_placed_symbol(movie, place_symbol);
        if let Some(local_bounds) = local_bounds {
            return Some(Bounds {
                min_x: place_symbol.transform.x
                    + local_bounds.min_x * place_symbol.transform.x_scale,
                min_y: place_symbol.transform.y
                    + local_bounds.min_y * place_symbol.transform.y_scale,
                max_x: place_symbol.transform.x
                    + local_bounds.max_x * place_symbol.transform.x_scale,
                max_y: place_symbol.transform.y
                    + local_bounds.max_y * place_symbol.transform.y_scale,
            });
        }
        None
    }

    fn local_bounds_of_placed_symbol(
        &self,
        movie: &Movie,
        place_symbol: &PlaceSymbol,
    ) -> Option<Bounds> {
        let symbol = movie
            .symbols
            .get(place_symbol.symbol_index as usize)
            .expect("Invalid symbol placed");
        match symbol {
            Symbol::Bitmap(bitmap) => match bitmap.size() {
                Some(size) => Some(Bounds {
                    min_x: size.0 as f64 / -2.0,
                    min_y: size.1 as f64 / -2.0,
                    max_x: size.0 as f64 / 2.0,
                    max_y: size.1 as f64 / 2.0,
                }),
                None => None,
            },
            Symbol::MovieClip(movieclip) => {
                if movieclip.place_symbols.len() == 0 {
                    return Some(Bounds {
                        min_x: -EMPTY_CLIP_WIDTH / 2.0,
                        min_y: -EMPTY_CLIP_HEIGHT / 2.0,
                        max_x: EMPTY_CLIP_WIDTH / 2.0,
                        max_y: EMPTY_CLIP_HEIGHT / 2.0,
                    });
                }
                let mut total_bounds = Bounds {
                    min_x: 0.0,
                    min_y: 0.0,
                    max_x: 0.0,
                    max_y: 0.0,
                };
                for inner_place_symbol in &movieclip.place_symbols {
                    let bounds = self.bounds_of_placed_symbol(movie, inner_place_symbol);
                    let Some(bounds) = bounds else {
                        continue;
                    };
                    if bounds.min_x < total_bounds.min_x {
                        total_bounds.min_x = bounds.min_x;
                    }
                    if bounds.min_y < total_bounds.min_y {
                        total_bounds.min_y = bounds.min_y;
                    }
                    if bounds.max_x > total_bounds.max_x {
                        total_bounds.max_x = bounds.max_x;
                    }
                    if bounds.max_y > total_bounds.max_y {
                        total_bounds.max_y = bounds.max_y;
                    }
                }
                Some(total_bounds)
            }
            Symbol::Font(_) => {
                let text_properties = place_symbol.text.as_ref().unwrap();
                Some(Bounds {
                    min_x: -text_properties.width / 2.0,
                    min_y: -text_properties.height / 2.0,
                    max_x: text_properties.width / 2.0,
                    max_y: text_properties.height / 2.0,
                })
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
        renderer: &mut Renderer,
        text_renderer: &mut TextRenderer,
        movie: &Movie,
        symbol_index: SymbolIndexOrRoot,
        transform: Transform,
        directory: &PathBuf,
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

                    commands.extend(Self::render_placed_symbols(
                        renderer,
                        text_renderer,
                        movie,
                        Some(place_symbol.symbol_index as usize),
                        Transform {
                            matrix: transform.matrix * place_symbol_matrix,
                            color_transform: transform.color_transform,
                        },
                        directory,
                    ));
                }
                Symbol::Font(_font) => {
                    let place_symbol_matrix =
                        <swf::Matrix as Into<Matrix>>::into(<EditorTransform as Into<
                            swf::Matrix,
                        >>::into(
                            place_symbol.transform.clone()
                        ));
                    let text_properties = place_symbol.text.as_ref().unwrap();
                    // TODO: ids should be unique for the entire project or reset when switching to a different clip
                    // TODO: don't update the edit texts every frame
                    // TODO: nested edit text works right now because we add the text right before
                    // rendering it, but this won't work when we cache it
                    text_renderer
                        .add_edit_text(i, (place_symbol.symbol_index, *text_properties.clone()));
                    commands.extend(
                        text_renderer
                            .render(
                                i,
                                Transform {
                                    matrix: transform.matrix
                                        * place_symbol_matrix
                                        * Matrix::translate(
                                            Twips::from_pixels(text_properties.width / -2.0),
                                            Twips::from_pixels(text_properties.height / -2.0),
                                        ),
                                    color_transform: transform.color_transform,
                                },
                                renderer,
                            )
                            .commands,
                    );

                    /*if text_rendering_result.is_err() {
                        // draw a pink rectangle when loading/rendering fails
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
                    }*/
                }
            }
        }
        commands
    }

    fn stage_size_from_viewport_dimensions(viewport_dimensions: ViewportDimensions) -> StageSize {
        StageSize {
            width: viewport_dimensions.width - LIBRARY_WIDTH,
            // we don't know the height of the properties panel, so just use an approximation
            height: viewport_dimensions.height - 65,
        }
    }

    pub fn handle_mouse_move(&mut self, ctx: &mut MutableContext, mouse_x: f64, mouse_y: f64) {
        let world_space_mouse_position =
            self.camera
                .screen_to_world_matrix(Self::stage_size_from_viewport_dimensions(
                    ctx.viewport_dimensions,
                ))
                * Matrix::translate(Twips::from_pixels(mouse_x), Twips::from_pixels(mouse_y));
        let placed_symbols = ctx
            .movie
            .get_placed_symbols_mut(ctx.selection.stage_symbol_index);
        if let Some(drag_datas) = &self.drag_datas {
            for drag_data in drag_datas {
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
        }

        let mut updated_selected_placed_symbols: Option<Vec<PlacedSymbolIndex>> = None;

        if self.box_selection.is_some() {
            if let Some(box_selection) = &mut self.box_selection {
                box_selection.bounds = Bounds::from_points(
                    box_selection.start_x,
                    box_selection.start_y,
                    world_space_mouse_position.tx.to_pixels(),
                    world_space_mouse_position.ty.to_pixels(),
                );
            }
            let mut items_to_add_to_selection = Vec::new();
            let placed_symbols = ctx
                .movie
                .get_placed_symbols(ctx.selection.stage_symbol_index);
            if let Some(box_selection) = &self.box_selection {
                // add placed symbols to selection
                for i in 0..placed_symbols.len() {
                    if let Some(bounds) =
                        self.bounds_of_placed_symbol(ctx.movie, &placed_symbols[i])
                    {
                        if box_selection.bounds.contains(&bounds) {
                            if ctx
                                .selection
                                .placed_symbols
                                .iter()
                                .find(|index| **index == i)
                                .is_none()
                            {
                                items_to_add_to_selection.push(i);
                            }
                        }
                    }
                }

                // remove placed symbols from selection
                for i in &box_selection.items {
                    if let Some(bounds) =
                        self.bounds_of_placed_symbol(ctx.movie, &placed_symbols[*i])
                    {
                        if !box_selection.bounds.contains(&bounds) {
                            let placed_symbols_selection =
                                match &mut updated_selected_placed_symbols {
                                    Some(placed_symbols) => placed_symbols,
                                    None => &mut ctx.selection.placed_symbols.clone(),
                                };
                            placed_symbols_selection.retain(|index| *index != *i);
                            updated_selected_placed_symbols =
                                Some(placed_symbols_selection.clone());
                        }
                    }
                }
            }
            for item in items_to_add_to_selection {
                let placed_symbols_selection = match &mut updated_selected_placed_symbols {
                    Some(placed_symbols) => placed_symbols,
                    None => &mut ctx.selection.placed_symbols.clone(),
                };
                placed_symbols_selection.push(item);
                updated_selected_placed_symbols = Some(placed_symbols_selection.clone());

                if let Some(box_selection) = &mut self.box_selection {
                    box_selection.items.push(item);
                }
            }
        }

        if let Some(placed_symbols) = updated_selected_placed_symbols {
            ctx.message_bus
                .publish(EditorMessage::ChangeSelectedPlacedSymbols(placed_symbols));
        }

        self.camera.update_drag(mouse_x, mouse_y);
    }

    pub fn handle_mouse_input(
        &mut self,
        ctx: &mut MutableContext,
        mouse_x: f64,
        mouse_y: f64,
        button: MouseButton,
        state: ElementState,
    ) {
        let world_space_mouse_position =
            self.camera
                .screen_to_world_matrix(Self::stage_size_from_viewport_dimensions(
                    ctx.viewport_dimensions,
                ))
                * Matrix::translate(Twips::from_pixels(mouse_x), Twips::from_pixels(mouse_y));
        if button == MouseButton::Left && state == ElementState::Pressed {
            let symbol_index = self.get_placed_symbol_at_position(
                ctx.movie,
                ctx.viewport_dimensions,
                mouse_x,
                mouse_y,
                ctx.selection.stage_symbol_index,
            );
            if let Some(symbol_index) = symbol_index {
                let item_already_selected = ctx.selection.placed_symbols.contains(&symbol_index);
                let mut placed_symbols_selection = ctx.selection.placed_symbols.clone();
                if !ctx.modifiers.shift && !item_already_selected {
                    placed_symbols_selection = Vec::new();
                } else if item_already_selected && ctx.modifiers.shift {
                    placed_symbols_selection.retain(|si| *si != symbol_index);
                }
                if !item_already_selected {
                    placed_symbols_selection.push(symbol_index);
                }
                ctx.message_bus
                    .publish(EditorMessage::ChangeSelectedPlacedSymbols(
                        placed_symbols_selection.clone(),
                    ));

                self.drag_datas = Some(
                    placed_symbols_selection
                        .iter()
                        .map(|placed_symbol_index| {
                            let place_symbol = &ctx
                                .movie
                                .get_placed_symbols(ctx.selection.stage_symbol_index)
                                [*placed_symbol_index];
                            DragData {
                                symbol_start_transform: place_symbol.transform.clone(),
                                start_x: world_space_mouse_position.tx.to_pixels(),
                                start_y: world_space_mouse_position.ty.to_pixels(),
                                place_symbol_index: *placed_symbol_index,
                            }
                        })
                        .collect(),
                );
            } else {
                if !ctx.modifiers.shift {
                    ctx.message_bus
                        .publish(EditorMessage::ChangeSelectedPlacedSymbols(Vec::new()));
                }
                let mouse_world_x = world_space_mouse_position.tx.to_pixels();
                let mouse_world_y = world_space_mouse_position.ty.to_pixels();
                self.box_selection = Some(BoxSelection {
                    start_x: mouse_world_x,
                    start_y: mouse_world_y,
                    bounds: Bounds {
                        min_x: mouse_world_x,
                        min_y: mouse_world_y,
                        max_x: mouse_world_x,
                        max_y: mouse_world_y,
                    },
                    items: vec![],
                });
            }
            //self.update_selection();
        }
        if button == MouseButton::Left && state == ElementState::Released {
            if let Some(drag_datas) = self.drag_datas.clone() {
                let mut edits = Vec::with_capacity(drag_datas.len());
                for drag_data in drag_datas {
                    let end = EditorTransform {
                        x: drag_data.symbol_start_transform.x
                            + world_space_mouse_position.tx.to_pixels()
                            - drag_data.start_x,
                        y: drag_data.symbol_start_transform.y
                            + world_space_mouse_position.ty.to_pixels()
                            - drag_data.start_y,
                        x_scale: ctx
                            .movie
                            .get_placed_symbols(ctx.selection.stage_symbol_index)
                            [drag_data.place_symbol_index]
                            .transform
                            .x_scale,
                        y_scale: ctx
                            .movie
                            .get_placed_symbols(ctx.selection.stage_symbol_index)
                            [drag_data.place_symbol_index]
                            .transform
                            .y_scale,
                    };

                    // only insert an edit if you actually moved the placed symbol
                    if f64::abs(drag_data.symbol_start_transform.x - end.x) > EDIT_EPSILON
                        || f64::abs(drag_data.symbol_start_transform.y - end.y) > EDIT_EPSILON
                    {
                        edits.push(MultiEditEdit::EditPlacedSymbol(PlacedSymbolEdit {
                            editing_symbol_index: ctx.selection.stage_symbol_index,
                            placed_symbol_index: drag_data.place_symbol_index,
                            start: PlaceSymbol::from_transform(
                                ctx.movie
                                    .get_placed_symbols(ctx.selection.stage_symbol_index)
                                    [drag_data.place_symbol_index]
                                    .clone(),
                                drag_data.symbol_start_transform.clone(),
                            ),
                            end: PlaceSymbol::from_transform(
                                ctx.movie
                                    .get_placed_symbols(ctx.selection.stage_symbol_index)
                                    [drag_data.place_symbol_index]
                                    .clone(),
                                end,
                            ),
                        }));
                    }
                }
                if edits.len() > 0 {
                    ctx.message_bus
                        .publish(EditorMessage::Edit(MovieEdit::Multi(MultiEdit {
                            editing_symbol_index: ctx.selection.stage_symbol_index,
                            edits,
                        })));
                }

                self.drag_datas = None;
            }
            self.box_selection = None;
        }
        if button == MouseButton::Middle && state == ElementState::Pressed {
            self.camera.start_drag(mouse_x, mouse_y)
        }
        if button == MouseButton::Middle && state == ElementState::Released {
            self.camera.stop_drag();
        }
    }

    fn get_placed_symbol_at_position(
        &self,
        movie: &Movie,
        viewport_dimensions: ViewportDimensions,
        x: f64,
        y: f64,
        symbol_index: SymbolIndexOrRoot,
    ) -> SymbolIndexOrRoot {
        let world_space_position =
            self.camera
                .screen_to_world_matrix(Self::stage_size_from_viewport_dimensions(
                    viewport_dimensions,
                ))
                * Matrix::translate(Twips::from_pixels(x), Twips::from_pixels(y));

        self.get_placed_symbol_at_position_local_space(
            movie,
            world_space_position.tx.to_pixels(),
            world_space_position.ty.to_pixels(),
            symbol_index,
        )
    }
    fn get_placed_symbol_at_position_local_space(
        &self,
        movie: &Movie,
        x: f64,
        y: f64,
        symbol_index: SymbolIndexOrRoot,
    ) -> SymbolIndexOrRoot {
        let placed_symbols = movie.get_placed_symbols(symbol_index);
        // iterate from top to bottom to get the item that's on top
        for i in (0..placed_symbols.len()).rev() {
            let place_symbol = &placed_symbols[i];
            let symbol = movie
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
                        movie,
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

    pub fn reset_camera(&mut self, ctx: Context) {
        if let Some(symbol_index) = ctx.selection.stage_symbol_index {
            let Symbol::MovieClip(_) = ctx.movie.symbols[symbol_index] else {
                // only select movieclips
                return;
            };
            // center the camera on the origin when you open a movieclip
            self.camera.reset_to_origin();
        } else {
            // center the camera on the stage when you open root
            self.camera.reset_to_center_stage(&ctx.movie.properties);
        }
    }
}
