use crate::data::message::{MessageDataList, MessageList};

use super::{alphabet::{Alphabet, MAX_UNITS}, unit_totals::UnitTotals};

/**
 * Sorted frequency data for all units in a collection of messages. Information
 * which maps a frequency to a specific unit is destroyed, so this is only
 * useful for comparing with other frequency distributions.
 */
#[derive(Clone)]
pub struct UnitFrequency {
    pub name: Box<str>,
    pub data: [f64; MAX_UNITS],
}

impl Default for UnitFrequency {
    fn default() -> Self {
        UnitFrequency {
            name: "".into(),
            data: [0.0; MAX_UNITS],
        }
    }
}

impl UnitFrequency {
    pub fn from_alphabet(alphabet: &Alphabet) -> UnitFrequency {
        let mut freq = UnitFrequency::default();

        let mut weight_total = 0f64;
        let mut u: usize = 0;
        for (_, alpha_unit) in alphabet.iter_units() {
            let weight = alpha_unit.weight;
            weight_total += weight;
            freq.data[u] = weight;
            u += 1;
        }

        // normalize weights, to make sure they're actually frequencies
        if weight_total != 1.0 && weight_total != 0.0 {
            for i in 0..u {
                freq.data[i] /= weight_total;
            }
        }

        freq.name = alphabet.get_name().clone();
        freq.sort();
        freq
    }

    pub fn from_unit_totals(totals: &UnitTotals) -> UnitFrequency {
        let mut total: usize = 0;
        for i in totals.data {
            total += i;
        }

        let mut freq = UnitFrequency { name: "".into(), data: [0f64; MAX_UNITS] };
        for i in 0..MAX_UNITS {
            freq.data[i] = totals.data[i] as f64 / total as f64;
        }

        freq.sort();
        freq
    }

    pub fn from_unit_totals_with_name(name: &str, totals: &UnitTotals) -> UnitFrequency {
        let mut x = UnitFrequency::from_unit_totals(totals);
        x.name = name.into();
        x
    }

    pub fn from_messages(messages: &MessageList) -> UnitFrequency {
        UnitFrequency::from_unit_totals(&UnitTotals::from_messages(messages))
    }

    pub fn from_message_data_list(messages: &MessageDataList) -> UnitFrequency {
        UnitFrequency::from_unit_totals(&UnitTotals::from_message_data_list(messages))
    }

    pub fn get_error(&self, other: &UnitFrequency) -> f64 {
        let mut error: f64 = 0.0;

        for i in 0..MAX_UNITS {
            error += (self.data[i] - other.data[i]).abs();
        }

        error
    }

    pub fn sort(&mut self) {
        self.data.sort_by(|a, b| b.partial_cmp(a).unwrap());
    }
}

// TODO compare character at index i with character at index i - 1; basically subtract but take possible modulo into account, to measure if there's a consistent "drift"