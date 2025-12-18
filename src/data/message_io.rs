use std::io::Write;

use unicode_segmentation::UnicodeSegmentation;

use crate::{analysis::alphabet::Alphabet, utils::run::{AnyErrorResult, UnitResult}};

use super::{format_error::{InvalidFormatError, InvalidFormatErrorKind}, message::{Message, MessageList}};

pub fn export_csv_messages(path: &std::path::PathBuf, messages: &MessageList) -> UnitResult {
    let mut file = std::fs::File::create(path)?;
    let mut first = true;

    for message in messages.iter() {
        if first {
            first = false;
        } else {
            file.write(b"\n")?;
        }

        file.write(message.name.as_bytes())?;
        for c in message.data.iter() {
            file.write(format!(",{}", c).as_bytes())?;
        }
    }

    Ok(())
}

pub fn import_csv_messages(path: &std::path::PathBuf, alphabet: &Alphabet) -> AnyErrorResult<MessageList> {
    let csv = std::fs::read_to_string(path)?;

    let mut messages = MessageList::default();
    let mut r = 0;
    for row in csv.split('\n') {
        let row_trim = row.trim();
        if row_trim.len() > 0 {
            let mut c = 0;
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
                    let unit = col.parse::<u8>().or(Err(InvalidFormatError { kind: InvalidFormatErrorKind::InvalidDatum, row: r, col: c }))?;
                    if alphabet.get_unit(unit).is_some() && message.data.try_push(unit).is_none() {
                        return Err(InvalidFormatError { kind: InvalidFormatErrorKind::MessageLengthLimitExceeded, row: r, col: c }.into());
                    }
                }

                c += 1;
            }

            if first || message.data.len() == 0 {
                return Err(InvalidFormatError { kind: InvalidFormatErrorKind::EmptyMessage, row: r, col: c }.into());
            }

            if messages.try_push(message).is_none() {
                return Err(InvalidFormatError { kind: InvalidFormatErrorKind::MessageLimitExceeded, row: r, col: c }.into());
            }
        }

        r += 1;
    }

    if messages.len() == 0 {
        return Err(InvalidFormatError { kind: InvalidFormatErrorKind::NoMessages, row: r, col: 0 }.into());
    }

    Ok(messages)
}

pub fn import_txt_messages(path: &std::path::PathBuf, alphabet: &Alphabet) -> AnyErrorResult<MessageList> {
    let txt = std::fs::read_to_string(path)?;

    let mut messages = MessageList::default();
    let mut r = 0;
    for row in txt.split('\n') {
        let mut message = Message::from_name(format!("message-{}", messages.len()).into());
        let mut c = 0;

        for grapheme in row.graphemes(true) {
            let unit = alphabet.get_unit_idx(&grapheme.into());
            if let Some(unit) = unit {
                if message.data.try_push(unit).is_none() {
                    return Err(InvalidFormatError { kind: InvalidFormatErrorKind::MessageLengthLimitExceeded, row: r, col: c }.into());
                }
            }

            c += 1;
        }

        if message.data.len() == 0 {
            return Err(InvalidFormatError { kind: InvalidFormatErrorKind::EmptyMessage, row: r, col: 0 }.into());
        }

        if messages.try_push(message).is_none() {
            return Err(InvalidFormatError { kind: InvalidFormatErrorKind::MessageLimitExceeded, row: r, col: 0 }.into());
        }

        r += 1;
    }

    if messages.len() == 0 {
        return Err(InvalidFormatError { kind: InvalidFormatErrorKind::NoMessages, row: r, col: 0 }.into());
    }

    Ok(messages)
}

pub fn import_messages(data_path: &std::path::PathBuf, alphabet: &Alphabet) -> AnyErrorResult<MessageList> {
    let ext = data_path.extension();
    if let Some(ext) = ext && ext.to_ascii_lowercase() == "txt" {
        import_txt_messages(data_path, alphabet)
    } else {
        import_csv_messages(data_path, alphabet)
    }
}