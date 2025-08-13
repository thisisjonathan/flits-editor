use std::path::PathBuf;

use swf::{
    CharacterId, ExportedAsset, Font, Point, PointDelta, Rectangle, ShapeRecord, SwfStr, Tag, Twips,
};

pub trait FontSwfBuilder<'a> {
    fn add_tag(&mut self, tag: swf::Tag<'a>);
}
pub trait FontAllocator {
    fn alloc_swf_string(&self, string: String) -> &SwfStr;
}

// adapted from: https://github.com/djcsdy/swfmill/blob/53d769029adc9d817972e1ccd648b7b335bf78b7/src/swft/swft_import_ttf.cpp#L289
pub fn font_to_swf<'a>(
    name: String,
    path: PathBuf,
    characters: String,
    character_id: CharacterId,
    swf_builder: &mut impl FontSwfBuilder<'a>,
    allocator: &'a impl FontAllocator,
) -> Result<(), Box<dyn std::error::Error>> {
    let scaling_factor = 1024;

    let font_data = std::fs::read(path)?;
    let mut face = rustybuzz::Face::from_slice(&font_data, 0).ok_or("Font doesn't have a face")?;
    // swfmill calls this, not sure what it does
    face.set_pixels_per_em(Some((scaling_factor as u16, scaling_factor as u16)));
    // formula found by manually finding scaling factors for fonts with different units_per_em
    // by lining things up visually and then creating a formula that worked for both values i tested
    // (font with units_per_em 1000 and 2048, scaling factors 3.2 and 1.6)
    let shape_scaling_factor: f64 = -0.001527 * face.units_per_em() as f64 + 4.727;
    // dividing by 64 is what swfmill does
    let shape_scaling_factor_x = 1.0 / 64.0 * shape_scaling_factor;
    let shape_scaling_factor_y = -1.0 / 64.0 * shape_scaling_factor;
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    // TODO: shape each character seperately to avoid kerning and litugates
    buffer.push_str(&characters);
    let features = vec![];
    let glyph_buffer = rustybuzz::shape(&face, &features, buffer);

    let characters_as_utf16: Vec<u16> = characters.encode_utf16().collect();

    let mut glyphs = Vec::with_capacity(glyph_buffer.len());
    for (index, (glyph_info, glyph_pos)) in glyph_buffer
        .glyph_infos()
        .iter()
        .zip(glyph_buffer.glyph_positions())
        .enumerate()
    {
        // we need to cast to u16 for some reason, it's what rustybuzz does: https://github.com/harfbuzz/rustybuzz/blob/51d99b83ae78e4ad8993f393f0e5ce05701ebb7e/src/hb/buffer.rs#L247
        let glyph_id = rustybuzz::ttf_parser::GlyphId(glyph_info.glyph_id as u16);
        let bounding_box = face
            .glyph_bounding_box(glyph_id)
            .ok_or("Font doesn't have a bounding box")?;
        let mut builder = ShapeRecordBuilder::new(shape_scaling_factor_x, shape_scaling_factor_y);
        face.outline_glyph(glyph_id, &mut builder);
        glyphs.push(swf::Glyph {
            shape_records: builder.shape_records,
            code: characters_as_utf16[index],
            // swfmill does this, but it produces way too small results:
            // advance: 1 + (glyph_pos.x_advance >> 6) as i16,
            // this is much closer for fonts with units_per_em of 1000:
            // advance: glyph_pos.x_advance as i16,
            // but not for fonts with units_per_em of 2048, hence this code:
            advance: (glyph_pos.x_advance as f64 * (1030.0 / face.units_per_em() as f64)) as i16,
            bounds: Some(Rectangle {
                x_min: Twips::from_pixels(bounding_box.x_min as f64 * shape_scaling_factor_x),
                x_max: Twips::from_pixels(bounding_box.x_max as f64 * shape_scaling_factor_x),
                // min and max are reversed because we are multiplying with a negative number
                y_min: Twips::from_pixels(bounding_box.y_max as f64 * shape_scaling_factor_y),
                y_max: Twips::from_pixels(bounding_box.y_min as f64 * shape_scaling_factor_y),
            }),
        });
    }
    dbg!(face.units_per_em());

    swf_builder.add_tag(swf::Tag::DefineFont2(Box::new(Font {
        version: 2, // TODO: Why doesn't this work if it's 3?
        id: character_id,
        name: allocator.alloc_swf_string(name.clone()),
        language: swf::Language::Unknown, // swfmill doesn't seem to set this
        layout: Some(swf::FontLayout {
            ascent: (face.ascender() as i32 * scaling_factor / face.units_per_em()) as u16,
            descent: (-face.descender() as i32 * scaling_factor / face.units_per_em()) as u16,
            leading: (face.line_gap() as i32 * scaling_factor / face.units_per_em()) as i16,
            kerning: vec![], // TODO: swfmill has a TODO for kerning
        }),
        glyphs,
        flags: swf::FontFlag::HAS_LAYOUT | swf::FontFlag::IS_ANSI, // TODO: find out correct flags, plus we should be able to handle non-ascii characters
    })));
    swf_builder.add_tag(Tag::ExportAssets(vec![ExportedAsset {
        id: character_id,
        name: allocator.alloc_swf_string(name),
    }]));

    // TODO: there are weird lines when viewing in Flash player

    Ok(())
}

struct ShapeRecordBuilder {
    shape_scaling_factor_x: f64,
    shape_scaling_factor_y: f64,
    last_x: f32,
    last_y: f32,
    shape_records: Vec<ShapeRecord>,
}
impl ShapeRecordBuilder {
    fn new(shape_scaling_factor_x: f64, shape_scaling_factor_y: f64) -> Self {
        ShapeRecordBuilder {
            shape_scaling_factor_x,
            shape_scaling_factor_y,
            last_x: 0.0,
            last_y: 0.0,
            shape_records: vec![],
        }
    }
}
impl rustybuzz::ttf_parser::OutlineBuilder for ShapeRecordBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.last_x = x;
        self.last_y = y;
        self.shape_records
            .push(ShapeRecord::StyleChange(Box::new(swf::StyleChangeData {
                move_to: Some(Point::new(
                    Twips::from_pixels(x as f64 * self.shape_scaling_factor_x),
                    Twips::from_pixels(y as f64 * self.shape_scaling_factor_y),
                )),
                fill_style_0: Some(1),
                fill_style_1: None,
                line_style: None,
                new_styles: None,
            })));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let dx = x - self.last_x;
        let dy = y - self.last_y;
        self.last_x = x;
        self.last_y = y;
        self.shape_records.push(ShapeRecord::StraightEdge {
            delta: PointDelta::new(
                Twips::from_pixels(dx as f64 * self.shape_scaling_factor_x),
                Twips::from_pixels(dy as f64 * self.shape_scaling_factor_y),
            ),
        })
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let ctx = x1 - self.last_x;
        let cty = y1 - self.last_y;
        let dx = (x - self.last_x) - ctx;
        let dy = (y - self.last_y) - cty;
        self.last_x = x;
        self.last_y = y;
        self.shape_records.push(ShapeRecord::CurvedEdge {
            anchor_delta: PointDelta::from_pixels(
                dx as f64 * self.shape_scaling_factor_x,
                dy as f64 * self.shape_scaling_factor_y,
            ),
            control_delta: PointDelta::from_pixels(
                ctx as f64 * self.shape_scaling_factor_x,
                cty as f64 * self.shape_scaling_factor_y,
            ),
        })
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        // TODO
        println!("curve to");
    }

    fn close(&mut self) {
        // TODO
    }
}
