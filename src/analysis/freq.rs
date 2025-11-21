use crate::data::message::MessageList;

const UNITS: usize = 256; // assuming ASCII, although it's probably mod 83

/**
 * The total occurrences of all units in a collection of messages. Each key
 * represents a unit, and each value represents the total occurrences of that
 * unit.
 */
pub type UnitTotals = [usize; UNITS];

/**
 * Sorted frequency data for all units in a collection of messages. Information
 * which maps a frequency to a specific unit is destroyed, so this is only
 * useful for comparing with other frequency distributions.
 */
pub struct UnitFrequency {
    pub name: String,
    pub data: [f64; UNITS],
}

impl Default for UnitFrequency {
    fn default() -> Self {
        UnitFrequency {
            name: String::new(),
            data: [0.0; UNITS],
        }
    }
}

impl UnitFrequency {
    pub fn get_error(&self, other: &UnitFrequency) -> f64 {
        let mut error: f64 = 0.0;

        for i in 0..UNITS {
            error += (self.data[i] - other.data[i]).abs();
        }

        error
    }
}

pub fn count_units(messages: &MessageList) -> UnitTotals {
    let mut counter: UnitTotals = [0; UNITS];

    for message in messages.iter() {
        for c in message.data.iter() {
            counter[*c as usize] += 1;
        }
    }

    counter
}

pub fn sort_freq(freq: &mut UnitFrequency) {
    freq.data.sort_by(|a, b| b.partial_cmp(a).unwrap());
}

pub fn frequency_analysis(name: &str, totals: &UnitTotals) -> UnitFrequency {
    let mut total: usize = 0;
    for i in totals {
        total += i;
    }

    let mut freq = UnitFrequency { name: String::from(name), data: [0f64; UNITS] };
    for i in 0..UNITS {
        freq.data[i] = totals[i] as f64 / total as f64;
    }

    sort_freq(&mut freq);
    freq
}

// TODO compare character at index i with character at index i - 1; basically subtract but take possible modulo into account, to measure if there's a consistent "drift"