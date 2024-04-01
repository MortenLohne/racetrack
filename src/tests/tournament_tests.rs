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
        openings: (0..36)
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
fn two_engines_round_robin_test() {
    let head_to_head_settings: TournamentSettings<Position<6>> =
        dummy_tournament(12, TournamentType::RoundRobin(2));

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
fn double_gauntlet_test() {
    let gauntlet_settings: TournamentSettings<Position<6>> =
        dummy_tournament(24, TournamentType::Gauntlet(NonZeroUsize::new(2).unwrap()));
    let gauntlet_games = gauntlet_settings.schedule();

    // 3 different openings are played in the 24 games
    let mut openings: Vec<_> = gauntlet_games
        .iter()
        .map(|game| game.opening.clone())
        .collect();
    openings.dedup();
    assert_eq!(openings.len(), 6);

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

#[test]
fn round_robin_three_engines_test() {
    let round_robin_settings: TournamentSettings<Position<6>> =
        dummy_tournament(60, TournamentType::RoundRobin(3));
    let round_robin_games = round_robin_settings.schedule();

    assert_eq!(round_robin_games.len(), 60);

    // An engine should never play itself
    for game in round_robin_games.iter() {
        assert_ne!(
            game.white_engine_id, game.black_engine_id,
            "Game #{}: {:?} vs {:?}",
            game.round_number, game.white_engine_id, game.black_engine_id
        )
    }

    // Engine 1 plays white in 1/3 of games
    assert_eq!(
        round_robin_games
            .iter()
            .filter(|game| game.white_engine_id == EngineId(0))
            .count(),
        round_robin_games.len() / 3
    );

    // Engine 1 plays in 2/3 of games
    assert_eq!(
        round_robin_games
            .iter()
            .filter(
                |game| game.white_engine_id == EngineId(0) || game.black_engine_id == EngineId(0)
            )
            .count(),
        2 * round_robin_games.len() / 3,
        "{:?}",
        round_robin_games
    );
}

#[test]
fn round_robin_n_engines_test() {
    for num_engines in 2..=6 {
        let round_robin_settings: TournamentSettings<Position<6>> =
            dummy_tournament(60, TournamentType::RoundRobin(num_engines));
        let round_robin_games = round_robin_settings.schedule();

        assert_eq!(round_robin_games.len(), 60);

        // An engine should never play itself
        for game in round_robin_games.iter() {
            assert_ne!(
                game.white_engine_id, game.black_engine_id,
                "Game #{}: {:?} vs {:?}",
                game.round_number, game.white_engine_id, game.black_engine_id
            )
        }

        for id in 0..num_engines {
            // Each engine plays white in 1/n of games
            assert_eq!(
                round_robin_games
                    .iter()
                    .filter(|game| game.white_engine_id == EngineId(id))
                    .count(),
                round_robin_games.len() / num_engines
            );

            // Engine 1 plays in 2/n of games
            assert_eq!(
                round_robin_games
                    .iter()
                    .filter(|game| game.white_engine_id == EngineId(id)
                        || game.black_engine_id == EngineId(id))
                    .count(),
                2 * round_robin_games.len() / num_engines,
                "{:?}",
                round_robin_games
            );
        }
    }
}

#[test]
fn book_test_one_engine_test() {
    let book_test_settings: TournamentSettings<Position<6>> =
        dummy_tournament(60, TournamentType::BookTest(1));
    let book_test_games = book_test_settings.schedule();

    assert_eq!(book_test_games.len(), 60);

    // Openings are unique
    for game in book_test_games.iter().skip(1).take(10) {
        assert_ne!(game.opening, book_test_games[0].opening);
    }

    // The engine always plays itself
    for game in book_test_games.iter() {
        assert_eq!(game.white_engine_id, EngineId(0));
        assert_eq!(game.black_engine_id, EngineId(0));
    }
}

#[test]
fn book_test_n_engines_test() {
    for num_engines in 1..=5 {
        let book_test_settings: TournamentSettings<Position<6>> =
            dummy_tournament(3600, TournamentType::BookTest(num_engines));
        let book_test_games = book_test_settings.schedule();

        assert_eq!(book_test_games.len(), 3600);

        for id in 0..num_engines {
            // Each engine plays white in 1/n of games
            assert_eq!(
                book_test_games
                    .iter()
                    .filter(|game| game.white_engine_id == EngineId(id))
                    .count(),
                book_test_games.len() / num_engines
            );

            // Each engine plays in (1 + 2 * (n - 1))/n^2 of games
            assert_eq!(
                book_test_games
                    .iter()
                    .filter(|game| game.white_engine_id == EngineId(id)
                        || game.black_engine_id == EngineId(id))
                    .count(),
                book_test_games.len() * (2 * num_engines - 1) / (num_engines * num_engines),
                "{} engines",
                num_engines
            );
        }
    }
}
