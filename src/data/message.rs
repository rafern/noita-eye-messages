use smallvec::SmallVec;

// TODO separate metadata (name) from data, and interleave messages so that
//      indices from different messages are near each other, which should
//      provide a substancial speed-up due to better cache locality

#[derive(Clone, Default)]
pub struct Message {
    // only need 137 for original messages, but can get 143 for free due to
    // alignment requirements
    pub data: SmallVec<[u8; 143]>,
    pub name: Box<str>,
}

impl Message {
    pub fn from_name(name: Box<str>) -> Self {
        Self { name, data: SmallVec::new() }
    }
}

pub type MessageList = SmallVec<[Message; 9]>;