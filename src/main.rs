use std::io::{BufWriter, Result};

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
        Some(path) => match cli_args.size {
            4 => openings::openings_from_file::<4>(path),
            5 => openings::openings_from_file::<5>(path),
            6 => openings::openings_from_file::<6>(path),
            7 => openings::openings_from_file::<7>(path),
            8 => openings::openings_from_file::<8>(path),
            s => panic!("Size {} not supported", s),
        }?,
        None => vec![vec![]],
    };
    match cli_args.size {
        4 => run_match::<4>(openings, cli_args)?,
        5 => run_match::<5>(openings, cli_args)?,
        6 => run_match::<6>(openings, cli_args)?,
        7 => run_match::<7>(openings, cli_args)?,
        8 => run_match::<8>(openings, cli_args)?,
        s => panic!("Size {} not supported", s),
    }

    Ok(())
}

fn run_match<const S: usize>(openings: Vec<Vec<Move>>, cli_args: CliOptions) -> Result<()> {
    let engine_builders: Vec<EngineBuilder> = cli_args
        .engine_paths
        .iter()
        .zip(cli_args.engine_args.iter())
        .map(|(path, args)| EngineBuilder { path, args })
        .collect();

    let settings: r#match::TournamentSettings<Board<S>> = r#match::TournamentSettings {
        size: cli_args.size,
        concurrency: cli_args.concurrency,
        time: cli_args.time,
        increment: cli_args.increment,
        openings,
        num_minimatches: (cli_args.games + 1) / 2,
        pgn_writer: cli_args.pgnout.as_ref().map(|pgnout| {
            Mutex::new(r#match::PgnWriter::new(BufWriter::new(
                fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(pgnout)
                    .unwrap(),
            )))
        }),
    };

    println!("CLI args: {:?}", cli_args);
    println!("Settings: {:?}", settings);

    let _ = r#match::play_match(
        &settings,
        engine_builders[0].clone(),
        engine_builders[1].clone(),
    );
    Ok(())
}
