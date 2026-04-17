use crate::core::message::Message;

/// 会话状态管理
pub struct Conversation {
    messages: Vec<Message>,
    max_history: usize,
}

impl Conversation {
    pub fn new(max_history: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_history,
        }
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
        self.trim_history();
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    fn trim_history(&mut self) {
        if self.messages.len() > self.max_history {
            let drain_count = self.messages.len() - self.max_history;
            self.messages.drain(0..drain_count);
        }
    }
}
