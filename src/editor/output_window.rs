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
                .show_rows(ui, row_height, num_rows, |ui, row_range| {
                    for row in row_range {
                        // TODO: handle colored output
                        ui.label(&self.lines[row]);
                    }
                });
        });
    }
    pub fn add_line(&mut self, line: String) {
        // TODO: limit capacity
        self.lines.push(line);
    }
}
