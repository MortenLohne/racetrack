use std::io::{BufWriter, Result};
use std::{io, process, result};

use crate::cli::CliOptions;
use crate::engine::EngineBuilder;
use crate::openings::Opening;
use crate::pgn_writer::PgnWriter;
use crate::tournament::{Tournament, TournamentSettings};
use fern::InitError;
use log::error;
use std::fs;
use std::sync::Mutex;
use tiltak::position::{Position, Settings};

mod cli;
mod engine;
mod game;
mod openings;
mod pgn_writer;
#[cfg(test)]
mod tests;
mod tournament;
pub mod uci;

fn main() -> Result<()> {
    let cli_args = cli::parse_cli_arguments();
    println!("CLI args: {:?}", cli_args);

    if let Some(log_file_name) = &cli_args.log_file_name.as_ref() {
        setup_logger(log_file_name).map_err(|err| match err {
            InitError::Io(io_err) => io_err,
            InitError::SetLoggerError(_) => panic!("Logger already initialized"),
        })?;
    }

    match cli_args.size {
        4 => {
            let openings = get_openings(cli_args.book_path.as_ref());
            run_match::<4>(openings, cli_args)
        }
        5 => {
            let openings = get_openings(cli_args.book_path.as_ref());
            run_match::<5>(openings, cli_args)
        }
        6 => {
            let openings = get_openings(cli_args.book_path.as_ref());
            run_match::<6>(openings, cli_args)
        }
        7 => {
            let openings = get_openings(cli_args.book_path.as_ref());
            run_match::<7>(openings, cli_args)
        }
        8 => {
            let openings = get_openings(cli_args.book_path.as_ref());
            run_match::<8>(openings, cli_args)
        }
        s => panic!("Size {} not supported", s),
    }

    Ok(())
}

fn get_openings<const S: usize>(path: Option<&String>) -> Vec<Opening<Position<S>>> {
    assert!(S > 3 && S <= 8);
    println!("Loading opening book");
    if let Some(path) = path {
        openings::openings_from_file::<Position<S>>(path).expect("Valid")
    } else {
        vec![crate::openings::Opening::default()]
    }
}

fn setup_logger(file_name: &str) -> result::Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Debug)
        .chain(fern::log_file(file_name)?)
        .apply()?;
    Ok(())
}

fn run_match<const S: usize>(openings: Vec<Opening<Position<S>>>, cli_args: CliOptions) {
    let desired_uci_options = vec![(
        "HalfKomi".to_string(),
        cli_args.komi.half_komi().to_string(),
    )];

    let engine_builders: Vec<EngineBuilder> = cli_args
        .engine_paths
        .iter()
        .zip(cli_args.engine_args.iter())
        .map(|(path, args)| EngineBuilder {
            path: path.to_string(),
            args: args.clone(),
            desired_uci_options: desired_uci_options.clone(),
        })
        .collect();

    let pgnout = if let Some(file_name) = cli_args.pgnout.as_ref() {
        PgnWriter::new(BufWriter::new(
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(file_name)
                .unwrap(),
        ))
    } else {
        PgnWriter::new(io::sink())
    };

    let settings: TournamentSettings<Position<S>> = TournamentSettings {
        size: cli_args.size,
        position_settings: Settings {
            komi: cli_args.komi,
        },
        concurrency: cli_args.concurrency,
        time: cli_args.time,
        increment: cli_args.increment,
        openings,
        num_games: cli_args.games,
        pgn_writer: Mutex::new(pgnout),
    };

    let tournament = Tournament::new_head_to_head(settings);

    tournament.play(cli_args.concurrency, &engine_builders);
}

/// Utility for quickly exiting during initialization, generally due to a user error
/// Engines that have already been started still seem to get killed, at least on Linux
fn exit_with_error(error_message: &str) -> ! {
    eprintln!("{}", error_message);
    error!("{}", error_message);
    process::exit(1)
}
