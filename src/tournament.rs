use crate::engine::{Engine, EngineBuilder};
use crate::exit_with_error;
use crate::game::ScheduledGame;
use crate::pgn_writer::PgnWriter;
use board_game_traits::GameResult::*;
use pgn_traits::PgnPosition;
use std::sync::{Arc, Mutex};
use std::thread::{Builder, JoinHandle};
use std::time::Duration;
use std::{fmt, io};
use tiltak::ptn::Game;

pub struct TournamentSettings<B: PgnPosition> {
    pub size: usize,
    pub concurrency: usize,
    pub time: Duration,
    pub increment: Duration,
    pub num_games: usize,
    pub openings: Vec<Vec<B::Move>>,
    pub pgn_writer: Mutex<PgnWriter<B>>,
}

impl<B: PgnPosition> fmt::Debug for TournamentSettings<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Concurrency: {}", self.concurrency)?;
        writeln!(f, "Total time: {:.1}s", self.time.as_secs_f32())?;
        writeln!(f, "num_games: {}", self.num_games)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineId(usize);

#[derive(Clone, Debug, PartialEq, Eq)]
struct GamesSchedule<B: PgnPosition> {
    scheduled_games: Vec<ScheduledGame<B>>,
    next_game_id: usize,
}

pub struct Tournament<B: PgnPosition> {
    games_schedule: Mutex<GamesSchedule<B>>,
    finished_games: Mutex<Vec<Option<Game<B>>>>,
    pgn_writer: Mutex<PgnWriter<B>>,
}

impl<B> Tournament<B>
where
    B: PgnPosition + Clone + Send + 'static,
    B::Move: Send,
{
    pub fn new_head_to_head(settings: TournamentSettings<B>) -> Self {
        let scheduled_games = (0..settings.num_games)
            .map(|round_number| ScheduledGame {
                round_number,
                opening: settings.openings[(round_number / 2) % settings.openings.len()].clone(),
                white_engine_id: EngineId(round_number % 2),
                black_engine_id: EngineId((round_number + 1) % 2),
                time: settings.time,
                increment: settings.increment,
                size: settings.size,
            })
            .collect();

        Tournament {
            games_schedule: Mutex::new(GamesSchedule {
                scheduled_games,
                next_game_id: 0,
            }),
            finished_games: Mutex::new(vec![None; settings.num_games]),
            pgn_writer: settings.pgn_writer,
        }
    }

    pub fn initialize_with_options_or_exit(builder: &EngineBuilder) -> Engine {
        let mut engine = match builder.init() {
            Ok(engine) => engine,
            Err(err) => match err.kind() {
                io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied => {
                    exit_with_error(&format!(
                        "Failed to start engine \"{}\", caused by: {}",
                        builder.path, err
                    ))
                }
                _ => exit_with_error(&format!(
                    "Error while initializing \"{}\", the engine may have crashed. Caused by: {}",
                    builder.path, err
                )),
            },
        };
        if engine.support_options_from_builder() {
            engine.set_options_from_builder().unwrap();
        } else {
            exit_with_error(&format!(
                "Engine \"{}\" does not support given options",
                builder.path,
            ));
        }
        engine
    }

    pub fn play(self, threads: usize, engine_builders: &[EngineBuilder]) -> Vec<Game<B>> {
        let engine_names: Vec<String> = engine_builders
            .iter()
            .map(|builder| builder.path.clone())
            .collect();

        // Initialize engines
        println!("Initializing engines");

        let workers: Vec<Worker> = (0..threads)
            .map(|id| Worker {
                id,
                engines: engine_builders
                    .iter()
                    .map(|builder| Self::initialize_with_options_or_exit(builder))
                    .collect(),
            })
            .collect();

        let tournament_arc = Arc::new(self);

        println!(
            "Starting {} worker thread(s) to play {} games",
            workers.len(),
            tournament_arc
                .games_schedule
                .lock()
                .unwrap()
                .scheduled_games
                .len()
        );

        let thread_handles: Vec<JoinHandle<()>> = workers
            .into_iter()
            .map(|mut worker| {
                let thread_tournament = tournament_arc.clone();
                let engine_names = engine_names.clone();
                Builder::new()
                    .name(format!("Worker #{}", worker.id))
                    .spawn(move || {
                        while let Some(scheduled_game) = thread_tournament.next_unplayed_game() {
                            let round_number = scheduled_game.round_number;
                            let game = scheduled_game.play_game(&mut worker).unwrap();
                            {
                                let mut finished_games =
                                    thread_tournament.finished_games.lock().unwrap();
                                finished_games[round_number] = Some(game.clone());
                            }
                            {
                                let writer = &thread_tournament.pgn_writer;
                                writer.lock().unwrap().submit_game(round_number, game);
                            }
                            thread_tournament.print_score(&engine_names);
                        }
                        for engine in worker.engines.iter_mut() {
                            let _ = engine.uci_write_line("quit");
                        }
                    })
                    .unwrap()
            })
            .collect();
        for thread_handle in thread_handles {
            thread_handle.join().unwrap();
        }
        tournament_arc.print_score(&engine_names);
        let tournament = tournament_arc.finished_games.lock().unwrap();
        tournament
            .iter()
            .map(|game| game.to_owned().unwrap())
            .collect()
    }

    fn print_score(&self, engine_names: &[String]) {
        let (schedule, finished_games) = loop {
            if let Ok(schedule) = self.games_schedule.try_lock() {
                if let Ok(finished_games) = self.finished_games.try_lock() {
                    break (schedule, finished_games);
                }
            }
        };

        println!(
            "Played {} games.",
            finished_games.iter().filter(|a| a.is_some()).count()
        );

        let mut engine1_wins = 0;
        let mut draws = 0;
        let mut engine2_wins = 0;

        let mut white_wins = 0;
        let mut black_wins = 0;

        let engine1_id = EngineId(0);

        for (scheduled_game, game) in schedule
            .scheduled_games
            .iter()
            .zip(finished_games.iter())
            .filter_map(|(a, b)| b.as_ref().map(|c| (a, c)))
        {
            match (
                scheduled_game.white_engine_id == engine1_id,
                game.game_result,
            ) {
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
            engine_names[0],
            engine_names[1],
            engine1_wins,
            engine2_wins,
            draws,
            white_wins,
            black_wins
        );
    }

    fn next_unplayed_game(&self) -> Option<ScheduledGame<B>> {
        let mut games_schedule = self.games_schedule.lock().unwrap();
        if let Some(scheduled_game) = games_schedule
            .scheduled_games
            .get(games_schedule.next_game_id)
            .cloned()
        {
            games_schedule.next_game_id += 1;
            Some(scheduled_game)
        } else {
            None
        }
    }
}

pub(crate) struct Worker {
    id: usize,
    engines: Vec<Engine>,
}

impl Worker {
    pub(crate) fn get_engines(
        &mut self,
        white_id: EngineId,
        black_id: EngineId,
    ) -> Option<(&mut Engine, &mut Engine)> {
        if white_id == black_id {
            None
        } else {
            let white_ptr: *mut Engine = &mut self.engines[white_id.0];
            let black_ptr: *mut Engine = &mut self.engines[black_id.0];
            unsafe { Some((&mut *white_ptr, &mut *black_ptr)) }
        }
    }
}
