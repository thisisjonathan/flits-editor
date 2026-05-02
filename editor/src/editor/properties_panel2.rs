use flits_core::{
    BitmapProperties, EditorColor, MovieProperties, PlaceSymbol, PreloaderType, TextAlign,
};

use crate::{
    editor::Context,
    edits::{MovieAction, MovieChange, PlacedSymbolChange},
    message::EditorMessage,
    undo::EditMessage,
};

// TODO: high cpu usage when cursor is in field

#[derive(Default)]
pub struct PropertiesPanel2 {}
impl PropertiesPanel2 {
    pub fn do_ui(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        match ctx.selection.placed_symbols.len() {
            0 => match ctx.selection.properties_symbol_index {
                None => self.show_panel(
                    ui,
                    ctx,
                    ctx.movie.properties.clone(),
                    |model| EditMessage::Change(MovieChange::MovieProperties(model)),
                    (),
                ),
                Some(properties_symbol_index) => {
                    match &ctx.movie.symbols[properties_symbol_index] {
                        flits_core::Symbol::Bitmap(bitmap) => self.show_panel(
                            ui,
                            ctx,
                            bitmap.properties.clone(),
                            |model| {
                                EditMessage::Change(MovieChange::BitmapProperties(
                                    properties_symbol_index,
                                    model,
                                ))
                            },
                            BitmapPropertiesAdditionalInfo {
                                error: match &bitmap.cache {
                                    flits_core::BitmapCacheStatus::Invalid(error) => {
                                        Some(error.clone())
                                    }
                                    _ => None,
                                },
                            },
                        ),
                        flits_core::Symbol::MovieClip(movie_clip) => {}
                        flits_core::Symbol::Font(flits_font) => {}
                    }
                }
            },
            1 => self.show_panel(
                ui,
                ctx,
                ctx.movie
                    .get_placed_symbols(ctx.selection.properties_symbol_index)
                    [ctx.selection.placed_symbols[0]]
                    .clone(),
                |model| {
                    EditMessage::Change(MovieChange::PlacedSymbols(vec![PlacedSymbolChange {
                        editing_symbol_index: ctx.selection.properties_symbol_index,
                        placed_symbol_index: ctx.selection.placed_symbols[0],
                        placed_symbol: model,
                    }]))
                },
                (),
            ),
            _ => {
                ui.label("Multiple items selected");
                return;
            }
        }
    }
    fn show_panel<T: PanelType<I>, I>(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &Context,
        model: T,
        edit_message: impl FnOnce(T) -> EditMessage<MovieChange, MovieAction>,
        additional_info: I,
    ) {
        ui.heading(&model.name());

        let blocks = model.property_blocks(additional_info);

        let mut model_clone = model;
        let mut commit_needed = false;
        let mut propery_changed = false;

        for (block_index, block) in blocks.iter().enumerate() {
            let (needs_change, needs_commit) = block.do_ui(ui, &mut model_clone, block_index);
            if needs_change {
                propery_changed = true;
            }
            if needs_commit {
                commit_needed = true;
            }
        }

        if propery_changed {
            // only send change message when the properties actually changed
            ctx.message_bus
                .publish(EditorMessage::NewEdit((edit_message)(model_clone)));
        }
        if commit_needed {
            ctx.message_bus
                .publish(EditorMessage::NewEdit(EditMessage::Commit));
        }
    }
}

impl EnumProperty for PreloaderType {
    fn enumerate() -> Vec<Self> {
        vec![
            PreloaderType::None,
            PreloaderType::StartAfterLoading,
            PreloaderType::WithPlayButton,
        ]
    }
}
impl EnumProperty for TextAlign {
    fn enumerate() -> Vec<Self> {
        vec![
            TextAlign::Left,
            TextAlign::Right,
            TextAlign::Center,
            TextAlign::Justify,
        ]
    }
}

macro_rules! property {
    ($name:literal, $model: ident, $type:expr ) => {
        Box::new(Property {
            name: $name.into(),
            get: |$model: &Self| $type.clone(),
            set: |$model: &mut Self, value| $type = value,
            settings: None,
            invalid: false,
        })
    };
}
/// A property that's inside an option. Only create the property when the option is Some.
macro_rules! property_option {
    ($name:literal, $model: ident, $option:expr, $inner_model: ident, $type:expr ) => {
        Box::new(Property {
            name: $name.into(),
            get: |$model: &Self| {
                let $inner_model = $option.as_ref().unwrap();
                $type.clone()
            },
            set: |$model: &mut Self, value| {
                let $inner_model = $option.as_mut().unwrap();
                $type = value;
            },
            settings: None,
            invalid: false,
        })
    };
}

