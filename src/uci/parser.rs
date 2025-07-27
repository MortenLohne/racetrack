use crate::uci::{UciError, UciErrorKind, UciInfo, UciOption, UciOptionType};
use std::result;
use std::str::FromStr;
use std::time::Duration;

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

    for word in words_iter {
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

    if name.is_empty() {
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

pub fn parse_info_string(input: &str) -> Result<UciInfo, UciError> {
    let mut pv: Vec<String> = vec![];
    let mut cp_score = None;
    let mut depth: u16 = 0;
    let mut seldepth: u16 = 0;
    let mut time = 0;
    let mut nodes: u64 = 0;
    let mut nps: u64 = 0;

    // These words are "special", and are used to delimit multi-word infos such as "pv"
    const KEYWORDS: &[&str] = &[
        "depth",
        "seldepth",
        "nodes",
        "score",
        "time",
        "nps",
        "pv",
        "multipv",
        "currmove",
        "currmovenumber",
        "hashfull",
        "tbhits",
        "cpuload",
        "string",
        "refutation",
        "currline",
    ];

    let mut words_iter = input.split_whitespace().peekable();
    assert_eq!(words_iter.next(), Some("info"));
    while let Some(next_token) = words_iter.next() {
        match next_token {
            "score" => match words_iter.next() {
                Some("cp") => {
                    if let Some(cp_string) = words_iter.next() {
                        cp_score = Some(i64::from_str(cp_string).map_err(|err| {
                            UciError::new_caused_by(
                                UciErrorKind::ParseError,
                                format!("Failed to parse cp score \"{}\"", cp_string),
                                Box::new(err),
                            )
                        })?);
                    } else {
                        return Err(UciError::new_parse_error("No cp score".to_string()));
                    }
                }
                _ => return Err(UciError::new_parse_error("Invalid score".to_string())),
            },
            "pv" => {
                if !pv.is_empty() {
                    return Err(UciError::new_parse_error(
                        "Received multiple PVs".to_string(),
                    ));
                }
                while words_iter
                    .peek()
                    .is_some_and(|word| !KEYWORDS.contains(word))
                {
                    pv.push(words_iter.next().unwrap().to_string());
                }
            }
            "depth" => {
                if let Some(depth_string) = words_iter.next() {
                    match depth_string.parse() {
                        Ok(d) => depth = d,
                        Err(err) => {
                            return Err(UciError::new_caused_by(
                                UciErrorKind::ParseError,
                                format!("Failed to parse depth \"{}\"", depth_string),
                                Box::new(err),
                            ))
                        }
                    }
                } else {
                    return Err(UciError::new_parse_error(
                        "Did not receive a value for key \"depth\"".to_string(),
                    ));
                }
            }
            "seldepth" => {
                let d = words_iter
                    .next()
                    .ok_or(UciError::new_parse_error(
                        "Did not receive a value for key \"seldepth\"".to_string(),
                    ))?
                    .parse()
                    .map_err(|err| {
                        UciError::new_caused_by(
                            UciErrorKind::ParseError,
                            "Failed to parse seldepth".to_string(),
                            Box::new(err),
                        )
                    })?;
                seldepth = d;
            }
            "time" => {
                let time_ms = words_iter
                    .next()
                    .ok_or(UciError::new_parse_error(
                        "Did not receive a value for key \"time\"".to_string(),
                    ))?
                    .parse()
                    .map_err(|err| {
                        UciError::new_caused_by(
                            UciErrorKind::ParseError,
                            "Failed to parse time".to_string(),
                            Box::new(err),
                        )
                    })?;
                time = time_ms;
            }
            "nodes" => {
                let n = words_iter
                    .next()
                    .ok_or(UciError::new_parse_error(
                        "Did not receive a value for key \"nodes\"".to_string(),
                    ))?
                    .parse()
                    .map_err(|err| {
                        UciError::new_caused_by(
                            UciErrorKind::ParseError,
                            "Failed to parse nodes".to_string(),
                            Box::new(err),
                        )
                    })?;
                nodes = n;
            }
            "nps" => {
                let n = words_iter
                    .next()
                    .ok_or(UciError::new_parse_error(
                        "Did not receive a value for key \"nps\"".to_string(),
                    ))?
                    .parse()
                    .map_err(|err| {
                        UciError::new_caused_by(
                            UciErrorKind::ParseError,
                            "Failed to parse nps".to_string(),
                            Box::new(err),
                        )
                    })?;
                nps = n;
            }
            _ => (),
        }
    }
    if let Some(score) = cp_score {
        Ok(UciInfo {
            depth,
            seldepth,
            time,
            nodes,
            nps,
            hashfull: 0.0,
            cp_score: score,
            pv,
        })
    } else {
        Err(UciError::new_root(
            UciErrorKind::MissingField,
            "Info string had no score".to_string(),
        ))
    }
}

pub fn parse_tc(input: &str) -> Result<(Duration, Duration), UciError> {
    let error = || UciError::new_parse_error(format!("Couldn't parse tc \"{}\"", input));
    let mut parts = input.split('+');
    let time_part = parts.next().ok_or_else(error)?;
    let inc_part = parts.next();
    let time =
        Duration::from_millis((f64::from_str(time_part).map_err(|_| error())? * 1000.0) as u64);

    let inc = if let Some(inc_part) = inc_part {
        Duration::from_millis((f64::from_str(inc_part).map_err(|_| error())? * 1000.0) as u64)
    } else {
        Duration::default()
    };
    Ok((time, inc))
}
