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