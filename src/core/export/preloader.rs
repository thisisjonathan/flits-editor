use swf::{
    avm1::types::{Action, ConstantPool, DefineFunction2, FunctionFlags, If, Push, StoreRegister},
    BlendMode, ButtonActionCondition, ButtonRecord, ButtonState, Color, ColorTransform, FillStyle,
    Matrix, PlaceObject, PlaceObjectAction, Point, PointDelta, Rectangle, RemoveObject, Shape,
    ShapeFlag, ShapeRecord, ShapeStyles, Sprite, StyleChangeData, SwfStr, Tag, Twips,
};

use crate::core::{PreloaderType, SWF_VERSION};

use super::{SwfBuilder, SwfBuilderButton, SwfBuilderButtonAction, SwfBuilderTag};

pub(super) fn build_preloader(
    preloader_type: PreloaderType,
    swf_builder: &mut SwfBuilder,
    stage_width: f64,
    stage_height: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    let background_id = swf_builder.next_character_id();
    let loading_bar_background_id = swf_builder.next_character_id();
    let loading_bar_foreground_id = swf_builder.next_character_id();
    let loading_bar_clip_id = swf_builder.next_character_id();
    let center_matrix = Matrix::translate(
        Twips::from_pixels(stage_width as f64 / 2.0),
        Twips::from_pixels(stage_height as f64 / 2.0),
    );
    let loading_bar_width = stage_width * 0.85;
    let loading_bar_height = 2.0;
    let loading_bar_matrix = center_matrix
        * Matrix::translate(
            Twips::from_pixels(loading_bar_width / -2.0),
            Twips::from_pixels(loading_bar_height / -2.0),
        );
    swf_builder.tags.extend(create_rectangle(
        background_id,
        stage_width,
        stage_height,
        Color::BLACK,
        Matrix::IDENTITY,
    ));
    swf_builder.tags.extend(create_rectangle(
        loading_bar_background_id,
        loading_bar_width,
        loading_bar_height,
        Color::GRAY,
        loading_bar_matrix,
    ));

    let mut action_data: Vec<u8> = vec![];
    let mut action_writer = swf::avm1::write::Writer::new(&mut action_data, SWF_VERSION);
    let action = Action::ConstantPool(ConstantPool {
        strings: vec![
            SwfStr::from_utf8_str("loading_bar"),
            SwfStr::from_utf8_str("onEnterFrame"),
            SwfStr::from_utf8_str("_xscale"),
            SwfStr::from_utf8_str("getBytesLoaded"),
            SwfStr::from_utf8_str("getBytesTotal"),
            SwfStr::from_utf8_str("gotoAndStop"),
        ],
    });
    action_writer.write_action(&action)?;
    let action = Action::Push(Push {
        // loading_bar
        values: vec![swf::avm1::types::Value::ConstantPool(0)],
    });
    action_writer.write_action(&action)?;
    let action = Action::GetVariable;
    action_writer.write_action(&action)?;
    let action = Action::Push(Push {
        // onEnterFrame
        values: vec![swf::avm1::types::Value::ConstantPool(1)],
    });
    action_writer.write_action(&action)?;

    let mut on_enter_frame_action_data = vec![];
    let mut on_enter_frame_action_writer =
        swf::avm1::write::Writer::new(&mut on_enter_frame_action_data, SWF_VERSION);
    let action = Action::Push(Push {
        values: vec![
            swf::avm1::types::Value::Double(0.0),
            // _root
            swf::avm1::types::Value::Register(1),
            // getBytesLoaded
            swf::avm1::types::Value::ConstantPool(3),
        ],
    });
    on_enter_frame_action_writer.write_action(&action)?;
    let action = Action::CallMethod;
    on_enter_frame_action_writer.write_action(&action)?;
    let action = Action::Push(Push {
        values: vec![
            swf::avm1::types::Value::Double(0.0),
            // _root
            swf::avm1::types::Value::Register(1),
            // getBytesTotal
            swf::avm1::types::Value::ConstantPool(4),
        ],
    });
    on_enter_frame_action_writer.write_action(&action)?;
    let action = Action::CallMethod;
    on_enter_frame_action_writer.write_action(&action)?;
    let action = Action::Divide;
    on_enter_frame_action_writer.write_action(&action)?;
    let action = Action::Push(Push {
        values: vec![swf::avm1::types::Value::Int(100)],
    });
    on_enter_frame_action_writer.write_action(&action)?;

    let action = Action::Multiply;
    on_enter_frame_action_writer.write_action(&action)?;
    let action = Action::StoreRegister(StoreRegister { register: 2 });
    on_enter_frame_action_writer.write_action(&action)?;
    let action = Action::Pop;
    on_enter_frame_action_writer.write_action(&action)?;
    let action = Action::Push(Push {
        // loading_bar
        values: vec![swf::avm1::types::Value::ConstantPool(0)],
    });
    on_enter_frame_action_writer.write_action(&action)?;
    let action = Action::GetVariable;
    on_enter_frame_action_writer.write_action(&action)?;
    let action = Action::Push(Push {
        values: vec![
            // _xscale
            swf::avm1::types::Value::ConstantPool(2),
            swf::avm1::types::Value::Register(2),
        ],
    });
    on_enter_frame_action_writer.write_action(&action)?;
    let action = Action::SetMember;
    on_enter_frame_action_writer.write_action(&action)?;
    let action = Action::Push(Push {
        values: vec![
            swf::avm1::types::Value::Register(2),
            swf::avm1::types::Value::Int(100),
        ],
    });
    on_enter_frame_action_writer.write_action(&action)?;
    let action = Action::Less2;
    on_enter_frame_action_writer.write_action(&action)?;
    let action = Action::If(If { offset: 23 });
    on_enter_frame_action_writer.write_action(&action)?;
    let action = Action::Push(Push {
        values: vec![
            swf::avm1::types::Value::Int(2),
            swf::avm1::types::Value::Double(1.0),
            // _root
            swf::avm1::types::Value::Register(1),
            // gotoAndStop
            swf::avm1::types::Value::ConstantPool(5),
        ],
    });
    on_enter_frame_action_writer.write_action(&action)?;
    let action = Action::CallMethod;
    on_enter_frame_action_writer.write_action(&action)?;
    let action = Action::Pop;
    on_enter_frame_action_writer.write_action(&action)?;

    let action = Action::DefineFunction2(DefineFunction2 {
        name: "".into(),
        register_count: 3,
        params: vec![],
        flags: FunctionFlags::PRELOAD_ROOT
            | FunctionFlags::SUPPRESS_THIS
            | FunctionFlags::SUPPRESS_SUPER
            | FunctionFlags::SUPPRESS_ARGUMENTS,
        actions: &on_enter_frame_action_data,
    });
    action_writer.write_action(&action)?;
    let action = Action::SetMember;
    action_writer.write_action(&action)?;
    let action = Action::Stop;
    action_writer.write_action(&action)?;
    let action = Action::End;
    action_writer.write_action(&action)?;

    swf_builder.tags.extend(vec![
        define_rectangle(
            loading_bar_foreground_id,
            loading_bar_width,
            loading_bar_height,
            Color::WHITE,
        ),
        SwfBuilderTag::Tag(Tag::DefineSprite(Sprite {
            id: loading_bar_clip_id,
            num_frames: 1,
            tags: vec![Tag::PlaceObject(Box::new(PlaceObject {
                version: 2,
                action: PlaceObjectAction::Place(loading_bar_foreground_id),
                depth: 1,
                matrix: Some(Matrix::IDENTITY),
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
            }))],
        })),
        SwfBuilderTag::Tag(Tag::PlaceObject(Box::new(PlaceObject {
            version: 2,
            action: PlaceObjectAction::Place(loading_bar_clip_id),
            depth: loading_bar_clip_id,
            matrix: Some(loading_bar_matrix),
            color_transform: None,
            ratio: None,
            name: Some("loading_bar".into()),
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
        }))),
        SwfBuilderTag::DoAction(action_data),
        SwfBuilderTag::Tag(Tag::ShowFrame),
    ]);
    let mut play_button_action_data = vec![];
    let mut play_button_action_writer =
        swf::avm1::write::Writer::new(&mut play_button_action_data, SWF_VERSION);
    if preloader_type == PreloaderType::WithPlayButton {
        let action = Action::Push(Push {
            values: vec![
                swf::avm1::types::Value::Double(0.0),
                swf::avm1::types::Value::Str("_root".into()),
            ],
        });
        play_button_action_writer.write_action(&action)?;
        let action = Action::GetVariable;
        play_button_action_writer.write_action(&action)?;
        let action = Action::Push(Push {
            values: vec![swf::avm1::types::Value::Str("nextFrame".into())],
        });
        play_button_action_writer.write_action(&action)?;
        let action = Action::CallMethod;
        play_button_action_writer.write_action(&action)?;
        let action = Action::Pop;
        play_button_action_writer.write_action(&action)?;
        let action = Action::End;
        play_button_action_writer.write_action(&action)?;

        let play_button_shape_id = swf_builder.next_character_id();
        let play_button_shape_over_id = swf_builder.next_character_id();
        let play_button_id = swf_builder.next_character_id();
        swf_builder.tags.extend(vec![
            define_play_button_shape(play_button_shape_id, 32.0, 32.0, Color::WHITE),
            define_play_button_shape(play_button_shape_over_id, 32.0, 32.0, Color::GRAY),
            SwfBuilderTag::DefineButton2(Box::new(SwfBuilderButton {
                id: play_button_id,
                is_track_as_menu: false,
                records: vec![
                    ButtonRecord {
                        states: ButtonState::UP,
                        id: play_button_shape_id,
                        depth: 1,
                        matrix: Matrix::IDENTITY,
                        color_transform: ColorTransform::default(),
                        filters: vec![],
                        blend_mode: BlendMode::Normal,
                    },
                    ButtonRecord {
                        states: ButtonState::OVER | ButtonState::DOWN | ButtonState::HIT_TEST,
                        id: play_button_shape_over_id,
                        depth: 1,
                        matrix: Matrix::IDENTITY,
                        color_transform: ColorTransform::default(),
                        filters: vec![],
                        blend_mode: BlendMode::Normal,
                    },
                ],
                actions: vec![SwfBuilderButtonAction {
                    conditions: ButtonActionCondition::OVER_DOWN_TO_OVER_UP,
                    action_data: play_button_action_data,
                }],
            })),
            SwfBuilderTag::Tag(Tag::PlaceObject(Box::new(PlaceObject {
                version: 2,
                action: PlaceObjectAction::Place(play_button_id),
                depth: 3,
                matrix: Some(
                    center_matrix
                        * Matrix::translate(Twips::from_pixels(-16.0), Twips::from_pixels(32.0)),
                ),
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
            }))),
            SwfBuilderTag::Tag(Tag::ShowFrame),
        ]);
    }
    swf_builder.tags.extend(vec![
        SwfBuilderTag::Tag(Tag::RemoveObject(RemoveObject {
            depth: background_id,
            character_id: Some(background_id),
        })),
        SwfBuilderTag::Tag(Tag::RemoveObject(RemoveObject {
            depth: loading_bar_background_id,
            character_id: Some(loading_bar_background_id),
        })),
        SwfBuilderTag::Tag(Tag::RemoveObject(RemoveObject {
            depth: loading_bar_clip_id,
            character_id: Some(loading_bar_clip_id),
        })),
    ]);
    if let PreloaderType::WithPlayButton = preloader_type {
        // remove play button
        swf_builder
            .tags
            .push(SwfBuilderTag::Tag(Tag::RemoveObject(RemoveObject {
                depth: 3,
                character_id: Some(3),
            })));
    }
    Ok(())
}

