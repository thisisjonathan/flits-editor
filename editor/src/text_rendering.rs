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
    fn build<'a>(&self) -> Box<dyn SwfFontsContainer + 'a> {
        Box::new(FontsConverter::new(
            self.fonts.clone(),
            self.directory.clone(),
        ))
    }
}
pub struct FontsConverter {
    fonts: Vec<(usize, FlitsFont)>,
    directory: PathBuf,
    font_container: FontContainer,
}
impl FontsConverter {
    pub fn new(fonts: Vec<(usize, FlitsFont)>, directory: PathBuf) -> Self {
        FontsConverter {
            fonts,
            directory,
            font_container: FontContainer::new(),
        }
    }
}
impl SwfFontsContainer for FontsConverter {
    fn convert_fonts(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.font_container
            .convert_fonts(&self.fonts, self.directory.clone())
    }
    fn get_fonts<'a>(&'a self) -> Vec<swf::Font<'a>> {
        self.font_container.fonts()
    }

    fn convert_edit_text<'a>(
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
