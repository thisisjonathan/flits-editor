use winit::event_loop::EventLoopProxy;

use crate::{custom_event::FlitsEvent, editor::Editor};

pub struct Menu<'a> {
    pub name: &'a str,
    pub items: &'a [MenuItem<'a>],
}

pub struct MenuItem<'a> {
    pub name: &'a str,
    pub keyboard_shortcut: Option<egui::KeyboardShortcut>,
    pub action: fn(editor: &mut Editor, event_loop: &EventLoopProxy<FlitsEvent>),
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

fn open_project(_editor: &mut Editor, event_loop: &EventLoopProxy<FlitsEvent>) {
    let _ = event_loop.send_event(FlitsEvent::OpenFile);
}

fn save_project(editor: &mut Editor, _event_loop: &EventLoopProxy<FlitsEvent>) {
    editor.save();
}

fn export_swf(editor: &mut Editor, _event_loop: &EventLoopProxy<FlitsEvent>) {
    // we don't care about the result here, export_swf sets the error message on the editor
    _ = editor.export_swf();
}

fn close_project(_editor: &mut Editor, event_loop: &EventLoopProxy<FlitsEvent>) {
    let _ = event_loop.send_event(FlitsEvent::CloseFile);
}

fn request_exit(_editor: &mut Editor, event_loop: &EventLoopProxy<FlitsEvent>) {
    let _ = event_loop.send_event(FlitsEvent::ExitRequested);
}

fn run_project(editor: &mut Editor, event_loop: &EventLoopProxy<FlitsEvent>) {
    editor.export_and_run(event_loop);
}

fn show_about_screen(_editor: &mut Editor, event_loop: &EventLoopProxy<FlitsEvent>) {
    let _ = event_loop.send_event(FlitsEvent::About);
}

fn undo(editor: &mut Editor, _event_loop: &EventLoopProxy<FlitsEvent>) {
    editor.do_undo();
}
fn redo(editor: &mut Editor, _event_loop: &EventLoopProxy<FlitsEvent>) {
    editor.do_redo();
}

fn delete_selection(editor: &mut Editor, _event_loop: &EventLoopProxy<FlitsEvent>) {
    editor.delete_selection();
}

fn reload_assets(editor: &mut Editor, _event_loop: &EventLoopProxy<FlitsEvent>) {
    editor.reload_assets();
}

fn zoom_in(editor: &mut Editor, _event_loop: &EventLoopProxy<FlitsEvent>) {
    editor.zoom(0.1);
}

fn zoom_out(editor: &mut Editor, _event_loop: &EventLoopProxy<FlitsEvent>) {
    editor.zoom(-0.1);
}

fn reset_zoom(editor: &mut Editor, _event_loop: &EventLoopProxy<FlitsEvent>) {
    editor.reset_zoom();
}
