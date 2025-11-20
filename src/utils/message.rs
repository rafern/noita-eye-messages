use super::stackvec::StackVec;

pub const MAX_MESSAGE_COUNT: usize = 9;
pub const MAX_MESSAGE_SIZE: usize = 137 /*256 - 24*/;

#[derive(Clone, Default)]
pub struct Message {
    pub name: String,
    pub data: StackVec<u8, MAX_MESSAGE_SIZE>,
}

pub type MessageList = StackVec<Message, MAX_MESSAGE_COUNT>;