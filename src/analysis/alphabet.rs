use std::{collections::{BTreeMap, HashMap, btree_map::Iter}, error::Error, fmt};

use unicode_segmentation::UnicodeSegmentation;

// can fit all ASCII chars (or a reasonable amount of unicode), although it's
// probably just 83 chars unless you're doing practice exercises
pub const MAX_UNITS: usize = 256;

#[derive(Debug)]
pub enum AlphabetError {
    InvalidGrapheme,
    DuplicateGrapheme,
    UnitLimitExceeded,
    DuplicateUnit,
}

impl fmt::Display for AlphabetError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match self {
            Self::InvalidGrapheme => "Invalid grapheme",
            Self::DuplicateGrapheme => "Duplicate grapheme",
            Self::UnitLimitExceeded => "Unit limit exceeded",
            Self::DuplicateUnit => "Duplicate unit",
        })
    }
}

impl Error for AlphabetError {}

pub struct AlphabetUnit {
    pub grapheme: Box<str>,
    pub weight: f64,
}

impl AlphabetUnit {
    pub fn is_printable(&self) -> bool {
        self.grapheme.len() > 0
    }
}

pub struct Alphabet {
    name: Box<str>,
    units: BTreeMap<u8, AlphabetUnit>,
    grapheme_map: HashMap<Box<str>, u8>,
}

impl Alphabet {
    pub fn new(name: Box<str>) -> Self {
        Self { name, units: BTreeMap::new(), grapheme_map: HashMap::new() }
    }

    pub fn len(&self) -> usize {
        self.units.len()
    }

    pub fn iter_units(&self) -> Iter<'_, u8, AlphabetUnit> {
        self.units.iter()
    }

    pub fn get_name(&self) -> &Box<str> {
        &self.name
    }

    pub fn add_unit(&mut self, unit: u8, grapheme: Box<str>, weight: f64) -> Result<(), AlphabetError> {
        if self.len() >= MAX_UNITS {
            Err(AlphabetError::UnitLimitExceeded)
        } else if grapheme.graphemes(true).count() != 1 {
            Err(AlphabetError::InvalidGrapheme)
        } else if self.grapheme_map.contains_key(&grapheme) {
            Err(AlphabetError::DuplicateGrapheme)
        } else if self.units.contains_key(&unit) {
            Err(AlphabetError::DuplicateUnit)
        } else {
            self.grapheme_map.insert(grapheme.clone(), unit);
            self.units.insert(unit, AlphabetUnit { grapheme, weight });
            Ok(())
        }
    }

    pub fn add_anonymous_unit(&mut self, unit: u8, weight: f64) -> Result<(), AlphabetError> {
        if self.len() >= MAX_UNITS {
            Err(AlphabetError::UnitLimitExceeded)
        } else if self.units.contains_key(&unit) {
            Err(AlphabetError::DuplicateUnit)
        } else {
            self.units.insert(unit, AlphabetUnit { grapheme: "".into(), weight });
            Ok(())
        }
    }

    pub fn get_unit(&self, idx: u8) -> Option<&AlphabetUnit> {
        self.units.get(&idx)
    }

    pub fn get_unit_idx(&self, grapheme: &Box<str>) -> Option<u8> {
        self.grapheme_map.get(grapheme).copied()
    }

    pub fn get_unit_min(&self) -> u8 {
        let mut min = u8::MAX;
        for (u, alpha_unit) in self.units.iter() {
            if !alpha_unit.is_printable() { continue }

            let u = *u;
            if u < min {
                min = u;
            }
        }

        min
    }
}

impl Default for Alphabet {
    fn default() -> Self {
        let mut alphabet = Alphabet::new("ASCII".into());

        for u in 0x00..=0x1fu8 {
            alphabet.add_anonymous_unit(u, 0.0).expect("expected default alphabet to never fail creation");
        }

        for u in 0x20..=0x7eu8 {
            alphabet.add_unit(
                u,
                str::from_utf8(&[u]).expect("expected default alphabet to never fail creation").into(),
                0.0
            ).expect("expected default alphabet to never fail creation");
        }

        for u in 0x7f..=0xffu8 {
            alphabet.add_anonymous_unit(u, 0.0).expect("expected default alphabet to never fail creation");
        }

        alphabet
    }
}