type PropertyBox<T> = Box<dyn PropertyTrait<T>>;
struct Block<T: ?Sized> {
    properties: Vec<PropertyBox<T>>,
    // the condition needs to be evaluated when showing the ui because
    // it might change depending on an earlier property.
    condition: Option<fn(model: &T) -> bool>,
    error: Option<String>,
}
impl<T> Block<T> {
    fn new(properties: Vec<PropertyBox<T>>) -> Self {
        Self {
            properties,
            condition: None,
            error: None,
        }
    }
    fn do_ui(&self, ui: &mut egui::Ui, model: &mut T, index: usize) -> (bool, bool) {
        if let Some(condition) = self.condition {
            if !condition(model) {
                return (false, false);
            }
        }
        struct PropertyContext<T> {
            model_clone: T,
            commit_needed: bool,
            propery_changed: bool,
        }
        let mut context = PropertyContext {
            model_clone: model,
            commit_needed: false,
            propery_changed: false,
        };
        let iterator = self.properties.iter().map(|property| {
            |ui: &mut egui::Ui, context: &mut PropertyContext<&mut T>| {
                let (needs_change, needs_commit) = property.do_ui(ui, &mut context.model_clone);
                if needs_change {
                    context.propery_changed = true;
                }
                if needs_commit {
                    context.commit_needed = true;
                }
            }
        });
        // TODO: more accurate width estimate
        if ui.available_width() > (self.properties.len() * 140) as f32 {
            horizontal_layout(ui, &mut context, iterator);
        } else {
            // TODO: change amount of rows based on available width
            vertical_grid_layout(ui, &mut context, iterator, index);
        }

        if let Some(error) = &self.error {
            ui.colored_label(ui.style().visuals.error_fg_color, error);
        }

        (context.propery_changed, context.commit_needed)
    }
    fn with_condition(mut self, condition: fn(model: &T) -> bool) -> Self {
        self.condition = Some(condition);
        self
    }
    fn with_error(mut self, error: Option<String>) -> Self {
        self.error = error;
        self
    }
}

trait PanelType<I> {
    fn name(&self) -> String;
    fn property_blocks(&self, additional_info: I) -> Vec<Block<Self>>;
}

impl PanelType<()> for MovieProperties {
    fn name(&self) -> String {
        "Movie properties".into()
    }

    fn property_blocks(&self, _additional_info: ()) -> Vec<Block<Self>> {
        vec![Block::new(vec![
            property!("Width", model, model.width),
            property!("Height", model, model.height),
            property!("Framerate", model, model.frame_rate),
            // if i remember correctly, the spec specifies this as rgb. the alpha is ignored (TODO: check)
            property!("Background color", model, model.background_color),
            property!("Preloader", model, model.preloader),
        ])]
    }
}

struct BitmapPropertiesAdditionalInfo {
    error: Option<String>,
}
impl PanelType<BitmapPropertiesAdditionalInfo> for BitmapProperties {
    fn name(&self) -> String {
        "Bitmap properties".into()
    }

    fn property_blocks(&self, additional_info: BitmapPropertiesAdditionalInfo) -> Vec<Block<Self>> {
        vec![
            Block::new(vec![
                property!("Name", model, model.name),
                property!("Path", model, model.path).with_invalid(additional_info.error.is_some()),
            ])
            .with_error(additional_info.error),
            Block::new(vec![property!("Animated", model, model.animation)]),
            Block::new(vec![
                // TODO: if the value is too big the editor crashes because the frame is less than 1px
                // TODO: slower drag speed?
                // TODO: minimum?
                property_option!(
                    "Frames",
                    model,
                    model.animation,
                    inner_model,
                    inner_model.frame_count
                ),
                property_option!(
                    "Frames delay after each frame",
                    model,
                    model.animation,
                    inner_model,
                    inner_model.frame_delay
                ),
                property_option!(
                    "On last frame call (e.g. 'stop' or 'removeMovieClip')",
                    model,
                    model.animation,
                    inner_model,
                    inner_model.end_action
                ),
            ])
            .with_condition(|model| model.animation.is_some()),
        ]
    }
}

impl PanelType<()> for PlaceSymbol {
    fn name(&self) -> String {
        "Placed symbol properties".into()
    }

