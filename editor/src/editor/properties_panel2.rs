use flits_core::{MovieProperties, PreloaderType};

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

#[derive(Default)]
pub struct PropertiesPanel2 {}
impl PropertiesPanel2 {
    pub fn do_ui(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        let properties: [Box<dyn PropertyTrait<MovieProperties>>; 4] = [
            property!("Width", model, model.width),
            property!("Height", model, model.height),
            property!("Framerate", model, model.frame_rate),
            property!("Preloader", model, model.preloader),
        ];

        let mut model_clone = ctx.movie.properties.clone();
        let mut commit_needed = false;
        let mut propery_changed = false;
        for property in properties {
            let (needs_change, needs_commit) = property.do_ui(ui, &mut model_clone);
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
                .publish(EditorMessage::NewEdit(EditMessage::Change(
                    MovieChange::MovieProperties(model_clone),
                )));
        }
        if commit_needed {
            ctx.message_bus
                .publish(EditorMessage::NewEdit(EditMessage::Commit));
        }
    }
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
