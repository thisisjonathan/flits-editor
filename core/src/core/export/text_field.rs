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
        .with_font_id(font_character_id, Twips::from_pixels(text.size))
        .with_bounds(Rectangle {
            x_min: Twips::from_pixels(0.0),
            x_max: Twips::from_pixels(text.width),
            y_min: Twips::from_pixels(0.0),
            y_max: Twips::from_pixels(text.height),
        })
        .with_color(Some(text.color.clone().into()))
        .with_layout(Some(swf::TextLayout {
            align: text.align.clone().into(),
            left_margin: Twips::ZERO,
            right_margin: Twips::ZERO,
            indent: Twips::ZERO,
            leading: Twips::ZERO,
        }))
        // TODO: check if the font supports all the characters in the initial text?
        .with_initial_text(Some(arenas.alloc_swf_string(text.text.clone())))
        .with_is_read_only(!text.editable)
        .with_is_selectable(text.selectable)
        .with_is_password(text.is_password)
        .with_is_html(text.is_html)
        .with_is_multiline(text.is_multiline)
        .with_is_word_wrap(text.word_wrap)
        .with_use_outlines(true); // enables embedded fonts
    swf_builder
        .tags
        .push(Tag::DefineEditText(Box::new(edit_text)));

    edit_text_id
}
