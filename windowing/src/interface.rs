use egui::CursorIcon;
use ruffle_render::backend::RenderBackend;
use std::sync::MutexGuard;

use crate::MovieView;

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
    ) -> ();
    fn on_player_destroyed(&self);
    fn is_context_menu_visible(&self) -> bool;
    fn height_offset_unscaled(&self) -> u32;
    fn cursor_icon(&self) -> Option<CursorIcon>;
}
pub trait Player {
    fn render(&mut self);
    fn renderer_mut(&mut self) -> &mut dyn RenderBackend;
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
}
