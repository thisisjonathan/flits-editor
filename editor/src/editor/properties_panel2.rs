use flits_core::{EditorColor, MovieProperties, PlaceSymbol, PreloaderType};

use crate::{
    editor::Context,
    edits::{MovieAction, MovieChange, PlacedSymbolChange},
    message::EditorMessage,
    undo::EditMessage,
};

#[derive(Default)]
pub struct PropertiesPanel2 {}
impl PropertiesPanel2 {
    pub fn do_ui(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        match ctx.selection.placed_symbols.len() {
            0 => self.show_panel(ui, ctx, ctx.movie.properties.clone(), |model| {
                EditMessage::Change(MovieChange::MovieProperties(model))
            }),
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
            ),
            _ => {
                ui.label("Multiple items selected");
                return;
            }
        }
    }
    fn show_panel<T: PanelType>(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &Context,
        model: T,
        edit_message: impl FnOnce(T) -> EditMessage<MovieChange, MovieAction>,
    ) {
        ui.heading(&model.name());
        let properties = model.properties();
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
        let iterator = properties.iter().map(|property| {
            |ui: &mut egui::Ui, context: &mut PropertyContext<T>| {
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
                .publish(EditorMessage::NewEdit((edit_message)(context.model_clone)));
        }
        if context.commit_needed {
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

macro_rules! property {
    ($name:literal, $model: ident, $type:expr ) => {
        Box::new(Property {
            name: $name.into(),
            get: |$model: &Self| $type.clone(),
            set: |$model: &mut Self, value| $type = value,
        })
    };
}

type PropertyBox<T> = Box<dyn PropertyTrait<T>>;
trait PanelType {
    fn name(&self) -> String;
    fn properties(&self) -> Vec<PropertyBox<Self>>;
}

impl PanelType for MovieProperties {
    fn name(&self) -> String {
        "Movie properties".into()
    }

    fn properties(&self) -> Vec<PropertyBox<Self>> {
        vec![
            property!("Width", model, model.width),
            property!("Height", model, model.height),
            property!("Framerate", model, model.frame_rate),
            // if i remember correctly, the spec specifies this as rgb. the alpha is ignored (TODO: check)
            property!("Background color", model, model.background_color),
            property!("Preloader", model, model.preloader),
        ]
    }
}

impl PanelType for PlaceSymbol {
    fn name(&self) -> String {
        "Placed symbol properties".into()
    }

    fn properties(&self) -> Vec<PropertyBox<Self>> {
        vec![
            property!("x", model, model.transform.x),
            property!("y", model, model.transform.y),
            property!("X scale", model, model.transform.x_scale),
            property!("Y scale", model, model.transform.y_scale),
        ]
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
