use crate::utils::stackvec::StackVec;

pub const MAX_MESSAGE_COUNT: usize = 9;
pub const MAX_MESSAGE_SIZE: usize = 256 - 24;

#[derive(Clone, Default)]
pub struct Message {
    pub name: String,
    pub data: StackVec<u8, MAX_MESSAGE_SIZE>,
}

impl Message {
    pub fn from_name(name: String) -> Self {
        Self { name, data: StackVec::default() }
    }
}

pub type MessageList = StackVec<Message, MAX_MESSAGE_COUNT>;

pub enum MessageRenderGroup {
    Plaintext { grapheme: String },
    HexUnit { unit: u8 },
    CiphertextRange { from: usize, to: usize },
}

pub struct RenderMessage {
    render_groups: Vec<MessageRenderGroup>,
    msg_len: usize,
}

impl RenderMessage {
    pub fn new(render_groups: Vec<MessageRenderGroup>) -> Self {
        let mut msg_len = 0usize;
        for group in &render_groups {
            msg_len += match group {
                MessageRenderGroup::CiphertextRange { from, to } => to - from,
                _ => 1,
            }
        }

        Self { render_groups, msg_len }
    }

    pub fn get_msg_len(&self) -> usize {
        self.msg_len
    }

    pub fn get_render_groups(&self) -> &Vec<MessageRenderGroup> {
        &self.render_groups
    }
}

pub struct MessageRenderMap {
    messages: MessageList,
    render_messages: Vec<RenderMessage>,
}

impl MessageRenderMap {
    pub fn new(messages: MessageList, render_messages: Vec<RenderMessage>) -> Self {
        debug_assert!(messages.len() == render_messages.len());
        Self { messages, render_messages }
    }

    pub fn get_messages(&self) -> &MessageList {
        &self.messages
    }

    pub fn get_render_messages(&self) -> &Vec<RenderMessage> {
        &self.render_messages
    }

    pub fn len(&self) -> usize {
        self.render_messages.len()
    }
}

pub struct RenderMessageBuilder {
    render_groups: Vec<MessageRenderGroup>,
    next_unit_range: Option<(usize, usize)>,
}

impl RenderMessageBuilder {
    pub fn new() -> Self {
        Self {
            render_groups: Vec::new(),
            next_unit_range: None,
        }
    }

    fn flush(&mut self) {
        if let Some((from, to)) = self.next_unit_range {
            self.render_groups.push(MessageRenderGroup::CiphertextRange { from, to });
            self.next_unit_range = None;
        }
    }

    pub fn push_hex(&mut self, unit: u8) {
        self.flush();
        self.render_groups.push(MessageRenderGroup::HexUnit { unit });
    }

    pub fn push_plaintext(&mut self, grapheme: String) {
        if grapheme.len() == 1 && grapheme.is_ascii() {
            let ascii = grapheme.as_bytes()[0];
            if ascii <= 0x1f || ascii == 0x7f {
                self.push_hex(ascii);
                return;
            }
        }

        self.flush();
        self.render_groups.push(MessageRenderGroup::Plaintext { grapheme });
    }

    pub fn push_unit(&mut self, unit_idx: usize) {
        if let Some(range) = &mut self.next_unit_range {
            debug_assert_eq!(unit_idx, range.1 + 1);
            range.1 = unit_idx + 1;
        } else {
            self.flush();
            self.next_unit_range = Some((unit_idx, unit_idx + 1));
        }
    }

    pub fn done(mut self) -> RenderMessage {
        self.flush();
        RenderMessage::new(self.render_groups)
    }
}