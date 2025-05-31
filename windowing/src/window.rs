use crate::{GuiController, Player, PlayerController, RuffleGui};
use ruffle_render::backend::ViewportDimensions;
use std::time::Instant;
use winit::{
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow},
};

pub struct RuffleWindow<G, P>
where
    G: RuffleGui,
    P: PlayerController<Player = G::Player, Arguments = G::Arguments>,
{
    minimized: bool,
    time: Instant,
    next_frame_time: Option<Instant>,
    gui: GuiController<G>,
    player: P,
}
impl<G, P> RuffleWindow<G, P>
where
    G: RuffleGui,
    P: PlayerController<Player = G::Player, Arguments = G::Arguments>,
{
    pub fn new(gui: GuiController<G>, player: P) -> Self {
        RuffleWindow {
            time: Instant::now(),
            next_frame_time: None,
            minimized: false,
            player,
            gui,
        }
    }
    pub fn create_movie(&mut self, arguments: &G::Arguments) {
        self.gui.create_movie(&mut self.player, arguments);
    }
    pub fn window_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) -> bool {
        if matches!(event, WindowEvent::RedrawRequested) {
            // Don't render when minimized to avoid potential swap chain errors in `wgpu`.
            if !self.minimized {
                if let Some(mut player) = self.player.get() {
                    // Even if the movie is paused, user interaction with debug tools can change the render output
                    player.render();
                    self.gui.render(Some(player));
                } else {
                    self.gui.render(None);
                }
                self.gui
                    .gui()
                    .after_render(&self.gui.descriptors().wgpu_instance);
            }

            // Important that we return here, or we'll get a feedback loop with egui
            // (winit says redraw, egui hears redraw and says redraw, we hear redraw and tell winit to redraw...)
            return true;
        }

        if self.gui.handle_event(&event) {
            // Event consumed by GUI.
            return true;
        }
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                // TODO: Change this when winit adds a `Window::minimized` or `WindowEvent::Minimize`.
                self.minimized = size.width == 0 && size.height == 0;

                if let Some(mut player) = self.player.get() {
                    let viewport_scale_factor = self.gui.window().scale_factor();
                    player.set_viewport_dimensions(ViewportDimensions {
                        width: size.width,
                        height: size
                            .height
                            .saturating_sub(self.gui.height_offset_scaled() as u32),
                        scale_factor: viewport_scale_factor,
                    });
                }
                self.gui.window().request_redraw();
            }
            _ => (),
        }

        false
    }

    pub fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Core loop
        // [NA] This used to be called `MainEventsCleared`, but I think the behaviour is different now.
        // We should look at changing our tick to happen somewhere else if we see any behavioural problems.
        let new_time = Instant::now();
        let dt = new_time.duration_since(self.time).as_nanos();
        if dt > 0 {
            self.time = new_time;
            if let Some(mut player) = self.player.get() {
                player.tick(dt as f64 / 1_000_000.0);
                self.next_frame_time = match player.time_til_next_frame() {
                    Some(time_til_next_frame) => Some(new_time + time_til_next_frame),
                    None => None,
                }
            } else {
                self.next_frame_time = None;
            }
            self.check_redraw();
        }

        // The event loop is finished; let's find out how long we need to wait for.
        // We don't need to worry about earlier update requests, as it's the
        // only place where we're setting control flow, and events cancel wait.
        // Note: the control flow might be set to `ControlFlow::WaitUntil` with a
        // timestamp in the past! Take that into consideration when changing this code.
        if let Some(next_frame_time) = self.next_frame_time {
            event_loop.set_control_flow(ControlFlow::WaitUntil(next_frame_time));
        }
    }

    pub fn check_redraw(&self) {
        if self.gui.needs_render() {
            self.gui.window().request_redraw();
        }
    }
}