    fn property_blocks(&self, _additional_info: ()) -> Vec<Block<Self>> {
        let mut blocks: Vec<Block<Self>> = vec![Block::new(vec![
            property!("x", model, model.transform.x),
            property!("y", model, model.transform.y),
            property!("X scale", model, model.transform.x_scale),
            property!("Y scale", model, model.transform.y_scale),
            property!("Instance name", model, model.instance_name),
        ])];
        if self.text.is_some() {
            blocks.push(Block::new(vec![
                property_option!("Width", model, model.text, inner_model, inner_model.width),
                property_option!("Height", model, model.text, inner_model, inner_model.height),
                property_option!("Size", model, model.text, inner_model, inner_model.size),
                property_option!("Color", model, model.text, inner_model, inner_model.color),
                property_option!("Align", model, model.text, inner_model, inner_model.align),
            ]));
            blocks.push(Block::new(vec![
                // TODO: text field that are editable but not selectable are jank, maybe don't allow that combination?
                property_option!("Editable", model, model.text, im, im.editable),
                property_option!("Selectable", model, model.text, im, im.selectable),
                property_option!("Password", model, model.text, im, im.is_password),
                property_option!("HTML", model, model.text, im, im.is_html),
                property_option!("Multiline", model, model.text, im, im.is_multiline),
                property_option!("Word wrap", model, model.text, im, im.word_wrap),
            ]));
            blocks.push(Block::new(vec![property_option!(
                "Text",
                model,
                model.text,
                inner_model,
                inner_model.text
            )
            .with_settings(StringPropertySettings {
                multiline: self.text.as_ref().unwrap().is_multiline,
            })]));
        }

        blocks
    }
}

fn grid_layout<T>(
    ui: &mut egui::Ui,
    context: &mut T,
    iterator: impl ExactSizeIterator<Item = impl FnOnce(&mut egui::Ui, &mut T)>,
) {
    egui::Grid::new("movie_properties_grid").show(ui, |ui| {
        let length = iterator.len();
        for (index, callback) in iterator.enumerate() {
            (callback)(ui, context);
            if index == length / 2 {
                ui.end_row();
            }
        }
    });
}
/// A grid but vertical to let labels line up nicer
fn vertical_grid_layout<T>(
    ui: &mut egui::Ui,
    context: &mut T,
    iterator: impl ExactSizeIterator<Item = impl FnOnce(&mut egui::Ui, &mut T)>,
    index: usize,
) {
    let mut iterator = iterator.peekable();
    egui::Grid::new(format!("properties_vertical_grid_layout_{}", index)).show(ui, |ui| {
        let mut column_index = 0;
        loop {
            egui::Grid::new(format!(
                "properties_vertical_grid_layout_{}_inner_{}",
                index, column_index
            ))
            .show(ui, |ui| {
                for _ in 0..2 {
                    if let Some(callback) = iterator.next() {
                        (callback)(ui, context);
                        ui.end_row();
                    }
                }
            });
            column_index += 1;
            if iterator.peek().is_none() {
                break;
            }
        }
    });
}
fn horizontal_layout<T>(
    ui: &mut egui::Ui,
    context: &mut T,
    iterator: impl ExactSizeIterator<Item = impl FnOnce(&mut egui::Ui, &mut T)>,
) {
    ui.horizontal(|ui| {
        for callback in iterator {
            (callback)(ui, context);
            ui.add_space(5.0);
        }
    });
}

struct Property<Model, ValueType, Settings = ()> {
    name: String,
    get: fn(model: &Model) -> ValueType,
    set: fn(model: &mut Model, value: ValueType),
    settings: Option<Settings>,
    invalid: bool,
}
impl<Model, ValueType, Settings> Property<Model, ValueType, Settings> {
    fn with_settings(mut self, settings: Settings) -> Box<Self> {
        self.settings = Some(settings);
        Box::new(self)
    }
    fn with_invalid(mut self, invalid: bool) -> Box<Self> {
        self.invalid = invalid;
        Box::new(self)
    }
}

trait PropertyTrait<Model> {
    fn do_ui(&self, ui: &mut egui::Ui, model: &mut Model) -> (bool, bool);
}
// we need to implement these all seperately instead of using a trait to make prove
// the implementation doesn't overlap with EnumProperty
macro_rules! impl_numeric_properties {
    (for $($t:ty),+) => {
        $(
            impl<Model> PropertyTrait<Model> for Property<Model, $t> {
                fn do_ui(&self, ui: &mut egui::Ui, model: &mut Model) -> (bool, bool) {
                    ui.label(format!("{}:", self.name));
                    let mut value = (self.get)(&model);
                    let response = ui.add(egui::DragValue::new(&mut value));
                    if response.changed() {
                        (self.set)(model, value);
                    }
                    (
                        response.changed(),
                        response.lost_focus() || response.drag_stopped(),
                    )
                }
            }
        )*
    }
}
impl_numeric_properties!(for f32, f64, u32);

