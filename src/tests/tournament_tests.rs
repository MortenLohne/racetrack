use std::{io, num::NonZeroUsize, sync::Mutex};

use board_game_traits::Position as PositionTrait;
use tiltak::position::{Move, Position, Role, Square};

use crate::{
    game::ScheduledGame,
    openings::Opening,
    pgn_writer::PgnWriter,
    tournament::{EngineId, TournamentSettings, TournamentType},
};

fn dummy_tournament(
    num_games: usize,
    tournament_type: TournamentType,
) -> TournamentSettings<Position<6>> {
    TournamentSettings {
        size: 6,
        position_settings: Default::default(),
        concurrency: 1,
        num_games,
        openings: (0..num_games)
            .map(|i| Opening {
                root_position: Position::start_position(),
                moves: vec![Move::placement(Role::Flat, Square::from_u8(i as u8))],
            })
            .collect(),
        openings_start_index: 0,
        pgn_writer: Mutex::new(PgnWriter::new(io::empty())),
        tournament_type,
    }
}

#[test]
fn head_to_head_test() {
    let head_to_head_settings: TournamentSettings<Position<6>> =
        dummy_tournament(12, TournamentType::HeadToHead);

    let head_to_head_games = head_to_head_settings.schedule();

    // Check game #1
    assert_eq!(
        head_to_head_games[0],
        ScheduledGame {
            round_number: 0,
            opening: Opening {
                root_position: Position::start_position(),
                moves: vec![Move::placement(Role::Flat, Square::from_u8(0))],
            },
            white_engine_id: EngineId(0),
            black_engine_id: EngineId(1),
            size: 6
        }
    );

    // Check game #2
    assert_eq!(
        head_to_head_games[1],
        ScheduledGame {
            round_number: 1,
            opening: Opening {
                root_position: Position::start_position(),
                moves: vec![Move::placement(Role::Flat, Square::from_u8(0))],
            },
            white_engine_id: EngineId(1),
            black_engine_id: EngineId(0),
            size: 6
        }
    );

    // Engine #1 has white half the time
    assert_eq!(
        head_to_head_games
            .iter()
            .filter(|game| game.white_engine_id == EngineId(0))
            .count(),
        head_to_head_games.len() / 2
    );

    // 6 different openings are played in the 12 games
    let mut openings: Vec<_> = head_to_head_games
        .iter()
        .map(|game| game.opening.clone())
        .collect();
    openings.dedup();
    assert_eq!(openings.len(), 6);
}

#[test]
fn single_gauntlet_test() {
    let head_to_head_settings: TournamentSettings<Position<6>> =
        dummy_tournament(12, TournamentType::HeadToHead);
    let head_to_head_games = head_to_head_settings.schedule();

    let gauntlet_settings: TournamentSettings<Position<6>> =
        dummy_tournament(12, TournamentType::Gauntlet(NonZeroUsize::new(1).unwrap()));
    let gauntlet_games = gauntlet_settings.schedule();

    // A gauntlet with one challenger is just a head-to-head tournament
    for (head_to_head, gauntlet) in head_to_head_games.iter().zip(gauntlet_games.iter()) {
        assert_eq!(head_to_head, gauntlet);
    }
}

#[test]
fn double_gauntlet_test() {
    let gauntlet_settings: TournamentSettings<Position<6>> =
        dummy_tournament(12, TournamentType::Gauntlet(NonZeroUsize::new(2).unwrap()));
    let gauntlet_games = gauntlet_settings.schedule();

    // 3 different openings are played in the 12 games
    let mut openings: Vec<_> = gauntlet_games
        .iter()
        .map(|game| game.opening.clone())
        .collect();
    openings.dedup();
    assert_eq!(openings.len(), 3);

    // The champion plays white in game #1 and #2
    assert_eq!(gauntlet_games[0].white_engine_id, EngineId(0));
    assert_eq!(gauntlet_games[1].white_engine_id, EngineId(0));

    // The challengers take turns playing black
    assert_eq!(gauntlet_games[0].black_engine_id, EngineId(1));
    assert_eq!(gauntlet_games[1].black_engine_id, EngineId(2));

    // The champion plays in half the games
    assert!(gauntlet_games
        .iter()
        .all(|game| game.white_engine_id == EngineId(0) || game.black_engine_id == EngineId(0)));

    // The champion plays white in half the games
    assert_eq!(
        gauntlet_games
            .iter()
            .filter(|game| game.white_engine_id == EngineId(0))
            .count(),
        gauntlet_games.len() / 2
    );

    // The challengers play white in 1/4 the games each
    assert_eq!(
        gauntlet_games
            .iter()
            .filter(|game| game.white_engine_id == EngineId(1))
            .count(),
        gauntlet_games.len() / 4
    );
}
