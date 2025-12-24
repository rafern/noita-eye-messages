use crate::data::message::{InterleavedMessageData, MessageDataList, MessageList};

use super::alphabet::MAX_UNITS;

/**
 * The total occurrences of all units in a collection of messages. Each key
 * represents a unit, and each value represents the total occurrences of that
 * unit.
 */
pub struct UnitTotals {
    pub data: [usize; MAX_UNITS],
}

impl UnitTotals {
    pub fn from_messages(messages: &MessageList) -> UnitTotals {
        let mut counter = UnitTotals { data: [0; MAX_UNITS] };
        for message in messages.iter() {
            for c in message.data.iter() {
                counter.data[*c as usize] += 1;
            }
        }

        counter
    }

    pub fn from_message_data_list(message_data_list: &MessageDataList) -> UnitTotals {
        let mut counter = UnitTotals { data: [0; MAX_UNITS] };
        for data in message_data_list.iter() {
            for c in data.iter() {
                counter.data[*c as usize] += 1;
            }
        }

        counter
    }

    pub fn from_interleaved_message_data(interleaved_message_data: &InterleavedMessageData) -> UnitTotals {
        let mut counter = UnitTotals { data: [0; MAX_UNITS] };
        for m in 0..interleaved_message_data.get_message_count() {
            // SAFETY: m iterated over valid range
            for u in 0..unsafe { interleaved_message_data.get_unit_count(m) } {
                counter.data[interleaved_message_data[(m, u)] as usize] += 1;
            }
        }

        counter
    }
}
