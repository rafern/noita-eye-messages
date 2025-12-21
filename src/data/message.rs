use std::ops::{Index, IndexMut};

use smallvec::SmallVec;

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

pub struct InterleavedMessageData {
    message_count: usize,
    // only need 1233 (9 * 137) for original messages, but can get 1239 for free
    // due to alignment requirements
    inner: SmallVec<[u8; 1239]>,
    unit_counts: SmallVec<[usize; 9]>,
}

impl InterleavedMessageData {
    #[inline(always)]
    pub unsafe fn get_unchecked(&self, message_index: usize, unit_index: usize) -> &u8 {
        // FIXME: technically this is unsound, since the multiply add could
        //        overflow. not super important though, since who's going to be
        //        using messages that big?
        // SAFETY: caller must guarantee that message_index and unit_index are
        //         in-bounds
        unsafe { self.inner.get_unchecked(unit_index.unchecked_mul(self.message_count).unchecked_add(message_index)) }
    }

    #[inline(always)]
    pub unsafe fn get_unchecked_mut(&mut self, message_index: usize, unit_index: usize) -> &mut u8 {
        // SAFETY: caller must guarantee that message_index and unit_index are
        //         in-bounds
        unsafe { self.inner.get_unchecked_mut(unit_index.unchecked_mul(self.message_count).unchecked_add(message_index)) }
    }

    #[inline(always)]
    pub const fn get_message_count(&self) -> usize {
        self.message_count
    }

    #[inline(always)]
    pub unsafe fn get_unit_count(&self, message_index: usize) -> usize {
        // SAFETY: caller must guarantee that message_index is in-bounds
        unsafe { *self.unit_counts.get_unchecked(message_index) }
    }
}

impl Index<(usize, usize)> for InterleavedMessageData {
    type Output = u8;
    fn index(&self, idxs: (usize, usize)) -> &<Self as Index<(usize, usize)>>::Output {
        let (message_index, unit_index) = idxs;
        assert!(message_index < self.message_count);
        // SAFETY: message_index bounds verified by previous assert
        assert!(unit_index < unsafe { *self.unit_counts.get_unchecked(message_index) });
        // SAFETY: bounds verified by previous asserts
        unsafe { self.get_unchecked(message_index, unit_index) }
    }
}

impl IndexMut<(usize, usize)> for InterleavedMessageData {
    fn index_mut(&mut self, idxs: (usize, usize)) -> &mut <Self as Index<(usize, usize)>>::Output {
        let (message_index, unit_index) = idxs;
        assert!(message_index < self.message_count);
        // SAFETY: message_index bounds verified by previous assert
        assert!(unit_index < unsafe { *self.unit_counts.get_unchecked(message_index) });
        // SAFETY: bounds verified by previous asserts
        unsafe { self.get_unchecked_mut(message_index, unit_index) }
    }
}

pub struct AcceleratedMessageList {
    pub data: InterleavedMessageData,
    pub names: Vec<Box<str>>,
}

impl AcceleratedMessageList {
    pub fn from_messages(message_list: &MessageList) -> Self {
        let mut len_max = 0;
        let mut unit_counts = SmallVec::new();
        let mut names = Vec::new();
        let message_count = message_list.len();
        for msg in message_list.iter() {
            let len = msg.data.len();
            len_max = len_max.max(len);
            unit_counts.push(len);
            names.push(msg.name.clone());
        }

        let inner_len = len_max * message_count;
        let mut inner = SmallVec::with_capacity(inner_len);
        inner.resize(inner_len, 0);

        for m in 0..message_count {
            let msg_data = &message_list[m].data;
            for u in 0..msg_data.len() {
                inner[u * message_count + m] = msg_data[u];
            }
        }

        Self { data: InterleavedMessageData { message_count, inner, unit_counts }, names }
    }
}