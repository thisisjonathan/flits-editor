use egui::CursorIcon;
use ruffle_render::backend::{RenderBackend, ViewportDimensions};
use std::sync::{Arc, MutexGuard};
use winit::{
    event::{ElementState, MouseButton},
    window::Window,
};

use crate::MovieView;

pub enum NeedsRedraw {
    Yes,
    No,
}

pub trait RuffleGui {
    type Player: Player;
    type Arguments;

    fn on_player_created(
        &self,
        arguments: &Self::Arguments,
        player: MutexGuard<Self::Player>,
    ) -> ();
    fn update(
        &self,
        context: &egui::Context,
        show_menu: bool,
        player: Option<&mut Self::Player>,
        menu_height_offset: f64,
    ) -> NeedsRedraw;
    fn on_player_destroyed(&self);
    fn cursor_icon(&self) -> Option<CursorIcon>;

    fn after_window_init(&self, window: Arc<Window>, egui_ctx: &egui::Context);
    fn after_render(&self, instance: &wgpu::Instance);
}
pub trait Player {
    fn render(&mut self);
    fn renderer_mut(&mut self) -> &mut dyn RenderBackend;
    fn set_viewport_dimensions(&mut self, viewport_dimensions: ViewportDimensions);

    fn tick(&mut self, dt: f64);
    fn time_til_next_frame(&self) -> Option<std::time::Duration>;

    fn handle_mouse_move(&mut self, mouse_x: f64, mouse_y: f64);
    fn handle_mouse_input(
        &mut self,
        mouse_x: f64,
        mouse_y: f64,
        button: MouseButton,
        state: ElementState,
    );
}
pub trait PlayerController {
    type Player: Player;
    type Arguments;

    fn create(&mut self, arguments: &Self::Arguments, movie_view: MovieView);
    fn destroy(&mut self);

    fn get(&self) -> Option<MutexGuard<Self::Player>>;
}

pub struct Config<'a> {
    pub preferred_backends: wgpu::Backends,
    pub power_preference: wgpu::PowerPreference,
    pub trace_path: Option<&'a std::path::Path>,
    pub present_mode: wgpu::PresentMode,
    pub desired_maximum_frame_latency: u32,
    pub height_offset_unscaled: u32,
    pub send_tab_to_player: bool,
}
