use ansi_parser::{AnsiParser, AnsiSequence};
use egui::{Color32, FontFamily, RichText};

pub(crate) struct OutputWindow {
    lines: Vec<String>,
}
impl OutputWindow {
    pub fn new() -> Self {
        OutputWindow { lines: vec![] }
    }
    pub fn do_ui(&mut self, egui_ctx: &egui::Context) {
        egui::CentralPanel::default().show(egui_ctx, |ui| {
            let text_style = egui::TextStyle::Body;
            let row_height = ui.text_style_height(&text_style);
            let num_rows = self.lines.len();
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .stick_to_bottom(true)
                .scroll(true)
                .show_rows(ui, row_height, num_rows, |ui, row_range| {
                    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                    for row in row_range {
                        // TODO: caching of parsing
                        ui.horizontal(|ui| {
                            let mut color = Color32::WHITE;
                            for output in self.lines[row].ansi_parse() {
                                match output {
                                    ansi_parser::Output::TextBlock(text) => {
                                        ui.label(
                                            RichText::new(text)
                                                .color(color)
                                                .family(FontFamily::Monospace),
                                        );
                                    }
                                    ansi_parser::Output::Escape(seq) => match seq {
                                        AnsiSequence::SetGraphicsMode(mode) => {
                                            //ui.label(format!("({:?})", mode));
                                            // only codes that i've seen Ruffle use are implemented
                                            match mode[0] {
                                                0 => {
                                                    // reset
                                                    color = Color32::WHITE;
                                                }
                                                2 => {
                                                    // dim
                                                    color = Color32::GRAY;
                                                }
                                                31 => {
                                                    // foreground red
                                                    color = Color32::RED;
                                                }
                                                32 => {
                                                    // foreground green
                                                    color = Color32::GREEN;
                                                }
                                                33 => {
                                                    // foreground yellow
                                                    color = Color32::YELLOW;
                                                }
                                                _ => {}
                                            }
                                        }
                                        _ => {}
                                    },
                                }
                            }
                        });
                    }
                });
        });
    }
    pub fn add_line(&mut self, line: String) {
        // TODO: use circular buffer?
        self.lines.push(line);
    }
}
