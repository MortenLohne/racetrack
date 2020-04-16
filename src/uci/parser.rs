use crate::uci::{UciError, UciErrorKind, UciOption, UciOptionType};
use std::result;
use std::str::FromStr;

pub fn parse_option(input: &str) -> result::Result<UciOption, UciError> {
    let mut words_iter = input.split_whitespace();
    assert_eq!(words_iter.next(), Some("option"));

    #[derive(PartialEq, Eq, Debug, Clone, Copy)]
    enum ParserState {
        Name,
        Type,
        Default,
        Min,
        Max,
        Var,
    }

    impl ParserState {
        fn from_keyword(word: &str) -> Option<ParserState> {
            match word {
                "name" => Some(ParserState::Name),
                "type" => Some(ParserState::Type),
                "default" => Some(ParserState::Default),
                "min" => Some(ParserState::Min),
                "max" => Some(ParserState::Max),
                "var" => Some(ParserState::Var),
                _ => None,
            }
        }
    }

    let mut parser_state = None;

    let mut name = vec![];
    let mut uci_type = vec![];
    let mut default = vec![];
    let mut min = vec![];
    let mut max = vec![];
    let mut var = vec![];

    while let Some(word) = words_iter.next() {
        if let Some(new_parser_state) = ParserState::from_keyword(word) {
            parser_state = Some(new_parser_state);
            if new_parser_state == ParserState::Var {
                var.push(vec![]);
            }
        } else {
            match parser_state {
                Some(ParserState::Name) => name.push(word),
                Some(ParserState::Type) => uci_type.push(word),
                Some(ParserState::Default) => default.push(word),
                Some(ParserState::Min) => min.push(word),
                Some(ParserState::Max) => max.push(word),
                Some(ParserState::Var) => var.last_mut().unwrap().push(word),
                None => {
                    return Err(UciError::new_parse_error(format!(
                        "Invalid start of option description. Expected \"name\", got \"{}\"",
                        word
                    )))
                }
            }
        }
    }

    if name.len() == 0 {
        return Err(UciError::new_root(
            UciErrorKind::InvalidOption,
            format!("Got option string without a name \"{}\"", input),
        ));
    }

    let name = name.join(" ");

    if uci_type.len() != 1 {
        return Err(UciError::new_root(
            UciErrorKind::InvalidOption,
            format!(
                "Expected 1 type for option \"{}\", got {}",
                name,
                uci_type.len()
            ),
        ));
    }
    Ok(match uci_type[0] {
        "check" => {
            let default = bool::from_str(&default.join(" ")).map_err(|err| {
                UciError::new_caused_by(
                    UciErrorKind::ParseError,
                    format!("Couldn't parse default for option \"{}\"", name),
                    Box::new(err),
                )
            });
            UciOption {
                name,
                option_type: UciOptionType::Check(default?),
            }
        }
        "spin" => {
            let default = i64::from_str(&default.join(" ")).map_err(|err| {
                UciError::new_caused_by(
                    UciErrorKind::ParseError,
                    format!("Couldn't parse default for option \"{}\"", name),
                    Box::new(err),
                )
            });

            let min = i64::from_str(&min.join(" ")).map_err(|err| {
                UciError::new_caused_by(
                    UciErrorKind::ParseError,
                    format!("Couldn't parse default for option \"{}\"", name),
                    Box::new(err),
                )
            });

            let max = i64::from_str(&max.join(" ")).map_err(|err| {
                UciError::new_caused_by(
                    UciErrorKind::ParseError,
                    format!("Couldn't parse default for option \"{}\"", name),
                    Box::new(err),
                )
            });

            UciOption {
                name,
                option_type: UciOptionType::Spin(default?, min?, max?),
            }
        }
        "combo" => UciOption {
            name,
            option_type: UciOptionType::Combo(
                default.join(" "),
                var.iter().map(|var| var.join(" ")).collect(),
            ),
        },
        "button" => UciOption {
            name,
            option_type: UciOptionType::Button,
        },
        "string" => {
            if default.len() > 1 && default.contains(&"<empty>") {
                return Err(UciError::new_parse_error(format!(
                    "Wrong default value {} for {}. Cannot both be empty and non-empty",
                    default.join(" "),
                    name
                )));
            }
            let default: String = if default == vec!["<empty>"] {
                String::new()
            } else {
                default.join(" ")
            };
            UciOption {
                name,
                option_type: UciOptionType::String(default),
            }
        }
        s => {
            return Err(UciError::new_parse_error(format!(
                "Option {} has invalid type {}",
                name, s
            )))
        }
    })
}
