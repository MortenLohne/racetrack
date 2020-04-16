use std::error::Error;
use std::fmt;

pub mod parser;

#[derive(Debug)]
pub struct UciError {
    kind: UciErrorKind,
    desc: String,
    source: Option<Box<dyn Error>>,
}

impl UciError {
    pub fn new_parse_error(desc: String) -> UciError {
        UciError {
            kind: UciErrorKind::ParseError,
            desc,
            source: None,
        }
    }

    pub fn new_root(kind: UciErrorKind, desc: String) -> UciError {
        UciError {
            kind,
            source: None,
            desc,
        }
    }

    pub fn new_caused_by(kind: UciErrorKind, desc: String, source: Box<dyn Error>) -> UciError {
        UciError {
            kind,
            source: Some(source),
            desc,
        }
    }
}

impl fmt::Display for UciError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            UciErrorKind::ParseError => write!(f, "Uci parser error. {}", self.desc)?,
            UciErrorKind::InvalidOption => write!(f, "invalid option description. {}", self.desc)?,
        }
        if let Some(ref source) = self.source {
            write!(f, ". Caused by: {}", source)?
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum UciErrorKind {
    ParseError,
    InvalidOption,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq)]
pub struct UciOption {
    pub name: String,
    pub option_type: UciOptionType,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq)]
pub enum UciOptionType {
    Check(bool),
    Spin(i64, i64, i64), // Contains current value, minimum value, maximum value
    Combo(String, Vec<String>), // Contains current value, predefined values
    Button,
    String(String), // Contains the current value
}
