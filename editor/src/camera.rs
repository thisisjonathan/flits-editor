use ruffle_render::matrix::Matrix;
use swf::Twips;

use flits_core::MovieProperties;

use crate::editor::StageSize;

pub struct Camera {
    // x and y are the world coordinates at the center of the screen
    x: f64,
    y: f64,
    zoom_level: f64,
    drag_data: Option<CameraDragData>,
}
impl Camera {
    pub fn new_center_stage(movie_properties: &MovieProperties) -> Camera {
        let mut camera = Camera {
            x: 0.0,
            y: 0.0,
            zoom_level: 1.0,
            drag_data: None,
        };
        camera.reset_to_center_stage(movie_properties);
        camera
    }
    pub fn world_to_screen_matrix(&self, stage_size: StageSize) -> Matrix {
        Matrix::translate(
            Twips::from_pixels(stage_size.width as f64 / 2.0),
            Twips::from_pixels(stage_size.height as f64 / 2.0),
        ) * Matrix::create_box(
            self.zoom_level as f32,
            self.zoom_level as f32,
            Twips::from_pixels(-self.x * self.zoom_level),
            Twips::from_pixels(-self.y * self.zoom_level),
        )
    }
    pub fn screen_to_world_matrix(&self, stage_size: StageSize) -> Matrix {
        self.world_to_screen_matrix(stage_size)
            .inverse()
            .unwrap_or(Matrix::IDENTITY) // TODO: does this make sense?
    }

    pub fn start_drag(&mut self, mouse_x: f64, mouse_y: f64) {
        self.drag_data = Some(CameraDragData {
            previous_x: mouse_x,
            previous_y: mouse_y,
        });
    }
    pub fn stop_drag(&mut self) {
        self.drag_data = None;
    }
    pub fn update_drag(&mut self, mouse_x: f64, mouse_y: f64) {
        if let Some(camera_drag_data) = &self.drag_data {
            self.x -= (mouse_x - camera_drag_data.previous_x) / self.zoom_level;
            self.y -= (mouse_y - camera_drag_data.previous_y) / self.zoom_level;
            self.drag_data = Some(CameraDragData {
                previous_x: mouse_x,
                previous_y: mouse_y,
            });
        }
    }

    pub fn reset_to_origin(&mut self) {
        self.x = 0.0;
        self.y = 0.0;
        self.reset_zoom();
    }
    pub fn reset_to_center_stage(&mut self, movie_properties: &MovieProperties) {
        self.x = movie_properties.width / 2.0;
        self.y = movie_properties.height / 2.0;
        self.reset_zoom();
    }

    pub fn reset_zoom(&mut self) {
        self.zoom_level = 1.0;
    }
    pub fn zoom(&mut self, zoom_amount: f64) {
        self.zoom_level += zoom_amount;
    }

    pub fn zoom_level(&self) -> f64 {
        self.zoom_level
    }
}
struct CameraDragData {
    previous_x: f64,
    previous_y: f64,
}
