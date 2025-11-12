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
    if path.extension().is_none_or(|extension| extension != "ttf") {
        return Err(format!(
            "Only ttf files are supported, got {}",
            path.to_str().unwrap_or("Unable to convert path to string")
        )
        .into());
    }

    let scaling_factor = 1024;

    let font_data = std::fs::read(path)?;
    let face = ttf_parser::Face::parse(&font_data, 0)?;
    // formula found by manually finding scaling factors for fonts with different units_per_em
    // by lining things up visually and then creating a formula that worked for both values i tested
    // (font with units_per_em 1000 and 2048, scaling factors 3.2 and 1.6)
    let shape_scaling_factor: f64 = -0.001527 * face.units_per_em() as f64 + 4.727;
    // dividing by 64 is what swfmill does
    let shape_scaling_factor_x = 1.0 / 64.0 * shape_scaling_factor;
    let shape_scaling_factor_y = -1.0 / 64.0 * shape_scaling_factor;

    let mut glyphs = Vec::with_capacity(characters.len());
    let mut characters_vec: Vec<char> = characters.chars().collect();
    characters_vec.sort_by(|a, b| a.cmp(b));
    for character in characters_vec {
        let glyph_id = face
            .glyph_index(character)
            .ok_or_else(|| format!("Font '{}' doesn't have a glyph for '{}'", name, character))?;
        let bounding_box = face.glyph_bounding_box(glyph_id);
        let mut builder = ShapeRecordBuilder::new(shape_scaling_factor_x, shape_scaling_factor_y);
        face.outline_glyph(glyph_id, &mut builder);
        glyphs.push(swf::Glyph {
            shape_records: builder.shape_records,
            code: character as u16,
            // swfmill does this, but it produces way too small results:
            // advance: 1 + (face.glyph_hor_advance(glyph_id) >> 6) as i16,
            // this is much closer for fonts with units_per_em of 1000:
            // advance: face.glyph_hor_advance(glyph_id) as i16,
            // but not for fonts with units_per_em of 2048, hence this code:
            advance: (face.glyph_hor_advance(glyph_id).unwrap_or(0) as f64
                * (1030.0 / face.units_per_em() as f64)) as i16,
            bounds: match bounding_box {
                Some(bounding_box) => Some(Rectangle {
                    x_min: Twips::from_pixels(bounding_box.x_min as f64 * shape_scaling_factor_x),
                    x_max: Twips::from_pixels(bounding_box.x_max as f64 * shape_scaling_factor_x),
                    // min and max are reversed because we are multiplying with a negative number
                    y_min: Twips::from_pixels(bounding_box.y_max as f64 * shape_scaling_factor_y),
                    y_max: Twips::from_pixels(bounding_box.y_min as f64 * shape_scaling_factor_y),
                }),
                // space doesn't have a bounding box
                // TODO: is this correct? when using None the swf crate refuses to write
                None => Some(Rectangle::ZERO),
            },
        });
    }

    let mut font_family = face
        .names()
        .get(ttf_parser::name_id::FAMILY)
        .ok_or("Unable to get font family name")?;
    // for some reason .get() doesn't always return the right result, even though it exists?!?
    // it just says "unsuppored encoding" but the name id is also different
    // get the correct one manually if that's the case
    if font_family.name_id != ttf_parser::name_id::FAMILY {
        for name in face.names() {
            if name.name_id == ttf_parser::name_id::FAMILY {
                font_family = name;
                break;
            }
        }
    }
    if font_family.name_id != ttf_parser::name_id::FAMILY {
        // if we still haven' found the right thing, give up
        return Err("Unable to get font family name (even with workaround)".into());
    }

    // TODO: find out correct flags, plus we should be able to handle non-ascii characters
    let mut flags = swf::FontFlag::HAS_LAYOUT | swf::FontFlag::IS_ANSI;
    if face.is_bold() {
        flags |= swf::FontFlag::IS_BOLD;
    }
    if face.is_italic() {
        flags |= swf::FontFlag::IS_ITALIC;
    }

    swf_builder.add_tag(swf::Tag::DefineFont2(Box::new(Font {
        version: 2, // TODO: Why doesn't this work if it's 3?
        id: character_id,
        name: allocator.alloc_swf_string(
            font_family
                .to_string()
                .ok_or("Unable to convert font name to unicode")?,
        ),
        language: swf::Language::Unknown, // swfmill doesn't seem to set this
        layout: Some(swf::FontLayout {
            ascent: (face.ascender() as i32 * scaling_factor / face.units_per_em() as i32) as u16,
            descent: (-face.descender() as i32 * scaling_factor / face.units_per_em() as i32)
                as u16,
            leading: (face.line_gap() as i32 * scaling_factor / face.units_per_em() as i32) as i16,
            kerning: vec![], // TODO: swfmill has a TODO for kerning
        }),
        glyphs,
        flags,
    })));
    swf_builder.add_tag(Tag::ExportAssets(vec![ExportedAsset {
        id: character_id,
        // TODO: should this be the file name or the family name?
        // when you create a text field in actionscript, you need to use the family name,
        // not the exported name
        name: allocator.alloc_swf_string(name),
    }]));

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
impl ttf_parser::OutlineBuilder for ShapeRecordBuilder {
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

    fn curve_to(&mut self, _x1: f32, _y1: f32, _x2: f32, _y2: f32, _x: f32, _y: f32) {
        // ttf doesn't support cubic splines according to this:
        // https://github.com/godotengine/godot/issues/97420
        // https://typedrawers.com/discussion/4167/why-does-truetype-use-quadratic-splines
        // so we don't need to deal with this
        panic!("According to the internet, ttf fonts don't support cubic splines. If you see this it turns out that was wrong.");
    }

    fn close(&mut self) {
        // Flash player expects the last point to match up with the first point, otherwise it shows weird lines.
        // we don't match up exactly due to floating point math
        // compensate by finding how much the last point is off and then move it to match up
        // this is hacky but it works
        let mut dx = 0;
        let mut dy = 0;
        for shape_record in &self.shape_records {
            match shape_record {
                ShapeRecord::StyleChange(_) => {}
                ShapeRecord::StraightEdge { delta } => {
                    dx += delta.dx.get();
                    dy += delta.dy.get();
                }
                ShapeRecord::CurvedEdge {
                    control_delta,
                    anchor_delta,
                } => {
                    dx += control_delta.dx.get() + anchor_delta.dx.get();
                    dy += control_delta.dy.get() + anchor_delta.dy.get();
                }
            }
        }
        match self.shape_records.last_mut().unwrap() {
            ShapeRecord::StyleChange(_style_change_data) => {
                panic!("Last shape record of font is a move")
            }
            ShapeRecord::StraightEdge { delta } => {
                delta.dx -= Twips::new(dx);
                delta.dy -= Twips::new(dy);
            }
            ShapeRecord::CurvedEdge {
                control_delta: _,
                anchor_delta,
            } => {
                // TODO: also modify the control?
                anchor_delta.dx -= Twips::new(dx);
                anchor_delta.dy -= Twips::new(dy);
            }
        }
    }
}
