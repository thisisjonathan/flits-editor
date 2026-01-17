use typed_arena::Arena;

pub struct MessageBus<Message> {
    messages: Arena<Message>,
}
impl<Message> MessageBus<Message> {
    pub fn new() -> Self {
        Self {
            messages: Arena::new(),
        }
    }
    pub fn publish(&self, msg: Message) {
        self.messages.alloc(msg);
    }
    pub fn into_vec(self) -> Vec<Message> {
        self.messages.into_vec()
    }
}
