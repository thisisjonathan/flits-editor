use std::{any::Any, path::PathBuf};

use flits_core::{Movie, PlacedSymbolIndex, Symbol, SymbolIndexOrRoot};
use ruffle_render::backend::{RenderBackend, ViewportDimensions};
use tracing::instrument;
use undo::Record;
use winit::{
    event::{ElementState, MouseButton},
    event_loop::EventLoopProxy,
};

use crate::{
    edit::{MovieEdit, MoviePropertiesOutput},
    editor::{breadcrumb_bar::BreadcrumbBar, library::Library, menu_bar::MenuBar, stage::Stage},
    message::EditorMessage,
    message_bus::MessageBus,
    properties_panel::{MoviePropertiesPanel, PropertiesPanel},
    FlitsEvent,
};

mod breadcrumb_bar;
mod library;
mod menu_bar;
pub(crate) mod stage;

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

pub struct StageSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Default)]
pub struct Selection {
    pub stage_symbol_index: SymbolIndexOrRoot,
    pub properties_symbol_index: SymbolIndexOrRoot,
    pub placed_symbols: Vec<PlacedSymbolIndex>,
}

pub struct Context<'a> {
    pub movie: &'a Movie,
    pub selection: &'a Selection,
    pub modifiers: egui::Modifiers,
    pub message_bus: &'a MessageBus<EditorMessage>,
}
pub struct MutableContext<'a> {
    pub movie: &'a mut Movie,
    pub selection: &'a Selection,
    pub modifiers: egui::Modifiers,
    pub message_bus: &'a MessageBus<EditorMessage>,
    pub viewport_dimensions: ViewportDimensions,
}
pub struct RenderContext<'a> {
    pub movie: &'a mut Movie,
    pub selection: &'a Selection,
    pub renderer: &'a mut Renderer,
}

pub struct Editor {
    movie: Movie,
    project_file_path: PathBuf,
    directory: PathBuf,

    viewport_dimensions: ViewportDimensions,
    event_loop: EventLoopProxy<FlitsEvent>,

    selection: Selection,
    history: Record<MovieEdit>,
    modifiers: egui::Modifiers,

