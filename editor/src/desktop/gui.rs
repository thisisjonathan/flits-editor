mod controller;
mod movie;

pub use controller::GuiController;
pub use movie::MovieView;
use std::borrow::Cow;
use std::path::PathBuf;

use crate::desktop::custom_event::{NewProjectData, RuffleEvent};
use crate::editor::{Editor, NeedsRedraw};
use chrono::DateTime;
use egui::*;
use flits_core::Movie;
use fluent_templates::fluent_bundle::FluentValue;
use fluent_templates::loader::langid;
use fluent_templates::{static_loader, Loader};
use std::collections::HashMap;
use sys_locale::get_locale;
use unic_langid::LanguageIdentifier;
use winit::event_loop::EventLoopProxy;

static US_ENGLISH: LanguageIdentifier = langid!("en-US");

static_loader! {
    static TEXTS = {
        locales: "./assets/texts",
        fallback_language: "en-US"
    };
}

pub fn text<'a>(locale: &LanguageIdentifier, id: &'a str) -> Cow<'a, str> {
    TEXTS
        .try_lookup(locale, id)
        .map(Cow::Owned)
        .unwrap_or_else(|| {
            tracing::error!("Unknown desktop text id '{id}'");
            Cow::Borrowed(id)
        })
}

#[allow(dead_code)]
pub fn text_with_args<'a, T: AsRef<str>>(
    locale: &LanguageIdentifier,
    id: &'a str,
    args: &HashMap<T, FluentValue>,
) -> Cow<'a, str> {
    TEXTS
        .try_lookup_with_args(locale, id, args)
        .map(Cow::Owned)
        .unwrap_or_else(|| {
            tracing::error!("Unknown desktop text id '{id}'");
            Cow::Borrowed(id)
        })
}

/// Size of the top menu bar in pixels.
/// This is the offset at which the movie will be shown,
/// and added to the window size if trying to match a movie.
pub const MENU_HEIGHT: u32 = 48;

/// The main controller for the Ruffle GUI.
pub struct RuffleGui {
    event_loop: EventLoopProxy<RuffleEvent>,
    is_about_visible: bool,
    new_project: Option<NewProjectData>,
    locale: LanguageIdentifier,
}

impl RuffleGui {
    fn new(event_loop: EventLoopProxy<RuffleEvent>) -> Self {
        // TODO: language negotiation + https://github.com/1Password/sys-locale/issues/14
        // This should also be somewhere else so it can be supplied through UiBackend too

        let preferred_locale = get_locale();
        let locale = preferred_locale
            .and_then(|l| l.parse().ok())
            .unwrap_or_else(|| US_ENGLISH.clone());

        Self {
            event_loop,
            is_about_visible: false,
            new_project: None,
            locale,
        }
    }

    /// Renders all of the main Ruffle UI, including the main menu and context menus.
    fn update(
        &mut self,
        egui_ctx: &egui::Context,
        _show_menu: bool,
        player: Option<&mut Editor>,
    ) -> NeedsRedraw {
        let mut needs_redraw = NeedsRedraw::No;
        if let Some(player) = player {
            needs_redraw = player.do_ui(egui_ctx, &self.event_loop);
        } else {
            self.show_welcome_screen(egui_ctx);
        }

        // windows must be after panels
        self.new_project_window(egui_ctx);
        self.about_window(egui_ctx);

        needs_redraw
    }