fn create_rectangle<'a>(
    shape_id: u16,
    width: f64,
    height: f64,
    color: Color,
    matrix: Matrix,
) -> Vec<SwfBuilderTag<'a>> {
    vec![
        define_rectangle(shape_id, width, height, color),
        SwfBuilderTag::Tag(Tag::PlaceObject(Box::new(PlaceObject {
            version: 2,
            action: PlaceObjectAction::Place(shape_id),
            depth: shape_id,
            matrix: Some(matrix),
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
        }))),
    ]
}
fn define_rectangle<'a>(shape_id: u16, width: f64, height: f64, color: Color) -> SwfBuilderTag<'a> {
    SwfBuilderTag::Tag(Tag::DefineShape(Shape {
        version: 1,
        id: shape_id,
        shape_bounds: Rectangle {
            x_min: Twips::from_pixels(0.0),
            y_min: Twips::from_pixels(0.0),
            x_max: Twips::from_pixels(width as f64),
            y_max: Twips::from_pixels(height as f64),
        },
        edge_bounds: Rectangle {
            x_min: Twips::from_pixels(0.0),
            y_min: Twips::from_pixels(0.0),
            x_max: Twips::from_pixels(width as f64),
            y_max: Twips::from_pixels(height as f64),
        },
        flags: ShapeFlag::empty(),
        styles: ShapeStyles {
            fill_styles: vec![FillStyle::Color(color)],
            line_styles: vec![],
        },
        shape: vec![
            ShapeRecord::StyleChange(Box::new(StyleChangeData {
                move_to: Some(Point::new(
                    Twips::from_pixels(width as f64),
                    Twips::from_pixels(height as f64),
                )),
                fill_style_0: None,
                fill_style_1: Some(1),
                line_style: None,
                new_styles: None,
            })),
            ShapeRecord::StraightEdge {
                delta: PointDelta {
                    dx: Twips::from_pixels(-(width as f64)),
                    dy: Twips::from_pixels(0.0),
                },
            },
            ShapeRecord::StraightEdge {
                delta: PointDelta {
                    dx: Twips::from_pixels(0.0),
                    dy: Twips::from_pixels(-(height as f64)),
                },
            },
            ShapeRecord::StraightEdge {
                delta: PointDelta {
                    dx: Twips::from_pixels(width as f64),
                    dy: Twips::from_pixels(0.0),
                },
            },
            ShapeRecord::StraightEdge {
                delta: PointDelta {
                    dx: Twips::from_pixels(0.0),
                    dy: Twips::from_pixels(height as f64),
                },
            },
        ],
    }))
}
fn define_play_button_shape<'a>(
    shape_id: u16,
    width: f64,
    height: f64,
    color: Color,
) -> SwfBuilderTag<'a> {
    SwfBuilderTag::Tag(Tag::DefineShape(Shape {
        version: 1,
        id: shape_id,
        shape_bounds: Rectangle {
            x_min: Twips::from_pixels(0.0),
            y_min: Twips::from_pixels(0.0),
            x_max: Twips::from_pixels(width as f64),
            y_max: Twips::from_pixels(height as f64),
        },
        edge_bounds: Rectangle {
            x_min: Twips::from_pixels(0.0),
            y_min: Twips::from_pixels(0.0),
            x_max: Twips::from_pixels(width as f64),
            y_max: Twips::from_pixels(height as f64),
        },
        flags: ShapeFlag::empty(),
        styles: ShapeStyles {
            fill_styles: vec![FillStyle::Color(color)],
            line_styles: vec![],
        },
        shape: vec![
            ShapeRecord::StyleChange(Box::new(StyleChangeData {
                move_to: Some(Point::new(
                    Twips::from_pixels(width as f64),
                    Twips::from_pixels((height as f64) / 2.0),
                )),
                fill_style_0: None,
                fill_style_1: Some(1),
                line_style: None,
                new_styles: None,
            })),
            ShapeRecord::StraightEdge {
                delta: PointDelta {
                    dx: Twips::from_pixels(-(width as f64)),
                    dy: Twips::from_pixels((height as f64) / 2.0),
                },
            },
            ShapeRecord::StraightEdge {
                delta: PointDelta {
                    dx: Twips::from_pixels(0.0),
                    dy: Twips::from_pixels(-(height as f64)),
                },
            },
            ShapeRecord::StraightEdge {
                delta: PointDelta {
                    dx: Twips::from_pixels(width as f64),
                    dy: Twips::from_pixels((height as f64) / 2.0),
                },
            },
        ],
    }))
}
