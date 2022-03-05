use board_game_traits::Position;
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
            UciErrorKind::InvalidOption => write!(f, "Invalid option description. {}", self.desc)?,
            UciErrorKind::MissingField => write!(f, "Missing field. {}", self.desc)?,
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
    MissingField,
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

impl UciOptionType {
    pub fn value_is_supported(&self, value: &str) -> bool {
        match self {
            UciOptionType::Check(_) => value == "true" || value == "false",
            UciOptionType::Spin(_, min, max) => {
                if let Ok(int_value) = value.parse::<i64>() {
                    int_value >= *min && int_value <= *max
                } else {
                    false
                }
            }
            UciOptionType::Combo(_, possible_values) => possible_values.iter().any(|e| e == value),
            UciOptionType::Button => false,
            UciOptionType::String(_) => true,
        }
    }

    pub fn set_value(&mut self, value: &str) {
        match self {
            UciOptionType::Check(val) => *val = value.parse().unwrap(),
            UciOptionType::Spin(val, _, _) => *val = value.parse().unwrap(),
            UciOptionType::Combo(val, _) => *val = value.to_string(),
            UciOptionType::Button => (),
            UciOptionType::String(val) => *val = value.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct UciInfo<B: Position> {
    pub depth: u16,
    pub seldepth: u16,
    pub time: i64,
    pub nodes: u64,
    pub hashfull: f64,
    pub cp_score: i64,
    pub pv: Vec<B::Move>, // One or more principal variations, sorted from best to worst
}
