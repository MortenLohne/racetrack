use crate::engine::{UciError, UciErrorKind, UciOption, UciOptionType};
use std::result;
use std::str::FromStr;

pub fn parse_option(input: &str) -> result::Result<UciOption, UciError> {
    let mut words_iter = input.split_whitespace();
    assert_eq!(words_iter.next(), Some("option"));

    // TODO: Goes into infinite loop if no name is found
    while words_iter.next() != Some("name") {}

    let name: String = words_iter
        .by_ref()
        .take_while(|word| *word != "type")
        .collect::<Vec<_>>()
        .join(" ");

    let option_type_name = match words_iter.next() {
        None => {
            return Err(UciError::new_parse_error(format!(
                "No option type for option \"{}\"",
                name
            )));
        }
        Some(s) => s,
    };

    let mut default = None;
    let mut min = None;
    let mut max = None;
    let mut vars = vec![];

    while let Some(word) = words_iter.next() {
        match word {
            "default" => default = words_iter.next(),
            "min" => min = words_iter.next(),
            "max" => max = words_iter.next(),
            "var" => {
                words_iter.next().map(|var| vars.push(var));
            }
            s => {
                return Err(UciError::new_parse_error(format!(
                    "Couldn't parse option parameter \"{}\" for \"{}\"",
                    s, input
                )))
            }
        }
    }

    let uci_option_type = match option_type_name {
        "check" => UciOptionType::Check(
            default
                .map(bool::from_str)
                .map(result::Result::ok)
                .flatten()
                .ok_or_else(|| {
                    UciError::new_parse_error(format!(
                        "Couldn't parse default value for option \"{}\"",
                        name
                    ))
                })?,
        ),
        _ => unimplemented!(),
    };

    // TODO: Actually return the proper option type
    Ok(UciOption {
        name,
        option_type: UciOptionType::Button,
    })
}
