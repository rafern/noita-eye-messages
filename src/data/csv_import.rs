use std::{fmt, process};
use std::error::Error;
use crate::analysis::freq::{UnitFrequency, sort_freq};

use super::message::{MAX_MESSAGE_COUNT, MAX_MESSAGE_SIZE, Message, MessageList};

#[derive(Debug)]
pub enum InvalidFormatErrorKind {
    EmptyMessageName,
    EmptyMessage,
    InvalidDatum,
    MessageLimitExceeded,
    MessageLengthLimitExceeded,
    UnexpectedDatum,
    MissingLanguageName,
    UnitLimitExceeded,
}

#[derive(Debug)]
pub struct InvalidFormatError {
    pub kind: InvalidFormatErrorKind,
    pub row: usize,
    pub col: usize,
}

impl fmt::Display for InvalidFormatError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} at row {}, column {}", match self.kind {
            InvalidFormatErrorKind::EmptyMessageName => "empty message name",
            InvalidFormatErrorKind::EmptyMessage => "empty message",
            InvalidFormatErrorKind::InvalidDatum => "invalid datum",
            InvalidFormatErrorKind::MessageLimitExceeded => "message limit exceeded (please recompile with a higher message limit)",
            InvalidFormatErrorKind::MessageLengthLimitExceeded => "message length limit exceeded (please recompile with a higher message length limit)",
            InvalidFormatErrorKind::UnexpectedDatum => "unexpected datum",
            InvalidFormatErrorKind::MissingLanguageName => "missing language name",
            InvalidFormatErrorKind::UnitLimitExceeded => "language unit limit exceeded (you are probably loading the wrong file)",
        }, self.row + 1, self.col + 1)
    }
}

impl Error for InvalidFormatError {}

pub fn import_csv_messages(path: &std::path::PathBuf) -> Result<MessageList, Box<dyn Error>> {
    let csv = std::fs::read_to_string(path)?;

    let mut messages = MessageList::default();
    let mut r = 0;
    for row in csv.split('\n') {
        let row_trim = row.trim();
        if row_trim.len() > 0 {
            let mut c = 0;

            if messages.len() >= MAX_MESSAGE_COUNT {
                return Err(InvalidFormatError { kind: InvalidFormatErrorKind::MessageLimitExceeded, row: r, col: c }.into());
            }

            let mut message = Message::default();
            let mut first = true;
            for col in row.split(',') {
                let col_trim = col.trim();

                if first {
                    if col_trim.len() == 0 {
                        return Err(InvalidFormatError { kind: InvalidFormatErrorKind::EmptyMessageName, row: r, col: c }.into());
                    }

                    message.name = String::from(col_trim);
                    first = false;
                } else {
                    if message.data.len() >= MAX_MESSAGE_SIZE {
                        return Err(InvalidFormatError { kind: InvalidFormatErrorKind::MessageLengthLimitExceeded, row: r, col: c }.into());
                    }

                    message.data.push(col.parse::<u8>().or(Err(InvalidFormatError { kind: InvalidFormatErrorKind::InvalidDatum, row: r, col: c }))?);
                }

                c += 1;
            }

            if first || message.data.len() == 0 {
                return Err(InvalidFormatError { kind: InvalidFormatErrorKind::EmptyMessage, row: r, col: c }.into());
            }

            messages.push(message);
        }

        r += 1;
    }

    Ok(messages)
}

pub fn import_csv_messages_or_exit(path: &std::path::PathBuf) -> MessageList {
    match import_csv_messages(path) {
        Err(e) => {
            eprintln!("Failed to read data CSV: {}", e);
            process::exit(1);
        },
        Ok(v) => v
    }
}

pub fn import_csv_languages(paths: &Vec<std::path::PathBuf>) -> Result<Vec<UnitFrequency>, Box<dyn Error>> {
    let mut freqs: Vec<UnitFrequency> = Vec::new();

    for path in paths {
        let mut freq = UnitFrequency::default();
        let mut u = 0;

        let csv = std::fs::read_to_string(path)?;
        let mut first = true;
        let mut r = 0;
        for row in csv.split('\n') {
            let row_trim = row.trim();
            if row_trim.len() > 0 {
                if first {
                    freq.name = String::from(row_trim);
                    first = false;
                } else {
                    let mut c = 0;
                    for col in row.split(',') {
                        if c == 1 {
                            if u == freq.data.len() {
                                return Err(InvalidFormatError { kind: InvalidFormatErrorKind::UnitLimitExceeded, row: r, col: c }.into());
                            }

                            freq.data[u] = col.parse::<f64>().or(Err(InvalidFormatError { kind: InvalidFormatErrorKind::InvalidDatum, row: r, col: c }))?;
                            u += 1;
                        } else if c > 1 {
                            return Err(InvalidFormatError { kind: InvalidFormatErrorKind::UnexpectedDatum, row: r, col: c }.into());
                        }

                        c += 1;
                    }
                }
            }

            r += 1;
        }

        if first {
            return Err(InvalidFormatError { kind: InvalidFormatErrorKind::MissingLanguageName, row: r, col: 0 }.into());
        }

        sort_freq(&mut freq);
        freqs.push(freq);
    }

    Ok(freqs)
}

pub fn import_csv_languages_or_exit(paths: &Vec<std::path::PathBuf>) -> Vec<UnitFrequency> {
    match import_csv_languages(paths) {
        Err(e) => {
            eprintln!("Failed to read language CSV: {}", e);
            process::exit(1);
        },
        Ok(v) => v
    }
}