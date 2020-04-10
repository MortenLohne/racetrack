use std::io::Result;
use std::time::Duration;

use crate::engine::EngineBuilder;
use crate::game::play_game;
use board_game_traits::board::Board as BoardTrait;
use board_game_traits::board::GameResult::*;
use pgn_traits::pgn::PgnBoard;
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;
use std::sync::{Arc, Mutex};
use taik::board::{Board, Move, Piece, TunableBoard};
use taik::mcts;
use taik::pgn_writer::Game;

mod engine;
mod game;
mod uci_parser;

fn main() -> Result<()> {
    let opening_move_texts: [&[&str]; 106] = [
        &["a5", "b5"],
        &["a5", "c5"],
        &["a5", "d5"],
        &["a5", "e5"],
        &["a5", "b4"],
        &["a5", "c4"],
        &["a5", "d4"],
        &["a5", "e4"],
        &["a5", "c3", "c4"],
        &["a5", "c3", "b3"],
        &["a5", "c3", "d3"],
        &["a5", "c3", "c2"],
        &["a5", "d3"],
        &["a5", "e3"],
        &["a5", "d2"],
        &["a5", "e2"],
        &["a5", "e1"],
        &["b5", "a5"],
        &["b5", "c5"],
        &["b5", "d5", "d4"],
        &["b5", "d5", "d3"],
        &["b5", "d5", "d2"],
        &["b5", "e5"],
        &["b5", "a4"],
        &["b5", "b4"],
        &["b5", "c4"],
        &["b5", "d4", "b4"],
        &["b5", "d4", "c4"],
        &["b5", "d4", "d3"],
        &["b5", "e4", "b4"],
        &["b5", "e4", "c4"],
        &["b5", "a3"],
        &["b5", "b3"],
        &["b5", "c3", "c4"],
        &["b5", "c3", "b3"],
        &["b5", "c3", "c2"],
        &["b5", "d3"],
        &["b5", "e3"],
        &["b5", "a2"],
        &["b5", "b2"],
        &["b5", "c2"],
        &["b5", "d2", "d3"],
        &["b5", "d2", "b2"],
        &["b5", "d2", "c2"],
        &["b5", "e2", "b2"],
        &["b5", "e2", "c2"],
        &["b5", "a1"],
        &["b5", "b1"],
        &["b5", "c1"],
        &["b5", "d1", "d4"],
        &["b5", "d1", "d3"],
        &["b5", "d1", "d2"],
        &["b5", "e1"],
        &["c5", "a5"],
        &["c5", "b5"],
        &["c5", "a4"],
        &["c5", "b4"],
        &["c5", "c4"],
        &["c5", "a3"],
        &["c5", "b3"],
        &["c5", "c3", "b3"],
        &["c5", "c3", "d3"],
        &["c5", "a2"],
        &["c5", "b2"],
        &["c5", "c2"],
        &["c5", "a1"],
        &["c5", "b1"],
        &["c5", "c1"],
        &["b4", "a5"],
        &["b4", "b5"],
        &["b4", "c5"],
        &["b4", "d5"],
        &["b4", "e5"],
        &["b4", "c4"],
        &["b4", "d4"],
        &["b4", "e4"],
        &["b4", "c3", "c4"],
        &["b4", "c3", "b3"],
        &["b4", "d3"],
        &["b4", "e3"],
        &["b4", "d2"],
        &["b4", "e2"],
        &["b4", "e1"],
        &["c4", "a5"],
        &["c4", "b5"],
        &["c4", "c5"],
        &["c4", "a4"],
        &["c4", "b4"],
        &["c4", "a3"],
        &["c4", "b3"],
        &["c4", "c3", "b3"],
        &["c4", "c3", "d3"],
        &["c4", "c3", "b2"],
        &["c4", "c3", "c2"],
        &["c4", "c3", "d2"],
        &["c4", "a2"],
        &["c4", "b2"],
        &["c4", "c2"],
        &["c4", "a1"],
        &["c4", "b1"],
        &["c4", "c1"],
        &["c3", "a5"],
        &["c3", "b5"],
        &["c3", "c5"],
        &["c3", "b4"],
        &["c3", "c4"],
    ];

    let mut openings = vec![];

    for opening in opening_move_texts.iter() {
        let mut board = Board::start_board();
        let move1 = board.move_from_san(opening[0]).unwrap();
        board.do_move(move1.clone());
        openings.push(vec![move1, board.move_from_san(opening[1]).unwrap()]);
    }

    run_match(openings)?;

    Ok(())
}

