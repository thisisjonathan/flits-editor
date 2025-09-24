use std::path::PathBuf;

use flits_core::{FlitsFont, FontContainer};
use flits_text_rendering::{SwfFontsContainer, SwfFontsContainerBuilder};
use swf::EditText;

pub struct FontsConverterBuilder {
    pub fonts: Vec<(usize, FlitsFont)>,
    pub directory: PathBuf,
}
impl FontsConverterBuilder {
    pub fn new(fonts: Vec<(usize, FlitsFont)>, directory: PathBuf) -> Self {
        FontsConverterBuilder { fonts, directory }
    }
}
impl SwfFontsContainerBuilder for FontsConverterBuilder {
    fn build<'a>(&self) -> Box<dyn SwfFontsContainer<'a> + 'a> {
        Box::new(FontsConverter::new(
            self.fonts.clone(),
            self.directory.clone(),
        ))
    }
}
pub struct FontsConverter<'a> {
    fonts: Vec<(usize, FlitsFont)>,
    directory: PathBuf,
    font_container: FontContainer<'a>,
}
impl<'a> FontsConverter<'a> {
    pub fn new(fonts: Vec<(usize, FlitsFont)>, directory: PathBuf) -> Self {
        FontsConverter {
            fonts,
            directory,
            font_container: FontContainer::new(),
        }
    }
}
impl<'a> SwfFontsContainer<'a> for FontsConverter<'a> {
    fn convert_fonts(&'a mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.font_container
            .convert_fonts(&self.fonts, self.directory.clone())
    }
    fn get_fonts<'b>(&'b self) -> &'b Vec<swf::Font<'b>> {
        self.font_container.fonts()
    }

    fn convert_edit_text(
        &'a mut self,
        properties: Box<dyn std::any::Any>,
    ) -> Result<EditText<'a>, Box<dyn std::error::Error>> {
        let Ok(properties) = properties.downcast() else {
            return Err("".into());
        };
        let (font_symbol_index, edit_text_properties) = *properties;
        self.font_container
            .convert_text_field(font_symbol_index, edit_text_properties)
    }
}
