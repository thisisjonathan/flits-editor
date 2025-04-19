use crate::desktop::cli::Opt;
use crate::desktop::custom_event::RuffleEvent;
use crate::desktop::executor::GlutinAsyncExecutor;
use crate::desktop::gui::MovieView;
use crate::editor::Editor;
use crate::{RENDER_INFO, SWF_INFO};
use anyhow::anyhow;
use ruffle_render::backend::RenderBackend;
use ruffle_render_wgpu::backend::WgpuRenderBackend;
use ruffle_render_wgpu::descriptors::Descriptors;
use std::rc::Rc;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use url::Url;
use winit::event_loop::EventLoopProxy;
use winit::window::Window;

struct ActivePlayer {
    player: Arc<Mutex<Editor>>,
    executor: Arc<Mutex<GlutinAsyncExecutor>>,
}

impl ActivePlayer {
    pub fn new(
        opt: &Opt,
        event_loop: EventLoopProxy<RuffleEvent>,
        movie_url: Url,
        window: Arc<Window>,
        descriptors: Arc<Descriptors>,
        movie_view: MovieView,
    ) -> Self {
        /*let mut builder = PlayerBuilder::new();

        match CpalAudioBackend::new() {
            Ok(mut audio) => {
                audio.set_volume(opt.volume);
                builder = builder.with_audio(audio);
            }
            Err(e) => {
                tracing::error!("Unable to create audio device: {}", e);
            }
        };*/

        let executor = GlutinAsyncExecutor::new(event_loop.clone());
        /*let navigator = ExternalNavigatorBackend::new(
            opt.base.to_owned().unwrap_or_else(|| movie_url.clone()),
            channel,
            event_loop.clone(),
            opt.proxy.clone(),
            opt.upgrade_to_https,
            opt.open_url_mode,
        );

        if cfg!(feature = "software_video") {
            builder =
                builder.with_video(ruffle_video_software::backend::SoftwareVideoBackend::new());
        }*/

        let renderer = WgpuRenderBackend::new(descriptors, movie_view)
            .map_err(|e| anyhow!(e.to_string()))
            .expect("Couldn't create wgpu rendering backend");
        /*RENDER_INFO.with(|i| *i.borrow_mut() = Some(renderer.debug_info().to_string()));

        builder = builder
            .with_navigator(navigator)
            .with_renderer(renderer)
            .with_storage(DiskStorageBackend::new().expect("Couldn't create storage backend"))
            .with_ui(DesktopUiBackend::new(window.clone()).expect("Couldn't create ui backend"))
            .with_autoplay(true)
            .with_letterbox(opt.letterbox)
            .with_max_execution_duration(Duration::from_secs_f64(opt.max_execution_duration))
            .with_quality(opt.quality)
            .with_warn_on_unsupported_content(!opt.dont_warn_on_unsupported_content)
            .with_scale_mode(opt.scale, opt.force_scale)
            .with_fullscreen(opt.fullscreen)
            .with_load_behavior(opt.load_behavior)
            .with_spoofed_url(opt.spoof_url.clone().map(|url| url.to_string()))
            .with_player_version(opt.player_version)
            .with_frame_rate(opt.frame_rate);
        let player = builder.build();*/

        let name = movie_url
            .path_segments()
            .and_then(|segments| {
                // show the directory the project is in
                segments
                    .rev()
                    .skip_while(|segment| segment.ends_with(".json"))
                    .nth(0)
            })
            .unwrap_or_else(|| movie_url.as_str())
            .to_string();

        window.set_title(&format!("{name} - Flits Editor"));

        /*SWF_INFO.with(|i| *i.borrow_mut() = Some(name.clone()));

        let on_metadata = move |swf_header: &ruffle_core::swf::HeaderExt| {
            let _ = event_loop.send_event(RuffleEvent::OnMetadata(swf_header.clone()));
        };

        let mut parameters: Vec<(String, String)> = movie_url.query_pairs().into_owned().collect();
        parameters.extend(opt.parameters());

        {
            let mut player_lock = player.lock().expect("Player lock must be available");
            CALLSTACK.with(|callstack| {
                *callstack.borrow_mut() = Some(player_lock.callstack());
            });
            player_lock.fetch_root_movie(movie_url.to_string(), parameters, Box::new(on_metadata));
        }*/

        let player = Arc::new(Mutex::new(Editor::new(
            Box::new(renderer),
            movie_url.to_file_path().expect("Invalid movie path"),
        )));

        Self { player, executor }
    }
}

pub struct PlayerController {
    player: Option<ActivePlayer>,
    event_loop: EventLoopProxy<RuffleEvent>,
    window: Arc<Window>,
    descriptors: Arc<Descriptors>,
}

impl PlayerController {
    pub fn new(
        event_loop: EventLoopProxy<RuffleEvent>,
        window: Arc<Window>,
        descriptors: Arc<Descriptors>,
    ) -> Self {
        Self {
            player: None,
            event_loop,
            window,
            descriptors,
        }
    }

    pub fn create(&mut self, opt: &Opt, movie_url: Url, movie_view: MovieView) {
        self.player = Some(ActivePlayer::new(
            opt,
            self.event_loop.clone(),
            movie_url,
            self.window.clone(),
            self.descriptors.clone(),
            movie_view,
        ));
    }

    pub fn destroy(&mut self) {
        // set the window title back to the default
        self.window.set_title("Flits Editor");
        self.player = None;
    }

    pub fn get(&self) -> Option<MutexGuard<Editor>> {
        match &self.player {
            None => None,
            // We don't want to return None when the lock fails to grab as that's a fatal error, not a lack of player
            Some(player) => Some(
                player
                    .player
                    .try_lock()
                    .expect("Player lock must be available"),
            ),
        }
    }

    pub fn poll(&self) {
        /*if let Some(player) = &self.player {
            player
            .executor
            .lock()
            .expect("Executor lock must be available")
            .poll_all()
        }*/
    }
}
