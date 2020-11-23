use crate::engine::{Engine, EngineBuilder};
use crate::game::play_game;
use crate::pgn_writer::Game;
use board_game_traits::board::Board;
use board_game_traits::board::GameResult::*;
use pgn_traits::pgn::PgnBoard;
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;
use std::fmt::Formatter;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{fmt, io};

pub fn play_match<B>(
    settings: &TournamentSettings<B>,
    engine1: EngineBuilder,
    engine2: EngineBuilder,
) -> Result<Vec<Game<B>>, io::Error>
where
    B: PgnBoard + Clone + Send,
    <B as Board>::Move: Send + Sync,
{
    let engines: Vec<_> = (0..settings.concurrency)
        .map(|_| {
            // TODO: Handle error
            let engine1 = engine1.init().unwrap();
            let engine2 = engine2.init().unwrap();
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
            let thread_index = rayon::current_thread_index().unwrap_or(0);

            let mut white = engines[thread_index].0.try_lock().unwrap();
            let mut black = engines[thread_index].1.try_lock().unwrap();
            let game_number = round * 2;
            println!(
                "Starting game {}, {} vs {}.",
                game_number,
                white.name(),
                black.name()
            );

            let game1 = play_game(
                &settings,
                &mut white,
                &mut black,
                &settings.openings[round as usize % settings.openings.len()],
                game_number,
            )
            .unwrap();

            {
                if let Some(ref writer) = settings.pgn_writer {
                    writer
                        .lock()
                        .unwrap()
                        .submit_game(game_number, game1.clone());
                }
            }

            let game_number = round * 2 + 1;
            println!(
                "Starting game {}, {} vs {}.",
                game_number,
                black.name(),
                white.name()
            );

            let game2 = play_game(
                &settings,
                &mut black,
                &mut white,
                &settings.openings[round as usize % settings.openings.len()],
                game_number,
            )
            .unwrap();

            {
                if let Some(ref writer) = settings.pgn_writer {
                    writer
                        .lock()
                        .unwrap()
                        .submit_game(game_number, game2.clone());
                }
            }

            vec![game1, game2]
        })
        .flatten()
        .collect();

    print_score(&games, &engines);

    Ok(games)
}

pub fn print_score<B: PgnBoard>(
    games: &[Game<B>],
    engines: &[(Arc<Mutex<Engine>>, Arc<Mutex<Engine>>)],
) {
    println!("Played {} games.", games.len());

    let mut engine1_wins = 0;
    let mut draws = 0;
    let mut engine2_wins = 0;

    let mut white_wins = 0;
    let mut black_wins = 0;

    let engine1_name = &engines[0].0.lock().unwrap().name().to_string();
    let engine2_name = &engines[0].1.lock().unwrap().name().to_string();

    for game in games.iter() {
        let (_, white_name) = game.tags.iter().find(|(tag, _val)| tag == "White").unwrap();
        match (white_name == engine1_name, game.game_result) {
            (true, Some(WhiteWin)) => {
                engine1_wins += 1;
                white_wins += 1;
            }
            (true, Some(BlackWin)) => {
                engine2_wins += 1;
                black_wins += 1;
            }
            (false, Some(WhiteWin)) => {
                engine2_wins += 1;
                white_wins += 1;
            }
            (false, Some(BlackWin)) => {
                engine1_wins += 1;
                black_wins += 1;
            }
            (_, None) | (_, Some(Draw)) => draws += 1,
        }
    }

    println!(
        "{} vs {}: +{}-{}={}. {} white wins, {} black wins.",
        engine1_name, engine2_name, engine1_wins, engine2_wins, draws, white_wins, black_wins
    );
}

pub struct TournamentSettings<B: PgnBoard> {
    pub concurrency: usize,
    pub time_per_move: Duration,
    pub num_minimatches: u64,
    pub openings: Vec<Vec<B::Move>>,
    pub pgn_writer: Option<Mutex<PgnWriter<B>>>,
}

impl<B: PgnBoard> fmt::Debug for TournamentSettings<B> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "Concurrency: {}", self.concurrency)?;
        writeln!(f, "Time per move: {:.1}s", self.time_per_move.as_secs_f32())?;
        writeln!(f, "num_minimatches: {}", self.num_minimatches)?;
        Ok(())
    }
}

/// A wrapper around a `Write` instance, to ensure that PGNs are written in order
///
pub struct PgnWriter<B: Board> {
    pgn_out: Box<dyn io::Write + Send>,
    pending_games: Vec<(u64, Game<B>)>,
    next_game_number: u64,
}

impl<B: PgnBoard + Clone> PgnWriter<B> {
    pub fn new<W: io::Write + Send + 'static>(pgn_out: W) -> Self {
        PgnWriter {
            pgn_out: Box::new(pgn_out),
            pending_games: vec![],
            next_game_number: 0,
        }
    }

    pub fn submit_game(&mut self, game_number: u64, game: Game<B>) {
        self.pending_games.push((game_number, game));
        self.pending_games
            .sort_by_key(|(game_number, _)| *game_number);
        self.try_write_games().unwrap();
    }

    fn try_write_games(&mut self) -> io::Result<()> {
        while !self.pending_games.is_empty() && self.pending_games[0].0 == self.next_game_number {
            let game = self.pending_games.remove(0).1;
            game.game_to_pgn(&mut self.pgn_out)?;
            self.next_game_number += 1;
        }
        self.pgn_out.flush()?;
        Ok(())
    }
}
