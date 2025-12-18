use std::{error::Error, fmt};

#[derive(Debug)]
pub enum InvalidFormatErrorKind {
    EmptyMessageName,
    EmptyMessage,
    InvalidDatum,
    MessageLimitExceeded,
    MessageLengthLimitExceeded,
    UnexpectedDatum,
    MissingAlphabetName,
    UnitLimitExceeded,
    MissingAlphabetWeight,
    EmptyAlphabet,
    NoMessages,
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
            InvalidFormatErrorKind::MissingAlphabetName => "missing alphabet name",
            InvalidFormatErrorKind::UnitLimitExceeded => "alphabet unit limit exceeded (you are probably loading the wrong file)",
            InvalidFormatErrorKind::MissingAlphabetWeight => "missing weight for alphabet unit",
            InvalidFormatErrorKind::EmptyAlphabet => "empty alphabet",
            InvalidFormatErrorKind::NoMessages => "no messages",
        }, self.row + 1, self.col + 1)
    }
}

impl Error for InvalidFormatError {}