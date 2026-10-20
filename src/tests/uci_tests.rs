use crate::uci::{parser, UciOption, UciOptionType};
use std::time::Duration;

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

#[test]
fn parse_tc_test() {
    assert_eq!(
        parser::parse_tc("60+0.6").unwrap(),
        (Duration::from_secs(60), Duration::from_millis(600))
    );
    assert_eq!(
        parser::parse_tc("0.5+0.1").unwrap(),
        (Duration::from_millis(500), Duration::from_millis(100))
    );
}

#[test]
fn parse_cp_info_with_wdl_test() {
    // Syntaks' format, including an aspiration-window bound
    let info = parser::parse_info_string(
        "info depth 24 seldepth 40 time 3781 nodes 79191264 nps 20942367 score cp 30 lowerbound wdl 264 606 130 hashfull 12 pv e3 e5",
    )
    .unwrap();
    assert_eq!(info.cp_score, Some(30));
    assert_eq!(info.mate, None);
    assert_eq!(info.wdl, Some([264, 606, 130]));
    assert_eq!(info.pv, vec!["e3", "e5"]);
}

#[test]
fn parse_mate_info_test() {
    // Syntaks reports forced wins as mate scores only, in moves
    let info = parser::parse_info_string(
        "info depth 30 seldepth 9 time 120 nodes 1000 nps 8333 score mate 3 wdl 1000 0 0 hashfull 1 pv a1 a2 a3 a4 a5",
    )
    .unwrap();
    assert_eq!(info.cp_score, None);
    assert_eq!(info.mate, Some(3));
    assert_eq!(info.wdl, Some([1000, 0, 0]));
    assert_eq!(info.pv.len(), 5);
}

#[test]
fn parse_mate_and_cp_info_test() {
    // Takzero reports a mate score followed by a cp score on the same line
    let info = parser::parse_info_string(
        "info time 1000 nodes 500 nps 500 wdl 0 0 1000 score mate -2 score cp -100 pv a1 a2 a3",
    )
    .unwrap();
    assert_eq!(info.cp_score, Some(-100));
    assert_eq!(info.mate, Some(-2));
    assert_eq!(info.wdl, Some([0, 0, 1000]));
    assert_eq!(info.pv, vec!["a1", "a2", "a3"]);
}

#[test]
fn parse_info_without_score_test() {
    assert!(parser::parse_info_string("info depth 12 currmove a1 currmovenumber 3").is_err());
    assert!(parser::parse_info_string("info depth 12 score lowerbound pv a1").is_err());
}

#[test]
fn bounded_info_is_detected_test() {
    // Syntaks' aspiration-window fail-high with a truncated PV
    assert!(parser::is_bounded_info(
        "info depth 24 seldepth 40 time 3781 nodes 79191264 nps 20942367 score cp 30 lowerbound wdl 264 606 130 hashfull 12 pv e3"
    ));
    assert!(parser::is_bounded_info(
        "info depth 24 score cp -30 upperbound pv e3"
    ));
    assert!(parser::is_bounded_info(
        "info depth 30 score mate 3 upperbound pv a1"
    ));
}

#[test]
fn exact_info_is_not_bounded_test() {
    assert!(!parser::is_bounded_info(
        "info depth 24 seldepth 40 time 3781 nodes 79191264 nps 20942367 score cp 30 wdl 264 606 130 hashfull 12 pv e3 e5"
    ));
    assert!(!parser::is_bounded_info(
        "info depth 30 score mate 3 wdl 1000 0 0 pv a1 a2"
    ));
    // A bound keyword that doesn't qualify the score is not a bound
    assert!(!parser::is_bounded_info(
        "info depth 3 score cp 10 nodes 5 lowerbound pv a1"
    ));
}
