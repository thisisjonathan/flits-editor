use swf::{CharacterId, EditText, Rectangle, Tag, Twips};

use crate::TextProperties;

use super::{Arenas, SwfBuilder};

pub(super) fn build_text_field<'a>(
    font_character_id: CharacterId,
    text: &TextProperties,
    swf_builder: &mut SwfBuilder<'a>,
    arenas: &'a Arenas,
) -> CharacterId {
    let edit_text_id = swf_builder.next_character_id();
    let edit_text = EditText::new()
        .with_id(edit_text_id)
        .with_font_id(font_character_id, Twips::from_pixels(50.0))
        .with_bounds(Rectangle {
            x_min: Twips::from_pixels(text.width / -2.0),
            x_max: Twips::from_pixels(text.width / 2.0),
            y_min: Twips::from_pixels(text.height / -2.0),
            y_max: Twips::from_pixels(text.height / 2.0),
        })
        .with_color(Some(swf::Color::RED))
        .with_layout(Some(swf::TextLayout {
            align: swf::TextAlign::Center,
            left_margin: Twips::ZERO,
            right_margin: Twips::ZERO,
            indent: Twips::ZERO,
            leading: Twips::ZERO,
        }))
        .with_initial_text(Some(arenas.alloc_swf_string(text.text.clone())))
        .with_use_outlines(true);
    swf_builder
        .tags
        .push(Tag::DefineEditText(Box::new(edit_text)));

    edit_text_id
}
