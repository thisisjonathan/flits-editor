use std::path::PathBuf;

use flits_core::{
    BitmapCacheStatus, CachedBitmap, EditorTransform, Movie, MovieProperties, PlaceSymbol, Symbol,
    SymbolIndexOrRoot,
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

use crate::{
    camera::Camera,
    editor::{
        BitmapHandleWrapper, RenderContext, Renderer, StageSize, EMPTY_CLIP_HEIGHT,
        EMPTY_CLIP_WIDTH, LIBRARY_WIDTH,
    },
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

pub struct Stage {
    camera: Camera,
    // Option because we need the renderer to intialize it
    text_renderer: Option<TextRenderer>,

    directory: PathBuf,
    box_selection: Option<BoxSelection>,
}
impl Stage {
    pub fn new(movie_properties: &MovieProperties, directory: PathBuf) -> Self {
        Stage {
            camera: Camera::new_center_stage(movie_properties),
            text_renderer: None,
            directory,
            box_selection: None,
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
            let bounds = self.bounds_of_placed_symbol(ctx, place_symbol);
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

    fn bounds_of_placed_symbol(
        &self,
        ctx: &RenderContext,
        place_symbol: &PlaceSymbol,
    ) -> Option<Bounds> {
        let local_bounds = self.local_bounds_of_placed_symbol(ctx, place_symbol);
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
        ctx: &RenderContext,
        place_symbol: &PlaceSymbol,
    ) -> Option<Bounds> {
        let symbol = ctx
            .movie
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
                    let bounds = self.bounds_of_placed_symbol(ctx, inner_place_symbol);
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
}