    menu_bar: MenuBar,
    library: Library,
    breadcrumb_bar: BreadcrumbBar,
    stage: Stage,
    properties_panel: PropertiesPanel,
}
impl Editor {
    pub fn new(
        path: PathBuf,
        viewport_dimensions: ViewportDimensions,
        event_loop: EventLoopProxy<FlitsEvent>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
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
            directory: directory.clone(),

            viewport_dimensions,
            event_loop,

            selection: Selection::default(),
            history: Record::new(),
            modifiers: egui::Modifiers::NONE,

            menu_bar: MenuBar::default(),
            library: Library::default(),
            breadcrumb_bar: BreadcrumbBar::default(),
            stage: Stage::new(&movie_properties, directory),
            properties_panel: PropertiesPanel::MovieProperties(MoviePropertiesPanel {
                before_edit: movie_properties,
            }),
        })
    }

    pub fn do_ui(
        &mut self,
        egui_ctx: &egui::Context,
        event_loop: &EventLoopProxy<FlitsEvent>,
    ) -> NeedsRedraw {
        egui_ctx.input(|input| self.modifiers = input.modifiers);

        let message_bus = MessageBus::new();
        let context = Context {
            movie: &self.movie,
            selection: &self.selection,
            modifiers: self.modifiers,
            message_bus: &message_bus,
        };

        egui::TopBottomPanel::top("menu_bar").show(egui_ctx, |ui| {
            self.menu_bar.do_ui(ui, &context);
        });

        egui::SidePanel::right("library")
            .resizable(false) // resizing causes glitches
            .min_width(LIBRARY_WIDTH as f32)
            .show(egui_ctx, |ui| {
                self.library.do_ui(ui, &context);
            });

        egui::TopBottomPanel::top("breadcrumb_bar").show(egui_ctx, |ui| {
            self.breadcrumb_bar.do_ui(ui, &context);
        });

        egui::TopBottomPanel::bottom("properties").show(egui_ctx, |ui| {
            let mut mutable_context = MutableContext {
                movie: &mut self.movie,
                selection: &self.selection,
                modifiers: self.modifiers,
                message_bus: &message_bus,
                viewport_dimensions: self.viewport_dimensions,
            };
            self.properties_panel.do_ui(ui, &mut mutable_context);
        });

        self.handle_messages(message_bus);

        NeedsRedraw::No
    }

    fn handle_message(&mut self, message: EditorMessage) {
        match message {
            EditorMessage::ChangeSelectedSymbol(symbol_index) => {
                // if root or movieclip, change the stage
                if symbol_index.is_none_or(|symbol_index| match &self.movie.symbols[symbol_index] {
                    Symbol::MovieClip(_movie_clip) => true,
                    _ => false,
                }) {
                    self.selection.stage_symbol_index = symbol_index;

                    let message_bus = MessageBus::new();
                    self.stage.reset_camera(Context {
                        movie: &self.movie,
                        selection: &self.selection,
                        modifiers: self.modifiers,
                        message_bus: &message_bus,
                    });
                    self.handle_messages(message_bus);

                    self.handle_message(EditorMessage::ChangeSelectedPlacedSymbols(Vec::new()));
                }

                // always change the properties panel selection
                self.selection.properties_symbol_index = symbol_index;

                self.selection.placed_symbols = Vec::new();
                self.properties_panel.update(&self.movie, &self.selection);
            }
            EditorMessage::ChangeSelectedPlacedSymbols(items) => {
                self.selection.placed_symbols = items;
                self.selection.properties_symbol_index = self.selection.stage_symbol_index;
                self.properties_panel.update(&self.movie, &self.selection);
            }
            EditorMessage::SelectAll => {
                // TODO: queue redraw
                self.handle_message(EditorMessage::ChangeSelectedPlacedSymbols(
                    (0..self
                        .movie
                        .get_placed_symbols(self.selection.stage_symbol_index)
                        .len())
                        .collect(),
                ));
            }
            EditorMessage::Edit(edit) => {
                let result = self.history.edit(&mut self.movie, edit);
                self.update_after_edit(Some(result));
            }
            EditorMessage::Undo => {
                let result = self.history.undo(&mut self.movie);
                self.update_after_edit(result);
            }
            EditorMessage::Redo => {
                let result = self.history.redo(&mut self.movie);
                self.update_after_edit(result);
            }
            EditorMessage::Stage(stage_message) => {
                self.stage.handle_message(stage_message);
            }
            EditorMessage::Event(flits_event) => {
                self.event_loop
                    .send_event(flits_event)
                    .unwrap_or_else(|err| {
                        eprintln!("Unable to send event: {}", err);
                    });
            }
            EditorMessage::TODO => todo!(),
        }
    }
    fn update_after_edit(&mut self, result: Option<MoviePropertiesOutput>) {
        if let Some(result) = result {
            match result {
                MoviePropertiesOutput::Stage(editing_clip) => {
                    // TODO: this should always change the stage
                    self.handle_message(EditorMessage::ChangeSelectedSymbol(editing_clip));
                }
                MoviePropertiesOutput::Properties(editing_clip) => {
                    self.selection.properties_symbol_index = editing_clip;
                    self.properties_panel.update(&self.movie, &self.selection);
                }
                MoviePropertiesOutput::Multi(editing_clip, items) => {
                    self.handle_message(EditorMessage::ChangeSelectedSymbol(editing_clip));
                    self.handle_message(EditorMessage::ChangeSelectedPlacedSymbols(items));
                }
            }
        }

        self.event_loop
            .send_event(FlitsEvent::UpdateTitle)
            .unwrap_or_else(|err| {
                eprintln!("Unable to send command output event: {}", err);
            });
    }

    #[instrument(level = "debug", skip_all)]
    pub fn render(&mut self, renderer: &mut Renderer) {
        if !self.is_editor_visible() {
            return;
        }

        let viewport_dimensions = renderer.viewport_dimensions();
        self.viewport_dimensions = viewport_dimensions;
        let mut context = RenderContext {
            // movie needs to be mutable because of bitmap handles
            movie: &mut self.movie,
            selection: &self.selection,
            renderer,
        };

        self.stage.render(&mut context);
    }

    pub fn handle_mouse_move(&mut self, mouse_x: f64, mouse_y: f64) {
        if !self.is_editor_visible() {
            return;
        }
        let message_bus = MessageBus::new();
        let mut mutable_context = MutableContext {
            movie: &mut self.movie,
            selection: &mut self.selection,
            modifiers: self.modifiers,
            message_bus: &message_bus,
            viewport_dimensions: self.viewport_dimensions,
        };
        self.stage
            .handle_mouse_move(&mut mutable_context, mouse_x, mouse_y);
        self.handle_messages(message_bus);
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
        let message_bus = MessageBus::new();
        let mut mutable_context = MutableContext {
            movie: &mut self.movie,
            selection: &mut self.selection,
            modifiers: self.modifiers,
            message_bus: &message_bus,
            viewport_dimensions: self.viewport_dimensions,
        };
        self.stage
            .handle_mouse_input(&mut mutable_context, mouse_x, mouse_y, button, state);
        self.handle_messages(message_bus);
    }

    fn handle_messages(&mut self, message_bus: MessageBus<EditorMessage>) {
        for message in message_bus.into_vec() {
            self.handle_message(message);
        }
    }

    fn is_editor_visible(&self) -> bool {
        /*if let Some(run_ui) = &self.run_ui {
            return run_ui.is_editor_visible();
        }*/
        true
    }

    pub(crate) fn do_undo(&mut self) {}
    pub(crate) fn do_redo(&mut self) {}
    pub(crate) fn save(&mut self) {}
    pub(crate) fn delete_selection(&mut self) {}
    pub(crate) fn select_all(&mut self) {}

    pub fn reload_assets(&mut self) {}
    pub fn export_and_run(&mut self, event_loop: &EventLoopProxy<FlitsEvent>) {}
    pub fn export_swf(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    // TODO: maybe just hardcode the zoom percentages: https://www.uxpin.com/studio/blog/the-strikingly-precise-zoom/
    pub fn zoom(&mut self, zoom_amount: f64) {}
    pub fn reset_zoom(&mut self) {}
    pub fn receive_command_output(&mut self, line: String) -> NeedsRedraw {
        NeedsRedraw::No
    }
    pub fn on_ruffle_closed(&mut self) {}
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
