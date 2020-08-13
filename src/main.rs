use std::io::{BufWriter, Result};
use std::time::Duration;

use crate::cli::CliOptions;
use crate::engine::EngineBuilder;
use board_game_traits::board::Board as BoardTrait;
use pgn_traits::pgn::PgnBoard;
use std::fs;
use std::sync::Mutex;
use taik::board::{Board, Move};

mod cli;
mod engine;
mod game;
mod r#match;
mod openings;
pub mod pgn_writer;
#[cfg(test)]
mod tests;
pub mod uci;
use std::time;

fn main() -> Result<()> {
    for n in 1..=4 {
        let openings = openings::all_flatstone_n_ply_openings(n);
        println!(
            "{} openings: {:?}",
            openings.len(),
            openings.iter().take(10).collect::<Vec<_>>()
        );

        let start_time = time::Instant::now();

        openings::print_opening_evals(openings.clone());
        println!("Evaluated {} openings in {:.1}s", openings.len(), start_time.elapsed().as_secs_f64())
    }

    let cli_args = cli::parse_cli_arguments();

    let mut openings = vec![];

    for opening in openings::OPENING_MOVE_TEXTS.iter() {
        let mut board = Board::start_board();
        let moves: Vec<Move> = opening
            .iter()
            .map(|move_string| {
                let mv = board.move_from_san(move_string).unwrap();
                board.do_move(mv.clone());
                mv
            })
            .collect();
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
