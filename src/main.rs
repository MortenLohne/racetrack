use std::io::{BufWriter, Result};
use std::sync::atomic::{self, AtomicBool};
use std::{io, process, result};

use crate::cli::CliOptions;
use crate::engine::EngineBuilder;
use crate::pgn_writer::PgnWriter;
use crate::tournament::{Tournament, TournamentSettings};
use fern::InitError;
use log::error;
use openings::Opening;
use rand::seq::SliceRandom;
use std::fs;
use std::sync::Mutex;
use tiltak::position::{Position, Settings};

mod cli;
mod engine;
mod game;
mod openings;
mod pgn_writer;
mod simulation;
mod sprt;
#[cfg(test)]
mod tests;
mod tournament;
pub mod uci;
mod visualizer;

fn main() -> Result<()> {
    let cli_args = cli::parse_cli_arguments();
    println!("CLI args: {:?}", cli_args);

    match cli_args.size {
        4 => main_sized::<4>(cli_args),
        5 => main_sized::<5>(cli_args),
        6 => main_sized::<6>(cli_args),
        7 => main_sized::<7>(cli_args),
        8 => main_sized::<8>(cli_args),
        s => panic!("Size {} not supported", s),
    }
}

pub fn main_sized<const S: usize>(cli_args: CliOptions) -> Result<()> {
    let mut openings = match &cli_args.book_path {
        Some(path) => {
            println!("Loading opening book");
            openings::openings_from_file::<Position<S>>(path, cli_args.book_format)?
        }
        None => vec![Opening {
            root_position: Position::start_position_with_komi(cli_args.komi),
            moves: vec![],
        }],
    };

    if cli_args.shuffle_book {
        openings.shuffle(&mut rand::thread_rng());
    }

    if let Some(log_file_name) = &cli_args.log_file_name.as_ref() {
        setup_logger(log_file_name).map_err(|err| match err {
            InitError::Io(io_err) => io_err,
            InitError::SetLoggerError(_) => panic!("Logger already initialized"),
        })?;
    }

    // If user presses ctrl-c, try to finish the games that are already running
    let is_shutting_down: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(false)));

    ctrlc::set_handler(move || {
        // If is_shutting_down was already set, exit immediately
        if is_shutting_down.swap(true, atomic::Ordering::SeqCst) {
            process::exit(0)
        } else {
            println!("\nGot Ctrl-C, waiting for running games to finish...");
            println!("Press Ctrl-C again to exit immediately");
        }
    })
    .expect("Error setting Ctrl-C handler");

    run_match(openings, cli_args, is_shutting_down);
    Ok(())
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

fn run_match<const S: usize>(
    openings: Vec<Opening<Position<S>>>,
    cli_args: CliOptions,
    is_shutting_down: &'static AtomicBool,
) {
    let engine_builders: Vec<EngineBuilder> = cli_args
        .engines
        .iter()
        .map(|engine| {
            let mut desired_uci_options = engine.tei_settings.clone();
            desired_uci_options.push((
                "HalfKomi".to_string(),
                cli_args.komi.half_komi().to_string(),
            ));
            EngineBuilder {
                path: engine.path.to_string(),
                args: engine.cli_args.clone(),
                desired_uci_options,
                game_time: engine.time,
                increment: engine.increment,
            }
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
        openings,
        openings_start_index: cli_args.book_start_index,
        num_games: cli_args.games,
        pgn_writer: Mutex::new(pgnout),
        tournament_type: cli_args.tournament_type,
        sprt: cli_args.sprt,
        visualize: cli_args.visualize,
    };

    let tournament = Tournament::new(settings);

    tournament.play(cli_args.concurrency, is_shutting_down, &engine_builders);
}

/// Utility for quickly exiting during initialization, generally due to a user error
/// Engines that have already been started still seem to get killed, at least on Linux
fn exit_with_error(error_message: &str) -> ! {
    eprintln!("{}", error_message);
    error!("{}", error_message);
    process::exit(1)
}
