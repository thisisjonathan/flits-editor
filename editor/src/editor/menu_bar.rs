use egui::Widget;

use crate::{editor::stage::StageMessage, editor::Context, message::EditorMessage, FlitsEvent};

struct Menu<'a> {
    name: &'a str,
    items: &'a [MenuItem<'a>],
}

struct MenuItem<'a> {
    name: &'a str,
    keyboard_shortcut: Option<egui::KeyboardShortcut>,
    message: fn() -> EditorMessage,
}

const MENUS: &[Menu] = &[
    Menu {
        name: "File",
        items: &[
            // TODO: New...
            MenuItem {
                name: "Open...",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::O,
                )),
                message: || EditorMessage::Event(FlitsEvent::OpenFile),
            },
            MenuItem {
                name: "Save",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::S,
                )),
                message: || EditorMessage::Save,
            },
            MenuItem {
                name: "Export",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::E,
                )),
                message: || EditorMessage::Export,
            },
            MenuItem {
                name: "Close",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::W,
                )),
                message: || EditorMessage::Event(FlitsEvent::CloseFile),
            },
            MenuItem {
                name: "Exit",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::Q,
                )),
                message: || EditorMessage::Event(FlitsEvent::ExitRequested),
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
                message: || EditorMessage::Undo,
            },
            MenuItem {
                name: "Redo",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::R,
                )),
                message: || EditorMessage::Redo,
            },
            MenuItem {
                name: "Delete",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::NONE,
                    egui::Key::Delete,
                )),
                message: || EditorMessage::DeleteSelection,
            },
            MenuItem {
                name: "Select all",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::A,
                )),
                message: || EditorMessage::SelectAll,
            },
            MenuItem {
                name: "Reload assets",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::NONE,
                    egui::Key::F5,
                )),
                message: || EditorMessage::ReloadAssets,
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
                message: || EditorMessage::Stage(StageMessage::ZoomIn),
            },
            MenuItem {
                name: "Zoom out",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::NONE,
                    egui::Key::Minus,
                )),
                message: || EditorMessage::Stage(StageMessage::ZoomOut),
            },
            MenuItem {
                name: "Reset zoom",
                keyboard_shortcut: Some(egui::KeyboardShortcut::new(
                    egui::Modifiers::NONE,
                    egui::Key::Num0,
                )),
                message: || EditorMessage::Stage(StageMessage::ResetZoom),
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
            message: || EditorMessage::Run,
        }],
    },
    Menu {
        name: "Help",
        items: &[MenuItem {
            name: "About...",
            keyboard_shortcut: None,
            message: || EditorMessage::Event(FlitsEvent::About),
        }],
    },
];

#[derive(Default)]
pub struct MenuBar {}
impl MenuBar {
    pub fn do_ui(&mut self, ui: &mut egui::Ui, ctx: &Context) {
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
                        ctx.message_bus.publish((item.message)());
                        // TODO: reset focus when undoing/redoing
                        // (this may have been fixed as a side effect of the later stages of refactoring)
                        /*ui.memory_mut(|mem| {
                            if let Some(focused_widget) = mem.focused() {
                                mem.surrender_focus(focused_widget);
                            }
                        });*/
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
                            ctx.message_bus.publish((item.message)());
                            ui.close_menu();
                        }
                    }
                });
            }
        });
    }
}