fn run_match(openings: Vec<Vec<Move>>) -> Result<()> {
    let builder1 = EngineBuilder {
        path: "./taik_cpuct_1",
    };
    let builder2 = EngineBuilder {
        path: "./taik_cpuct_2",
    };

    let settings = Settings {
        concurrency: 2,
        time_per_move: Duration::from_millis(1000),
        openings,
        num_minimatches: 106,
    };
    let games = play_match(&settings, builder1, builder2);
    Ok(())
}

fn play_match(
    settings: &Settings,
    engine1: EngineBuilder,
    engine2: EngineBuilder,
) -> Result<Vec<Game<Board>>> {
    let engines: Vec<_> = (0..settings.concurrency)
        .map(|_| {
            let mut engine1 = engine1.init().unwrap();
            let mut engine2 = engine2.init().unwrap();
            (Arc::new(Mutex::new(engine1)), Arc::new(Mutex::new(engine2)))
        })
        .collect();

    ThreadPoolBuilder::new()
        .num_threads(settings.concurrency)
        .build_global()
        .unwrap();

    let games: Vec<_> = (0..settings.num_minimatches)
        .into_par_iter()
        .map(|round| {
            let thread_index = rayon::current_thread_index().unwrap();

            let mut white = engines[thread_index].0.try_lock().unwrap();
            let mut black = engines[thread_index].1.try_lock().unwrap();

            vec![
                play_game(
                    &settings,
                    &mut white,
                    &mut black,
                    &settings.openings[round as usize % settings.openings.len()],
                    round,
                ),
                play_game(
                    &settings,
                    &mut black,
                    &mut white,
                    &settings.openings[round as usize % settings.openings.len()],
                    round,
                ),
            ]
        })
        .flatten()
        .map(Result::unwrap)
        .collect();

    println!("Played {} games.", games.len());

    let mut engine1_wins = 0;
    let mut draws = 0;
    let mut engine2_wins = 0;

    let engine1_name = &engines[0].0.lock().unwrap().name().to_string();
    let engine2_name = &engines[0].1.lock().unwrap().name().to_string();

    for game in games.iter() {
        let (_, white_name) = game.tags.iter().find(|(tag, _val)| tag == "White").unwrap();
        match (white_name == engine1_name, game.game_result) {
            (true, Some(WhiteWin)) => engine1_wins += 1,
            (true, Some(BlackWin)) => engine2_wins += 1,
            (false, Some(WhiteWin)) => engine2_wins += 1,
            (false, Some(BlackWin)) => engine1_wins += 1,
            (_, None) | (_, Some(Draw)) => draws += 1,
        }
    }

    println!(
        "{} vs {}: +{}-{}={}",
        engine1_name, engine2_name, engine1_wins, engine2_wins, draws
    );

    Ok(games)
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Settings {
    concurrency: usize,
    time_per_move: Duration,
    num_minimatches: u64,
    openings: Vec<Vec<Move>>,
}

fn generate_openings(openings: &[Vec<Move>]) -> Vec<Vec<Move>> {
    let mut good_openings: Vec<_> = vec![];
    for opening in openings.iter() {
        let mut position = Board::start_board();
        for mv in opening.iter() {
            position.do_move(mv.clone());
        }

        let mut tree = mcts::Tree::new_root();
        let mut simple_moves = vec![];
        let mut moves = vec![];
        for _ in 0..20_000_000 {
            tree.select(
                &mut position.clone(),
                Board::VALUE_PARAMS,
                Board::POLICY_PARAMS,
                &mut simple_moves,
                &mut moves,
            );
        }
        println!("Analysis for opening {:?}", opening);
        tree.print_info();

        let alternative_moves: Vec<_> = tree
            .children
            .iter()
            .map(|(child, mv)| (mv.clone(), child.visits, child.mean_action_value))
            .filter(|(mv, visits, _)| {
                *visits > 50_000 && !matches!(mv, Move::Place(Piece::WhiteCap, _))
            })
            .collect();

        if alternative_moves.len() > 1 {
            for (mv, _, _) in alternative_moves.iter() {
                let mut good_opening = opening.clone();
                good_opening.push(mv.clone());
                println!("Added opening {:?}", good_opening);
                good_openings.push(good_opening)
            }
        } else {
            println!("Added opening {:?}", opening);
            good_openings.push(opening.clone());
        }
    }

    print!("[");
    for opening in good_openings.iter() {
        if opening.len() == 2 {
            print!("[\"{}\", \"{}\"], ", opening[0], opening[1]);
        } else {
            print!(
                "[\"{}\", \"{}\", \"{}\"], ",
                opening[0], opening[1], opening[2]
            );
        }
    }
    println!("]");

    good_openings
}
