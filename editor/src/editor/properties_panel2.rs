use flits_core::{EditorColor, MovieProperties, PreloaderType};

use crate::{editor::Context, edits::MovieChange, message::EditorMessage, undo::EditMessage};

impl EnumProperty for PreloaderType {
    fn enumerate() -> Vec<Self> {
        vec![
            PreloaderType::None,
            PreloaderType::StartAfterLoading,
            PreloaderType::WithPlayButton,
        ]
    }
}

macro_rules! property {
    ($name:literal, $model: ident, $type:expr ) => {
        Box::new(Property {
            name: $name.into(),
            get: |$model: &MovieProperties| $type.clone(),
            set: |$model: &mut MovieProperties, value| $type = value,
        })
    };
}

type PropertyBox<T> = Box<dyn PropertyTrait<T>>;

#[derive(Default)]
pub struct PropertiesPanel2 {}
impl PropertiesPanel2 {
    pub fn do_ui(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        let properties: [PropertyBox<MovieProperties>; _] = [
            property!("Width", model, model.width),
            property!("Height", model, model.height),
            property!("Framerate", model, model.frame_rate),
            // if i remember correctly, the spec specifies this as rgb. the alpha is ignored (TODO: check)
            property!("Background color", model, model.background_color),
            property!("Preloader", model, model.preloader),
        ];

        ui.heading("Movie properties");
        struct PropertyContext {
            model_clone: MovieProperties,
            commit_needed: bool,
            propery_changed: bool,
        }
        let mut context = PropertyContext {
            model_clone: ctx.movie.properties.clone(),
            commit_needed: false,
            propery_changed: false,
        };
        let iterator = properties.iter().map(|property| {
            |ui: &mut egui::Ui, context: &mut PropertyContext| {
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
        if ui.available_width() > (properties.len() * 140) as f32 {
            horizontal_layout(ui, &mut context, iterator);
        } else {
            // TODO: change amount of rows based on available width
            vertical_grid_layout(ui, &mut context, iterator);
        }

        if context.propery_changed {
            // only send change message when the properties actually changed
            ctx.message_bus
                .publish(EditorMessage::NewEdit(EditMessage::Change(
                    MovieChange::MovieProperties(context.model_clone),
                )));
        }
        if context.commit_needed {
            ctx.message_bus
                .publish(EditorMessage::NewEdit(EditMessage::Commit));
        }
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
) {
    let mut iterator = iterator.peekable();
    egui::Grid::new("movie_properties_grid").show(ui, |ui| {
        let mut column_index = 0;
        loop {
            egui::Grid::new(format!("movie_properties_grid_inner_{}", column_index)).show(
                ui,
                |ui| {
                    for _ in 0..2 {
                        if let Some(callback) = iterator.next() {
                            (callback)(ui, context);
                            ui.end_row();
                        }
                    }
                },
            );
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

struct Property<Model, ValueType> {
    name: String,
    get: fn(model: &Model) -> ValueType,
    set: fn(model: &mut Model, value: ValueType),
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
impl_numeric_properties!(for f32, f64);

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
