use std::{fmt, process};
use std::error::Error;
use super::message::{MAX_MESSAGE_COUNT, MAX_MESSAGE_SIZE, Message, MessageList};

#[derive(Debug)]
pub enum InvalidFormatErrorKind {
    EmptyMessageName,
    EmptyMessage,
    InvalidDatum,
    MessageLimitExceeded,
    MessageLengthLimitExceeded,
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