use winit::event_loop::EventLoopProxy;

use crate::desktop::custom_event::RuffleEvent;

use super::Editor;

pub struct Menu<'a> {
    pub name: &'a str,
    pub items: &'a [MenuItem<'a>],
}

pub struct MenuItem<'a> {
    pub name: &'a str,
    pub keyboard_shortcut: Option<egui::KeyboardShortcut>,
    pub action: fn(player: &mut Editor, event_loop: &EventLoopProxy<RuffleEvent>),
}

pub const MENUS: &[Menu] = &[
    Menu {
        name: "File",
        items: &[
            MenuItem {
                name: "Open...",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::O,
                )),
                action: open_project,
            },
            MenuItem {
                name: "Save",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::S,
                )),
                action: save_project,
            },
            MenuItem {
                name: "Export",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::E,
                )),
                action: export_swf,
            },
            MenuItem {
                name: "Close",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::W,
                )),
                action: close_project,
            },
            MenuItem {
                name: "Exit",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::Q,
                )),
                action: request_exit,
            },
        ],
    },
    Menu {
        name: "Edit",
        items: &[
            MenuItem {
                name: "Undo",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::CTRL,
                    egui::Key::Z,
                )),
                action: undo,
            },
            MenuItem {
                name: "Redo",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::CTRL,
                    egui::Key::R,
                )),
                action: redo,
            },
            MenuItem {
                name: "Delete",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::NONE,
                    egui::Key::Delete,
                )),
                action: delete_selection,
            },
            MenuItem {
                name: "Reload assets",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::NONE,
                    egui::Key::F5,
                )),
                action: reload_assets,
            },
        ],
    },
    Menu {
        name: "View",
        items: &[
            MenuItem {
                name: "Zoom in",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::NONE,
                    egui::Key::Equals,
                )),
                action: zoom_in,
            },
            MenuItem {
                name: "Zoom out",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::NONE,
                    egui::Key::Minus,
                )),
                action: zoom_out,
            },
            MenuItem {
                name: "Reset zoom",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::NONE,
                    egui::Key::Num0,
                )),
                action: reset_zoom,
            },
        ],
    },
    Menu {
        name: "Control",
        items: &[MenuItem {
            name: "Test Movie",
            keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                egui::Modifiers::CTRL,
                egui::Key::Enter,
            )),
            action: run_project,
        }],
    },
    Menu {
        name: "Help",
        items: &[MenuItem {
            name: "About...",
            keyboard_shortcut: None,
            action: show_about_screen,
        }],
    },
];

fn open_project(_player: &mut Editor, event_loop: &EventLoopProxy<RuffleEvent>) {
    let _ = event_loop.send_event(RuffleEvent::OpenFile);
}

fn save_project(player: &mut Editor, _event_loop: &EventLoopProxy<RuffleEvent>) {
    player.movie.save(&player.project_file_path);
}

fn export_swf(player: &mut Editor, _event_loop: &EventLoopProxy<RuffleEvent>) {
    // we don't care about the result here, export_swf sets the error message on the editor
    _ = player.export_swf();
}

fn close_project(_player: &mut Editor, event_loop: &EventLoopProxy<RuffleEvent>) {
    let _ = event_loop.send_event(RuffleEvent::CloseFile);
}

fn request_exit(_player: &mut Editor, event_loop: &EventLoopProxy<RuffleEvent>) {
    let _ = event_loop.send_event(RuffleEvent::ExitRequested);
}

fn run_project(player: &mut Editor, _event_loop: &EventLoopProxy<RuffleEvent>) {
    player.export_and_run();
}

fn show_about_screen(_player: &mut Editor, event_loop: &EventLoopProxy<RuffleEvent>) {
    let _ = event_loop.send_event(RuffleEvent::About);
}

fn undo(player: &mut Editor, _event_loop: &EventLoopProxy<RuffleEvent>) {
    player.do_undo();
}
fn redo(player: &mut Editor, _event_loop: &EventLoopProxy<RuffleEvent>) {
    player.do_redo();
}

fn delete_selection(player: &mut Editor, _event_loop: &EventLoopProxy<RuffleEvent>) {
    player.delete_selection();
}

fn reload_assets(player: &mut Editor, _event_loop: &EventLoopProxy<RuffleEvent>) {
    player.reload_assets();
}

fn zoom_in(player: &mut Editor, _event_loop: &EventLoopProxy<RuffleEvent>) {
    player.zoom(0.1);
}

fn zoom_out(player: &mut Editor, _event_loop: &EventLoopProxy<RuffleEvent>) {
    player.zoom(-0.1);
}

fn reset_zoom(player: &mut Editor, _event_loop: &EventLoopProxy<RuffleEvent>) {
    player.reset_zoom();
}
