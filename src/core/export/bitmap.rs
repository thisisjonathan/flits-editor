use std::{io::Write, path::PathBuf};

use image::{io::Reader as ImageReader, EncodableLayout};
use swf::{
    BitmapFormat, DefineBitsLossless, FillStyle, Fixed16, Matrix, PlaceObject, PlaceObjectAction,
    Point, PointDelta, Rectangle, Shape, ShapeFlag, ShapeRecord, ShapeStyles, Sprite,
    StyleChangeData, Tag, Twips,
};

use crate::core::{Bitmap, SymbolIndex};

use super::{Arenas, SwfBuilder, SwfBuilderExportedAsset, SwfBuilderTag};

pub(super) fn build_bitmap<'a>(
    symbol_index: SymbolIndex,
    bitmap: &Bitmap,
    swf_builder: &mut SwfBuilder<'a>,
    _arenas: &'a Arenas,
    directory: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: the images are probably already loaded when exporting a movie you are editing, maybe reuse that?
    let img = ImageReader::open(directory.join(bitmap.properties.path.clone()))
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => {
                format!("File not found: '{}'", bitmap.properties.path.clone())
            }
            _ => format!(
                "Unable to open file: '{}' Reason: {}",
                bitmap.properties.path.clone(),
                err
            ),
        })?
        .decode()
        .map_err(|err| {
            format!(
                "Error decoding '{}': {}",
                bitmap.properties.path.clone(),
                err
            )
        })?;

    let frame_count = match &bitmap.properties.animation {
        None => 1,
        Some(animation) => animation.frame_count,
    };
    let frames_per_animation_frame = match &bitmap.properties.animation {
        None => 1,
        Some(animation) => animation.frame_delay + 1,
    };
    let image_width = img.width();
    let image_height = img.height();
    if frame_count > image_width {
        return Err(format!(
            "Animation has more frames than width, causing frames to be less than 1 pixel: '{}'",
            bitmap.properties.path.clone()
        )
        .into());
    }
    let frame_width = image_width / frame_count;
    let frame_height = image_height;
    let rgba8 = img.into_rgba8();
    let image_data = &mut rgba8.as_bytes().to_owned();
    // convert to argb
    for i in 0..image_width {
        for j in 0..image_height {
            let index: usize = ((i + j * image_width) * 4) as usize;
            let r = image_data[index];
            let g = image_data[index + 1];
            let b = image_data[index + 2];
            let a = image_data[index + 3];

            // Flash player expects premultiplied alpha
            // see: https://open-flash.github.io/mirrors/swf-spec-19.pdf
            // Chapter 8 -> DefineBitsLossless2 -> ALPHACOLORMAPDATA
            // quote: "The RGB data must already be multiplied bythe alpha channel value."
            // (original includes typo)
            let a_float = a as f32 / 255.0;
            image_data[index] = a;
            image_data[index + 1] = (r as f32 * a_float) as u8;
            image_data[index + 2] = (g as f32 * a_float) as u8;
            image_data[index + 3] = (b as f32 * a_float) as u8;
        }
    }

    for frame_nr in 0..frame_count {
        let compressed_image_data_buffer = Vec::new();
        let mut encoder = flate2::write::ZlibEncoder::new(
            compressed_image_data_buffer,
            flate2::Compression::best(),
        );
        if frame_count == 1 {
            encoder.write_all(image_data)?;
        } else {
            let mut frame_data = vec![0; (frame_width * frame_height * 4) as usize];
            for i in 0..frame_width {
                for j in 0..frame_height {
                    let image_index: usize =
                        ((i + j * image_width + frame_nr * frame_width) * 4) as usize;
                    let frame_index: usize = ((i + j * frame_width) * 4) as usize;
                    let a = image_data[image_index];
                    let r = image_data[image_index + 1];
                    let g = image_data[image_index + 2];
                    let b = image_data[image_index + 3];
                    frame_data[frame_index] = a;
                    frame_data[frame_index + 1] = r;
                    frame_data[frame_index + 2] = g;
                    frame_data[frame_index + 3] = b;
                }
            }
            encoder.write_all(&frame_data)?;
        }
        let mut compressed_image_data = encoder.finish()?;
        // small images disappear in Flash player
        // swfmill solves it with:
        // if( compressed_size < 1024 ) compressed_size = 1024;
        // source: https://github.com/djcsdy/swfmill/blob/53d769029adc9d817972e1ccd648b7b335bf78b7/src/swft/swft_import_png.cpp#L217
        // from expirimentation it seems the real limit on my pc is 256,
        // but let's do the same thing as swfmill to be safe
        if compressed_image_data.len() < 1024 {
            let mut zeros = vec![0; 1024 - compressed_image_data.len()];
            compressed_image_data.append(&mut zeros);
        }

        let bitmap_id = swf_builder.next_character_id();
        let shape_id = swf_builder.next_character_id();
        if frame_count == 1 {
            swf_builder
                .symbol_index_to_character_id
                .insert(symbol_index, shape_id);
            swf_builder
                .symbol_index_to_tag_index
                .insert(symbol_index, swf_builder.tags.len());
        }
        swf_builder.tags.extend(vec![
            SwfBuilderTag::Tag(Tag::DefineBitsLossless(DefineBitsLossless {
                version: 2,
                id: bitmap_id,
                format: BitmapFormat::Rgb32,
                width: frame_width as u16,
                height: frame_height as u16,
                data: std::borrow::Cow::from(compressed_image_data),
            })),
            SwfBuilderTag::Tag(Tag::DefineShape(Shape {
                version: 1,
                id: shape_id,
                shape_bounds: Rectangle {
                    x_min: Twips::from_pixels(0.0),
                    y_min: Twips::from_pixels(0.0),
                    x_max: Twips::from_pixels(frame_width as f64),
                    y_max: Twips::from_pixels(frame_height as f64),
                },
                edge_bounds: Rectangle {
                    x_min: Twips::from_pixels(0.0),
                    y_min: Twips::from_pixels(0.0),
                    x_max: Twips::from_pixels(frame_width as f64),
                    y_max: Twips::from_pixels(frame_height as f64),
                },
                flags: ShapeFlag::empty(),
                styles: ShapeStyles {
                    /*fill_styles: vec![FillStyle::Color(Color {
                        r: 0,
                        g: 0,
                        b: 255,
                        a: 255,
                    })],*/
                    fill_styles: vec![FillStyle::Bitmap {
                        id: bitmap_id,
                        matrix: Matrix::scale(Fixed16::from_f64(20.0), Fixed16::from_f64(20.0)),
                        is_repeating: false,
                        is_smoothed: false,
                    }],
                    line_styles: vec![],
                },
                shape: vec![
                    ShapeRecord::StyleChange(Box::new(StyleChangeData {
                        move_to: Some(Point::new(
                            Twips::from_pixels(frame_width as f64),
                            Twips::from_pixels(frame_height as f64),
                        )),
                        fill_style_0: None,
                        fill_style_1: Some(1),
                        line_style: None,
                        new_styles: None,
                    })),
                    ShapeRecord::StraightEdge {
                        delta: PointDelta {
                            dx: Twips::from_pixels(-(frame_width as f64)),
                            dy: Twips::from_pixels(0.0),
                        },
                    },
                    ShapeRecord::StraightEdge {
                        delta: PointDelta {
                            dx: Twips::from_pixels(0.0),
                            dy: Twips::from_pixels(-(frame_height as f64)),
                        },
                    },
                    ShapeRecord::StraightEdge {
                        delta: PointDelta {
                            dx: Twips::from_pixels(frame_width as f64),
                            dy: Twips::from_pixels(0.0),
                        },
                    },
                    ShapeRecord::StraightEdge {
                        delta: PointDelta {
                            dx: Twips::from_pixels(0.0),
                            dy: Twips::from_pixels(frame_height as f64),
                        },
                    },
                ],
            })),
        ]);
    }

    if frame_count > 1 {
        let movieclip_id = swf_builder.next_character_id();
        swf_builder
            .symbol_index_to_character_id
            .insert(symbol_index, movieclip_id);
        swf_builder
            .symbol_index_to_tag_index
            .insert(symbol_index, swf_builder.tags.len());
        let mut tags = Vec::with_capacity(frame_count as usize);
        for frame_nr in 0..frame_count {
            let character_id = movieclip_id - (frame_count * 2) as u16 + (frame_nr * 2) as u16 + 1;
            tags.push(Tag::PlaceObject(Box::new(PlaceObject {
                version: 2,
                // place on first frame, replace on the other ones
                action: if frame_nr == 0 {
                    PlaceObjectAction::Place(character_id)
                } else {
                    PlaceObjectAction::Replace(character_id)
                },
                depth: 1,
                // use the coordinates as the center of bitmaps instead of the top left
                matrix: Some(Matrix::translate(
                    Twips::from_pixels(frame_width as f64 / -2.0),
                    Twips::from_pixels(frame_height as f64 / -2.0),
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
            for _ in 0..frames_per_animation_frame {
                tags.push(Tag::ShowFrame);
            }
        }
        let end_action: Option<String> = match &bitmap.properties.animation {
            Some(animation) => {
                if animation.end_action.is_empty() {
                    None
                } else {
                    Some(animation.end_action.clone())
                }
            }
            None => None,
        };
        let sprite = Sprite {
            id: movieclip_id,
            num_frames: (frame_count * frames_per_animation_frame) as u16,
            tags,
        };
        let swf_builder_tag = match end_action {
            Some(action_str) => SwfBuilderTag::DefineSpriteWithEndAction(sprite, action_str),
            None => SwfBuilderTag::Tag(Tag::DefineSprite(sprite)),
        };
        swf_builder.tags.push(swf_builder_tag);

        // export all movieclips, this allows you to create them with attachMovie
        // this is easier than having to remember to check a box for each one
        // TODO: is there a reason not to do this?
        swf_builder
            .tags
            .push(SwfBuilderTag::ExportAssets(SwfBuilderExportedAsset {
                character_id: movieclip_id,
                name: bitmap.properties.name.clone(),
            }));
    }

    Ok(())
}