struct StringPropertySettings {
    multiline: bool,
}
impl<Model> PropertyTrait<Model> for Property<Model, String, StringPropertySettings> {
    fn do_ui(&self, ui: &mut egui::Ui, model: &mut Model) -> (bool, bool) {
        ui.label(format!("{}:", self.name));
        let mut value = (self.get)(&model);
        let mut text_edit = if self
            .settings
            .as_ref()
            .is_some_and(|settings| settings.multiline)
        {
            egui::TextEdit::multiline(&mut value).min_size(egui::Vec2::new(200.0, 0.0))
        } else {
            egui::TextEdit::singleline(&mut value).min_size(egui::Vec2::new(200.0, 0.0))
        };
        if self.invalid {
            text_edit = text_edit.text_color(ui.style().visuals.error_fg_color);
        }
        let response = ui.add(text_edit);
        if response.changed() {
            (self.set)(model, value);
        }
        (response.changed(), response.lost_focus())
    }
}

impl<Model> PropertyTrait<Model> for Property<Model, bool> {
    fn do_ui(&self, ui: &mut egui::Ui, model: &mut Model) -> (bool, bool) {
        let mut value = (self.get)(&model);
        let response = ui.checkbox(&mut value, &self.name);
        if response.changed() {
            (self.set)(model, value);
        }
        (response.changed(), response.changed())
    }
}

impl<Model, T: Default> PropertyTrait<Model> for Property<Model, Option<T>> {
    fn do_ui(&self, ui: &mut egui::Ui, model: &mut Model) -> (bool, bool) {
        let mut value = (self.get)(&model).is_some();
        let response = ui.checkbox(&mut value, &self.name);
        if response.changed() {
            if value {
                (self.set)(model, Some(T::default()));
            } else {
                (self.set)(model, None);
            }
        }
        (response.changed(), response.changed())
    }
}

trait EnumProperty: ToString + PartialEq + Clone
where
    Self: Sized,
{
    fn enumerate() -> Vec<Self>;
}
impl<Model, ValueType> PropertyTrait<Model> for Property<Model, ValueType>
where
    ValueType: EnumProperty,
{
    fn do_ui(&self, ui: &mut egui::Ui, model: &mut Model) -> (bool, bool) {
        ui.label(format!("{}:", self.name));
        let value_before = (self.get)(&model);
        let mut value = value_before.clone();
        egui::ComboBox::from_id_salt(&self.name)
            .selected_text(format!("{:}", value.to_string()))
            .show_ui(ui, |ui| {
                for variant in ValueType::enumerate() {
                    ui.selectable_value(&mut value, variant.clone(), variant.to_string());
                }
            });
        let changed = value != value_before;
        if changed {
            (self.set)(model, value);
        }
        (changed, changed)
    }
}

impl<Model> PropertyTrait<Model> for Property<Model, EditorColor> {
    fn do_ui(&self, ui: &mut egui::Ui, model: &mut Model) -> (bool, bool) {
        ui.label(format!("{}:", self.name));

        let original_value = (self.get)(&model);
        let mut value = original_value.clone();

        let mut color = egui::Color32::from_rgba_unmultiplied(value.r, value.g, value.b, value.a);
        let response = egui::color_picker::color_edit_button_srgba(
            ui,
            &mut color,
            // the alpha doesn't do anything for all places we currently use the color picker
            egui::color_picker::Alpha::Opaque,
        );
        let color_data = color.to_srgba_unmultiplied();
        value.r = color_data[0];
        value.g = color_data[1];
        value.b = color_data[2];
        value.a = color_data[3];

        let changed = value != original_value;
        if changed {
            (self.set)(model, value);
        }

        // response.clicked_elsewhere() is true even when you don't have the color picker selected
        // and you click anywhere in the program
        // TODO: this causes unnecessary commits, but a commit without changes is a noop so it might not be a problem?
        // response.clicked_elsewhere() is false when you press escape, we need to handle that seperately
        // might be fixed after the popup refactor in egui: https://github.com/emilk/egui/issues/5189
        (
            changed,
            ui.input(|i| i.key_pressed(egui::Key::Escape)) || response.clicked_elsewhere(),
        )
    }
}
