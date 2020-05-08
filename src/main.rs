use std::io::{BufWriter, Result};
use std::time::Duration;

use crate::cli::CliOptions;
use crate::engine::EngineBuilder;
use board_game_traits::board::Board as BoardTrait;
use pgn_traits::pgn::PgnBoard;
use std::fs;
use std::sync::Mutex;
use taik::board::{Board, Move, Piece, TunableBoard};
use taik::mcts;

mod cli;
mod engine;
mod game;
mod r#match;
pub mod pgn_writer;
#[cfg(test)]
mod tests;
pub mod uci;

fn main() -> Result<()> {
    let cli_args = cli::parse_cli_arguments();

    let mut openings = vec![];

    for opening in OPENING_MOVE_TEXTS.iter() {
        let mut board = Board::start_board();
        let moves : Vec<Move> = opening.iter().map(|move_string| {
            let mv = board.move_from_san(move_string).unwrap();
            board.do_move(mv.clone());
            mv
        }).collect();
        openings.push(moves);
    }

    run_match(openings, cli_args)?;

    Ok(())
}

fn run_match(openings: Vec<Vec<Move>>, cli_args: CliOptions) -> Result<()> {
    let engine_builders: Vec<EngineBuilder> = cli_args
        .engine_paths
        .iter()
        .map(|path| EngineBuilder { path })
        .collect();

    let settings: r#match::TournamentSettings<Board> = r#match::TournamentSettings {
        concurrency: cli_args.concurrency,
        time_per_move: Duration::from_millis(1000),
        openings,
        num_minimatches: 106,
        pgn_writer: cli_args.pgnout.map(|pgnout| {
            Mutex::new(r#match::PgnWriter::new(BufWriter::new(
                fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(pgnout)
                    .unwrap(),
            )))
        }),
    };
    let games = r#match::play_match(
        &settings,
        engine_builders[0].clone(),
        engine_builders[1].clone(),
    );
    Ok(())
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

const OPENING_MOVE_TEXTS: [&[&str]; 106] = [
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
