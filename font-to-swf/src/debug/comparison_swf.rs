use std::path::PathBuf;

use swf::{Tag, Twips};

pub fn create_comparision_swf(
    tags: Vec<Tag>,
    font_character_ids: Vec<(swf::CharacterId, swf::CharacterId)>,
    output_path: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut tags = Vec::from(tags);
    tags.push(Tag::SetBackgroundColor(swf::Color::WHITE));

    let mut index = 0;
    for character_ids in font_character_ids {
        create_textbox(&mut tags, character_ids.0, swf::Color::BLUE, index); // flits
        create_textbox(&mut tags, character_ids.1, swf::Color::RED, index); // swfmill
        index += 1;
    }
    tags.push(Tag::ShowFrame);

    let header = swf::Header {
        compression: swf::Compression::Zlib,
        version: 43, // latest version
        stage_size: swf::Rectangle {
            x_min: Twips::from_pixels(0.0),
            x_max: Twips::from_pixels(640.0),
            y_min: Twips::from_pixels(0.0),
            y_max: Twips::from_pixels(480.0),
        },
        frame_rate: swf::Fixed8::from_f32(60.0),
        num_frames: 1,
    };
    let file = std::fs::File::create(output_path)?;
    let writer = std::io::BufWriter::new(file);
    swf::write_swf(&header, &tags, writer)?;

    Ok(())
}
fn create_textbox(
    tags: &mut Vec<Tag>,
    character_id: swf::CharacterId,
    color: swf::Color,
    index: usize,
) {
    let textbox_character_id = character_id + 100;
    let edit_text = swf::EditText::new()
        .with_id(textbox_character_id)
        .with_font_id(character_id, Twips::from_pixels(50.0))
        .with_bounds(swf::Rectangle {
            // TODO: negative min values might be causing the selection jank in Ruffle?
            x_min: Twips::from_pixels(0.0),
            x_max: Twips::from_pixels(640.0),
            y_min: Twips::from_pixels(0.0),
            y_max: Twips::from_pixels(100.0),
        })
        .with_color(Some(color))
        .with_layout(Some(swf::TextLayout {
            align: swf::TextAlign::Left,
            left_margin: Twips::ZERO,
            right_margin: Twips::ZERO,
            indent: Twips::ZERO,
            leading: Twips::ZERO,
        }))
        .with_initial_text(Some(swf::SwfStr::from_utf8_str("0123456789 0123456789")))
        .with_is_selectable(true)
        .with_use_outlines(true); // enables embedded fonts
    tags.push(Tag::DefineEditText(Box::new(edit_text)));

    tags.push(Tag::PlaceObject(Box::new(swf::PlaceObject {
        version: 2,
        action: swf::PlaceObjectAction::Place(textbox_character_id),
        depth: character_id,
        matrix: Some(swf::Matrix::translate(
            Twips::ZERO,
            Twips::from_pixels(index as f64 * 100.0),
        )),
        color_transform: None,
        ratio: None,
        name: None,
        clip_depth: None,
        class_name: None,
        filters: None,
        background_color: None,
        blend_mode: None,
        clip_actions: None,
        has_image: true,
        is_bitmap_cached: None,
        is_visible: Some(true),
        amf_data: None,
    })));
}
