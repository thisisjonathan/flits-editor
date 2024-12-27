use std::{io::Write, path::PathBuf};

use image::{io::Reader as ImageReader, EncodableLayout};
use swf::{
    FillStyle, Fixed16, Matrix, Point, PointDelta, Rectangle, Shape, ShapeFlag, ShapeRecord,
    ShapeStyles, StyleChangeData, Tag, Twips,
};

use crate::core::{Bitmap, SymbolIndex};

use super::{SwfBuilder, SwfBuilderBitmap, SwfBuilderTag};

pub(super) fn build_bitmap<'a>(
    symbol_index: SymbolIndex,
    bitmap: &Bitmap,
    swf_builder: &mut SwfBuilder,
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
    let image_width = img.width();
    let image_height = img.height();
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
            image_data[index] = a;
            image_data[index + 1] = r;
            image_data[index + 2] = g;
            image_data[index + 3] = b;
        }
    }
    let compressed_image_data_buffer = Vec::new();
    let mut encoder =
        flate2::write::ZlibEncoder::new(compressed_image_data_buffer, flate2::Compression::best());
    encoder.write_all(image_data)?;
    let compressed_image_data = encoder.finish()?;

    let bitmap_id = swf_builder.next_character_id();
    let shape_id = swf_builder.next_character_id();
    swf_builder
        .symbol_index_to_character_id
        .insert(symbol_index, shape_id);
    swf_builder
        .symbol_index_to_tag_index
        .insert(symbol_index, swf_builder.tags.len());
    swf_builder.tags.extend(vec![
        SwfBuilderTag::Bitmap(SwfBuilderBitmap {
            character_id: bitmap_id,
            width: image_width,
            height: image_height,
            data: compressed_image_data,
        }),
        SwfBuilderTag::Tag(Tag::DefineShape(Shape {
            version: 1,
            id: shape_id,
            shape_bounds: Rectangle {
                x_min: Twips::from_pixels(0.0),
                y_min: Twips::from_pixels(0.0),
                x_max: Twips::from_pixels(image_width as f64),
                y_max: Twips::from_pixels(image_height as f64),
            },
            edge_bounds: Rectangle {
                x_min: Twips::from_pixels(0.0),
                y_min: Twips::from_pixels(0.0),
                x_max: Twips::from_pixels(image_width as f64),
                y_max: Twips::from_pixels(image_height as f64),
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
                        Twips::from_pixels(image_width as f64),
                        Twips::from_pixels(image_height as f64),
                    )),
                    fill_style_0: None,
                    fill_style_1: Some(1),
                    line_style: None,
                    new_styles: None,
                })),
                ShapeRecord::StraightEdge {
                    delta: PointDelta {
                        dx: Twips::from_pixels(-(image_width as f64)),
                        dy: Twips::from_pixels(0.0),
                    },
                },
                ShapeRecord::StraightEdge {
                    delta: PointDelta {
                        dx: Twips::from_pixels(0.0),
                        dy: Twips::from_pixels(-(image_height as f64)),
                    },
                },
                ShapeRecord::StraightEdge {
                    delta: PointDelta {
                        dx: Twips::from_pixels(image_width as f64),
                        dy: Twips::from_pixels(0.0),
                    },
                },
                ShapeRecord::StraightEdge {
                    delta: PointDelta {
                        dx: Twips::from_pixels(0.0),
                        dy: Twips::from_pixels(image_height as f64),
                    },
                },
            ],
        })),
    ]);
    Ok(())
}
