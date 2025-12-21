use std::path::PathBuf;

use crate::{analysis::alphabet::Alphabet, utils::run::AnyErrorResult};

use super::format_error::{InvalidFormatError, InvalidFormatErrorKind};

pub fn import_csv_alphabet(path: &PathBuf) -> AnyErrorResult<Alphabet> {
    let csv = std::fs::read_to_string(path)?;
    let mut alphabet: Option<Alphabet> = None;
    let mut r = 0;

    for row in csv.split('\n') {
        let row_trim = row.trim();
        if row_trim.len() > 0 {
            match &mut alphabet {
                Some(alphabet) => {
                    let cols: Vec<&str> = row.split(',').collect();
                    if cols.len() > 3 {
                        return Err(InvalidFormatError { kind: InvalidFormatErrorKind::UnexpectedDatum, row: r, col: 3 }.into());
                    } else if cols.len() < 3 {
                        return Err(InvalidFormatError { kind: InvalidFormatErrorKind::MissingAlphabetWeight, row: r, col: cols.len() }.into());
                    } else {
                        alphabet.add_unit(
                            cols[0].parse::<u8>().or(Err(InvalidFormatError { kind: InvalidFormatErrorKind::InvalidDatum, row: r, col: 0 }))?,
                            cols[1].into(),
                            cols[2].parse::<f64>().or(Err(InvalidFormatError { kind: InvalidFormatErrorKind::InvalidDatum, row: r, col: 2 }))?,
                        )?;
                    }
                },
                None => {
                    alphabet = Some(Alphabet::new(row_trim.into()));
                },
            }
        }

        r += 1;
    }

    match alphabet {
        Some(alphabet) => {
            if alphabet.len() == 0 {
                return Err(InvalidFormatError { kind: InvalidFormatErrorKind::EmptyAlphabet, row: r, col: 0 }.into());
            }

            Ok(alphabet)
        },
        None => return Err(InvalidFormatError { kind: InvalidFormatErrorKind::MissingAlphabetName, row: r, col: 0 }.into()),
    }
}

pub fn import_csv_alphabet_or_default(path: &Option<PathBuf>) -> AnyErrorResult<Alphabet> {
    Ok(match path {
        Some(p) => import_csv_alphabet(p)?,
        None => Alphabet::default(),
    })
}