use egui::Widget;
use flits_core::Movie;

use crate::{editor::Selection, message::EditorMessage, message_bus::MessageBus};

pub struct Menu<'a> {
    pub name: &'a str,
    pub items: &'a [MenuItem<'a>],
}

pub struct MenuItem<'a> {
    pub name: &'a str,
    pub keyboard_shortcut: Option<egui::KeyboardShortcut>,
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
            },
            MenuItem {
                name: "Save",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::S,
                )),
            },
            MenuItem {
                name: "Export",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::E,
                )),
            },
            MenuItem {
                name: "Close",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::W,
                )),
            },
            MenuItem {
                name: "Exit",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::Q,
                )),
            },
        ],
    },
    Menu {
        name: "Edit",
        items: &[
            MenuItem {
                name: "Undo",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::Z,
                )),
            },
            MenuItem {
                name: "Redo",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::R,
                )),
            },
            MenuItem {
                name: "Delete",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::NONE,
                    egui::Key::Delete,
                )),
            },
            MenuItem {
                name: "Select all",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::A,
                )),
            },
            MenuItem {
                name: "Reload assets",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::NONE,
                    egui::Key::F5,
                )),
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
            },
            MenuItem {
                name: "Zoom out",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::NONE,
                    egui::Key::Minus,
                )),
            },
            MenuItem {
                name: "Reset zoom",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::NONE,
                    egui::Key::Num0,
                )),
            },
        ],
    },
    Menu {
        name: "Control",
        items: &[MenuItem {
            name: "Test Movie",
            keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::Enter,
            )),
        }],
    },
    Menu {
        name: "Help",
        items: &[MenuItem {
            name: "About...",
            keyboard_shortcut: None,
        }],
    },
];

#[derive(Default)]
pub struct MenuBar {}
impl MenuBar {
    pub fn do_ui(
        &mut self,
        ui: &mut egui::Ui,
        movie: &Movie,
        selection: &Selection,
        message_bus: &MessageBus<EditorMessage>,
    ) {
        // this isn't just text field, also buttons and such
        let is_something_focused = ui.ctx().memory(|memory| memory.focused().is_some());
        for menu in MENUS {
            for item in menu.items {
                if let Some(keyboard_shortcut) = item.keyboard_shortcut {
                    // only activate keyboard shortcuts without modifiers when nothing is focused
                    let can_activate = keyboard_shortcut.modifiers.any() || !is_something_focused;
                    if can_activate
                        && ui
                            .ctx()
                            .input_mut(|input| input.consume_shortcut(&keyboard_shortcut))
                    {
                        //(item.action)(self, event_loop);
                        ui.close_menu();
                    }
                }
            }
        }

        egui::menu::bar(ui, |ui| {
            for menu in MENUS {
                egui::menu::menu_button(ui, menu.name, |ui| {
                    for item in menu.items {
                        let mut button = egui::Button::new(item.name);
                        if let Some(keyboard_shortcut) = item.keyboard_shortcut {
                            button =
                                button.shortcut_text(ui.ctx().format_shortcut(&keyboard_shortcut));
                        }
                        if button.ui(ui).clicked() {
                            //(item.action)(self, event_loop);
                            ui.close_menu();
                        }
                    }
                });
            }
        });
    }
}
