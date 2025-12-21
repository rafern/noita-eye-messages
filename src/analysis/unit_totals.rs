use crate::data::message::{MessageDataList, MessageList};

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
}
