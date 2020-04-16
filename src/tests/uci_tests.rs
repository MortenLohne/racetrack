use crate::uci::{UciOption, UciOptionType, parser};
use crate::{engine, };

#[test]
fn parse_check_option_description() {
    let option_string = "option name hard mode type check default false";

    assert_eq!(
        parser::parse_option(option_string).unwrap(),
        UciOption {
            name: "hard mode".to_string(),
            option_type: UciOptionType::Check(false)
        }
    )
}

#[test]
fn parse_spin_option_description() {
    let option_string = "option name Threads type spin min 1 max 256 default 1";

    assert_eq!(
        parser::parse_option(option_string).unwrap(),
        UciOption {
            name: "Threads".to_string(),
            option_type: UciOptionType::Spin(1, 1, 256)
        }
    )
}

#[test]
fn parse_var_option_description() {
    let option_string =
        "option name Chess Variant type combo var Normal Chess var Crazyhouse default Normal Chess";

    assert_eq!(
        parser::parse_option(option_string).unwrap(),
        UciOption {
            name: "Chess Variant".to_string(),
            option_type: UciOptionType::Combo(
                "Normal Chess".to_string(),
                vec!["Normal Chess".to_string(), "Crazyhouse".to_string()]
            )
        }
    )
}

#[test]
fn parse_empty_default() {
    let option_string = "option name NalimovPath type string default <empty>";

    assert_eq!(
        parser::parse_option(option_string).unwrap(),
        UciOption {
            name: "NalimovPath".to_string(),
            option_type: UciOptionType::String("".to_string())
        }
    )
}

#[test]
fn parse_option_description_without_name() {
    let option_string = "option Threads type spin min 1 max 256 default 1";

    assert!(parser::parse_option(option_string).is_err())
}
