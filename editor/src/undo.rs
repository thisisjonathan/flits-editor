#[derive(Debug, Clone)]
pub enum EditMessage<Change, Action>
where
    Change: ChangeEdit,
    Action: ActionEdit,
{
    Action(Action),
    Change(Change),
    Commit,
    Undo,
    Redo,
}
#[derive(Debug, Clone)]
enum Edit<Change: ChangeEdit, Action: ActionEdit> {
    Value(ValueEdit<Change>),
    Action(Action),
}
impl<Change: ChangeEdit, Action: ActionEdit<Model = Change::Model>> Edit<Change, Action> {
    fn apply(&self, model: &mut Change::Model) {
        match self {
            Edit::Value(value_edit) => value_edit.after.apply(model),
            Edit::Action(action) => action.apply(model),
        }
    }
    fn invert(self) -> Self {
        match self {
            Edit::Value(value_edit) => Edit::Value(ValueEdit {
                before: value_edit.after,
                after: value_edit.before,
            }),
            Edit::Action(action_edit) => Edit::Action(action_edit.invert()),
        }
    }
}
pub trait ActionEdit: std::fmt::Debug + Clone {
    type Model;
    fn apply(&self, model: &mut Self::Model);
    fn invert(self) -> Self;
}
pub trait ChangeEdit: std::fmt::Debug + Clone {
    type Model;
    fn apply(&self, model: &mut Self::Model);
    fn existing_value(&self, model: &Self::Model) -> Self;
}
#[derive(Debug, Clone)]
struct ValueEdit<Change: std::fmt::Debug + Clone> {
    before: Change,
    after: Change,
}

pub struct UndoStack<Change, Action>
where
    Change: ChangeEdit + 'static,
    Action: ActionEdit + 'static,
{
    undo_stack: Vec<Edit<Change, Action>>,
    redo_stack: Vec<Edit<Change, Action>>,
    preview_edit: Option<ValueEdit<Change>>,
}
impl<Change: ChangeEdit, Action: ActionEdit<Model = Change::Model>> UndoStack<Change, Action> {
    pub fn new() -> Self {
        UndoStack {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            preview_edit: None,
        }
    }
    pub fn update(&mut self, model: &mut Change::Model, message: EditMessage<Change, Action>) {
        match message {
            EditMessage::Action(action) => {
                action.apply(model);
                self.undo_stack.push(Edit::Action(action));
                self.redo_stack.clear();
            }
            EditMessage::Change(change) => {
                if let Some(preview_edit) = &self.preview_edit {
                    if std::mem::discriminant(&change)
                        != std::mem::discriminant(&preview_edit.before)
                    {
                        panic!(
                            "Made change overriding change of a different type. Types: {:?} {:?}",
                            std::mem::discriminant(&change),
                            std::mem::discriminant(&preview_edit.before)
                        );
                    }
                    change.apply(model);
                    self.preview_edit = Some(ValueEdit {
                        before: preview_edit.before.clone(),
                        after: change.clone(),
                    });
                } else {
                    self.preview_edit = Some(ValueEdit {
                        before: change.existing_value(model),
                        after: change.clone(),
                    });
                    change.apply(model);
                }
            }
            EditMessage::Commit => {
                if let Some(preview_edit) = self.preview_edit.take() {
                    self.undo_stack.push(Edit::Value(preview_edit));
                    self.redo_stack.clear();
                }
            }
            EditMessage::Undo => {
                let Some(edit) = self.undo_stack.pop() else {
                    return;
                };
                self.redo_stack.push(edit.clone());
                edit.invert().apply(model);
            }
            EditMessage::Redo => {
                let Some(edit) = self.redo_stack.pop() else {
                    return;
                };
                edit.apply(model);
                self.undo_stack.push(edit);
            }
        }
    }
}
