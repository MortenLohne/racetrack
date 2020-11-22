use std::io::{BufWriter, Result};
use std::time::Duration;

use crate::cli::CliOptions;
use crate::engine::EngineBuilder;
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

fn main() -> Result<()> {
    let cli_args = cli::parse_cli_arguments();

    let openings = match &cli_args.book_path {
        Some(path) => openings::openings_from_file(path)?,
        None => vec![vec![]],
    };

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
