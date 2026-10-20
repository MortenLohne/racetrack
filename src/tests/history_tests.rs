use std::time::Duration;

use board_game_traits::Position as _;
use tiltak::position::{Move, Position, Role, Square};

use crate::game::{ExternalGameState, ExternalMove};
use crate::history::MatchHistory;
use crate::openings::Opening;
use crate::uci::UciInfo;

fn place(square: u8) -> Move<6> {
    Move::placement(Role::Flat, Square::from_u8(square))
}

fn state(round_number: usize, opening_len: usize, played: usize) -> ExternalGameState<Position<6>> {
    ExternalGameState {
        white_player: "white".to_string(),
        black_player: "black".to_string(),
        opening: Opening {
            root_position: Position::start_position(),
            moves: (0..opening_len).map(|i| place(i as u8)).collect(),
        },
        moves: (0..played)
            .map(|i| ExternalMove {
                mv: place((10 + i) as u8),
                uci_info: UciInfo::default(),
            })
            .collect(),
        round_number,
        current_move_uci_info: None,
        white_time_left: Duration::from_secs(1),
        black_time_left: Duration::from_secs(2),
        white_time: Duration::from_secs(600),
        white_increment: Duration::from_secs(5),
        black_time: Duration::from_secs(300),
        black_increment: Duration::from_secs(3),
    }
}

fn history() -> MatchHistory<Position<6>> {
    let mut history = MatchHistory::new(Some(10));
    history.set_engines(vec!["a".to_string(), "b".to_string()]);
    history
}

#[test]
fn results_sorted_by_round() {
    let mut history = history();
    for round in [3, 1, 2] {
        history.record(0, 1, Some("1-0"), state(round, 0, 1));
    }
    let rounds: Vec<usize> = history.results().iter().map(|r| r.round_number).collect();
    assert_eq!(rounds, vec![1, 2, 3]);
}

#[test]
fn missing_result_is_star() {
    let mut history = history();
    history.record(0, 1, None, state(1, 0, 1));
    assert_eq!(history.results()[0].result, "*");
}

#[test]
fn result_string_is_kept() {
    let mut history = history();
    history.record(0, 1, Some("0-R"), state(1, 0, 1));
    assert_eq!(history.results()[0].result, "0-R");
}

#[test]
fn plies_include_opening() {
    let mut history = history();
    history.record(0, 1, Some("1-0"), state(1, 3, 2));
    assert_eq!(history.results()[0].plies, 5);
}

#[test]
fn snapshot_drops_live_info() {
    let mut history = history();
    let mut s = state(1, 0, 1);
    s.current_move_uci_info = Some(UciInfo::default());
    history.record(0, 1, Some("1-0"), s);
    assert!(history.game(1).unwrap().current_move_uci_info.is_none());
}

#[test]
fn engines_by_index_with_same_name() {
    let mut history = MatchHistory::new(None);
    history.set_engines(vec!["tiltak".to_string(), "tiltak".to_string()]);
    history.record(1, 0, Some("1-0"), state(1, 0, 1));
    let result = &history.results()[0];
    assert_eq!(result.white_engine, 1);
    assert_eq!(result.black_engine, 0);
    assert_eq!(history.engines(), ["tiltak", "tiltak"]);
    assert_eq!(history.total_rounds(), None);
}

#[test]
fn unknown_round_is_none() {
    let history = history();
    assert!(history.game(99).is_none());
}

#[cfg(feature = "http")]
mod http_output {
    use super::*;
    use crate::http::{finished_game_output, GameStateOutput};

    fn named_history(total_rounds: Option<usize>) -> MatchHistory<Position<6>> {
        let mut history = MatchHistory::new(total_rounds);
        history.set_engines(vec!["syntaks".to_string(), "takzero".to_string()]);
        history
    }

    #[test]
    fn output_includes_results_and_engines() {
        let mut history = named_history(Some(20));
        history.record(0, 1, Some("1-0"), state(1, 0, 2));
        let value = serde_json::to_value(GameStateOutput::new(state(2, 0, 1), &history)).unwrap();
        assert_eq!(value["engines"], serde_json::json!(["syntaks", "takzero"]));
        assert_eq!(value["results"][0]["whiteEngine"], 0);
        assert_eq!(value["results"][0]["roundNumber"], 1);
        assert_eq!(value["totalRounds"], 20);
    }

    #[test]
    fn output_omits_total_rounds_for_sprt() {
        let history = named_history(None);
        let value = serde_json::to_value(GameStateOutput::new(state(1, 0, 1), &history)).unwrap();
        assert!(value.get("totalRounds").is_none());
    }

    #[test]
    fn finished_game_output_has_moves() {
        let mut history = named_history(Some(20));
        history.record(0, 1, Some("1-0"), state(1, 0, 2));
        let output = finished_game_output(&history, 1).unwrap();
        let value = serde_json::to_value(output).unwrap();
        assert_eq!(value["moves"].as_array().unwrap().len(), 2);
        assert!(value["currentMoveUciInfo"].is_null());
    }

    #[test]
    fn output_includes_time_control() {
        let history = named_history(Some(20));
        let value = serde_json::to_value(GameStateOutput::new(state(1, 0, 1), &history)).unwrap();
        assert_eq!(
            value["whiteTime"],
            serde_json::json!({"secs": 600, "nanos": 0})
        );
        assert_eq!(
            value["whiteIncrement"],
            serde_json::json!({"secs": 5, "nanos": 0})
        );
        assert_eq!(
            value["blackTime"],
            serde_json::json!({"secs": 300, "nanos": 0})
        );
        assert_eq!(
            value["blackIncrement"],
            serde_json::json!({"secs": 3, "nanos": 0})
        );
    }

    #[test]
    fn finished_game_output_includes_time_control() {
        let mut history = named_history(Some(20));
        history.record(0, 1, Some("1-0"), state(1, 0, 2));
        let value = serde_json::to_value(finished_game_output(&history, 1).unwrap()).unwrap();
        assert_eq!(
            value["whiteTime"],
            serde_json::json!({"secs": 600, "nanos": 0})
        );
        assert_eq!(
            value["blackIncrement"],
            serde_json::json!({"secs": 3, "nanos": 0})
        );
    }

    #[test]
    fn finished_game_output_unknown_round() {
        let history = named_history(Some(20));
        assert!(finished_game_output(&history, 7).is_none());
    }
}

#[test]
fn record_move_updates_moves_and_clocks() {
    let mut game = state(1, 0, 1);
    game.current_move_uci_info = Some(UciInfo::default());
    game.record_move(
        place(20),
        UciInfo::default(),
        Duration::ZERO,
        Duration::from_secs(2),
    );
    assert_eq!(game.moves.len(), 2);
    assert!(game.current_move_uci_info.is_none());
    assert_eq!(game.white_time_left, Duration::ZERO);
    assert_eq!(game.black_time_left, Duration::from_secs(2));
}
