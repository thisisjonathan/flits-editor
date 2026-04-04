use flits_core::MovieProperties;

use crate::{editor::Context, edits::MovieChange, message::EditorMessage, undo::EditMessage};

#[derive(Default)]
pub struct PropertiesPanel2 {}
impl PropertiesPanel2 {
    pub fn do_ui(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        let properties: [Box<dyn PropertyTrait<MovieProperties>>; 3] = [
            Box::new(Property {
                name: "Width".into(),
                get: |model: &MovieProperties| model.width,
                set: |model: &mut MovieProperties, value| model.width = value,
            }),
            Box::new(Property {
                name: "Height".into(),
                get: |model: &MovieProperties| model.height,
                set: |model: &mut MovieProperties, value| model.height = value,
            }),
            Box::new(Property::<MovieProperties, f32> {
                name: "Framerate".into(),
                get: |model: &MovieProperties| model.frame_rate,
                set: |model: &mut MovieProperties, value| model.frame_rate = value,
            }),
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
impl<Model, ValueType> PropertyTrait<Model> for Property<Model, ValueType>
where
    ValueType: egui::emath::Numeric,
{
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