    fn show_welcome_screen(&mut self, egui_ctx: &egui::Context) {
        egui::Window::new("Welcome")
            .collapsible(false)
            .title_bar(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(egui_ctx, |ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(200.0, 100.0),
                    egui::Layout::top_down_justified(egui::Align::Center),
                    |ui| {
                        ui.style_mut().override_text_style = Some(egui::TextStyle::Heading);
                        if ui.button("New project...").clicked() {
                            self.open_new_project_window();
                        }
                        if ui.button("Open project...").clicked() {
                            let _ = self.event_loop.send_event(RuffleEvent::OpenFile);
                        }
                        if ui.button("About...").clicked() {
                            let _ = self.event_loop.send_event(RuffleEvent::About);
                        }
                    },
                );
            });
    }

    fn open_new_project_window(&mut self) {
        self.new_project = Some(NewProjectData::default());
    }

    fn new_project_window(&mut self, egui_ctx: &egui::Context) {
        if self.new_project.is_some() {
            let event_loop = &self.event_loop;

            let mut is_window_open = true;
            egui::Window::new("New project")
                .collapsible(false)
                .resizable(false)
                .open(&mut is_window_open)
                .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(egui_ctx, |ui| {
                    egui::Grid::new("movie_properties_grid").show(ui, |ui| {
                        let new_project = self.new_project.as_mut().unwrap();
                        ui.label("Directory:");
                        ui.label(new_project.path.to_str().unwrap());
                        if ui.button("Change...").clicked() {
                            if let Some(directory) = rfd::FileDialog::new().pick_folder() {
                                new_project.path = directory;
                            }
                        }
                        ui.end_row();

                        ui.label("Width:");
                        ui.add(egui::DragValue::new(
                            &mut new_project.movie_properties.width,
                        ));
                        ui.end_row();

                        ui.label("Height:");
                        ui.add(egui::DragValue::new(
                            &mut new_project.movie_properties.height,
                        ));
                        ui.end_row();

                        ui.label("Framerate:");
                        ui.add(egui::DragValue::new(
                            &mut new_project.movie_properties.frame_rate,
                        ));
                        ui.end_row();

                        if ui
                            .add_enabled(
                                !new_project.path.to_str().unwrap().is_empty(),
                                egui::Button::new("Create"),
                            )
                            .clicked()
                        {
                            let _ =
                                event_loop.send_event(RuffleEvent::NewFile(new_project.clone()));
                            self.new_project = None;
                        }
                        ui.end_row();
                    });
                });
            if !is_window_open {
                self.new_project = None;
            }
        }
    }

    fn about_window(&mut self, egui_ctx: &egui::Context) {
        egui::Window::new("About Flits Editor")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .open(&mut self.is_about_visible)
            .show(egui_ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("Flits Editor")
                            //.color(Color32::BLUE)
                            .size(32.0),
                    );
                    ui.label("Preview build");
                    /*Grid::new("about_ruffle_version_info")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label(text(&self.locale, "about-ruffle-version"));
                            ui.label(env!("CARGO_PKG_VERSION"));
                            ui.end_row();

                            ui.label(text(&self.locale, "about-ruffle-channel"));
                            ui.label(env!("CFG_RELEASE_CHANNEL"));
                            ui.end_row();

                            ui.label(text(&self.locale, "about-ruffle-build-time"));
                            ui.label(
                                /*DateTime::parse_from_rfc3339(env!("VERGEN_BUILD_TIMESTAMP"))
                                .map(|t| t.format("%c").to_string())
                                .unwrap_or_else(|_|*/
                                env!("VERGEN_BUILD_TIMESTAMP").to_string(), //),
                            );
                            ui.end_row();

                            ui.label(text(&self.locale, "about-ruffle-commit-ref"));
                            ui.hyperlink_to(
                                env!("VERGEN_GIT_SHA"),
                                format!(
                                    "https://github.com/ruffle-rs/ruffle/commit/{}",
                                    env!("VERGEN_GIT_SHA")
                                ),
                            );
                            ui.end_row();

                            ui.label(text(&self.locale, "about-ruffle-commit-time"));
                            ui.label(
                                /*DateTime::parse_from_rfc3339(env!("VERGEN_GIT_COMMIT_TIMESTAMP"))
                                .map(|t| t.format("%c").to_string())
                                .unwrap_or_else(|_| {*/
                                env!("VERGEN_GIT_COMMIT_TIMESTAMP").to_string(), //}),
                            );
                            ui.end_row();

                            ui.label(text(&self.locale, "about-ruffle-build-features"));
                            ui.horizontal_wrapped(|ui| {
                                ui.label(env!("VERGEN_CARGO_FEATURES").replace(',', ", "));
                            });
                            ui.end_row();
                        });

                    ui.horizontal(|ui| {
                        ui.hyperlink_to(
                            text(&self.locale, "about-ruffle-visit-website"),
                            "https://ruffle.rs",
                        );
                        ui.hyperlink_to(
                            text(&self.locale, "about-ruffle-visit-github"),
                            "https://github.com/ruffle-rs/ruffle/",
                        );
                        ui.hyperlink_to(
                            text(&self.locale, "about-ruffle-visit-discord"),
                            "https://discord.gg/ruffle",
                        );
                        ui.hyperlink_to(
                            text(&self.locale, "about-ruffle-visit-sponsor"),
                            "https://opencollective.com/ruffle/",
                        );
                        ui.shrink_width_to_current();
                    });*/
                })
            });
    }

    pub fn show_about_screen(&mut self) {
        self.is_about_visible = true;
    }
}
