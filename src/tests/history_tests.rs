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
