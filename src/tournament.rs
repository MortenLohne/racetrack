use crate::engine::{Engine, EngineBuilder};
use crate::game::ScheduledGame;
use crate::openings::Opening;
use crate::pgn_writer::PgnWriter;
use crate::simulation::MatchScore;
use crate::sprt::{PentanomialResult, SprtParameters};
use crate::{exit_with_error, simulation, visualize};
use board_game_traits::GameResult::*;
use pgn_traits::PgnPosition;
use std::num::NonZeroUsize;
use std::sync::atomic::{self, AtomicBool};
use std::sync::{Arc, Mutex};
use std::thread::{Builder, JoinHandle};
use std::{fmt, io};
use tiltak::ptn::Game;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TournamentType {
    Gauntlet(NonZeroUsize),
    RoundRobin(usize),
    BookTest(usize),
    Sprt,
}

impl TournamentType {
    pub fn num_engines(self) -> usize {
        match self {
            TournamentType::Gauntlet(num_challengers) => num_challengers.get() + 1,
            TournamentType::RoundRobin(num_engines) => num_engines,
            TournamentType::BookTest(num_engines) => num_engines,
            TournamentType::Sprt => 2,
        }
    }

    /// Number of games before every pair of opponents has played a round
    pub fn alignment(self) -> usize {
        match self {
            TournamentType::Gauntlet(num_challengers) => num_challengers.get() * 2,
            TournamentType::RoundRobin(num_engines) => num_engines * (num_engines - 1),
            TournamentType::BookTest(num_engines) => num_engines * num_engines,
            TournamentType::Sprt => 2,
        }
    }
}

pub struct TournamentSettings<B: PgnPosition> {
    pub size: usize,
    pub position_settings: B::Settings,
    pub concurrency: usize,
    pub num_games: usize,
    pub openings: Vec<Opening<B>>,
    pub openings_start_index: usize,
    pub pgn_writer: Mutex<PgnWriter<B>>,
    pub tournament_type: TournamentType,
    pub sprt: Option<SprtParameters>,
    pub visualize: bool,
}

impl<B: PgnPosition> fmt::Debug for TournamentSettings<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Concurrency: {}", self.concurrency)?;
        writeln!(f, "num_games: {}", self.num_games)?;
        Ok(())
    }
}

impl<B: PgnPosition + Clone> TournamentSettings<B> {
    pub fn schedule(&self) -> Vec<ScheduledGame<B>> {
        match self.tournament_type {
            TournamentType::Gauntlet(num_challengers) => (0..self.num_games)
                .map(|round_number| ScheduledGame {
                    round_number,
                    opening: self.openings[(self.openings_start_index
                        + round_number / self.tournament_type.alignment())
                        % self.openings.len()]
                    .clone(),
                    white_engine_id: if (round_number / num_challengers) % 2 == 0 {
                        EngineId(0)
                    } else {
                        EngineId((round_number % num_challengers) + 1)
                    },
                    black_engine_id: if (round_number / num_challengers) % 2 == 1 {
                        EngineId(0)
                    } else {
                        EngineId((round_number % num_challengers) + 1)
                    },
                    size: self.size,
                })
                .collect(),
            TournamentType::BookTest(num_engines) => (0..self.num_games)
                .map(|round_number| ScheduledGame {
                    round_number,
                    opening: self.openings[(self.openings_start_index
                        + round_number / self.tournament_type.alignment())
                        % self.openings.len()]
                    .clone(),
                    white_engine_id: EngineId((round_number / num_engines) % num_engines),
                    black_engine_id: EngineId(round_number % num_engines),
                    size: self.size,
                })
                .collect(),
            TournamentType::RoundRobin(num_engines) => (0..self.num_games)
                .map(|round_number| ScheduledGame {
                    round_number,
                    opening: self.openings[(self.openings_start_index
                        + round_number / (num_engines * (num_engines - 1)))
                        % self.openings.len()]
                    .clone(),
                    white_engine_id: EngineId((round_number / (num_engines - 1)) % num_engines),
                    black_engine_id: EngineId(
                        (round_number
                            + (round_number % (num_engines * (num_engines - 1))) / num_engines
                            + 1)
                            % num_engines,
                    ),
                    size: self.size,
                })
                .collect(),
            TournamentType::Sprt => (0..self.num_games)
                .map(|round_number| ScheduledGame {
                    round_number,
                    opening: self.openings
                        [(self.openings_start_index + round_number / 2) % self.openings.len()]
                    .clone(),
                    white_engine_id: EngineId(round_number % 2),
                    black_engine_id: EngineId((round_number + 1) % 2),
                    size: self.size,
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineId(pub usize);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GamesSchedule<B: PgnPosition> {
    scheduled_games: Vec<ScheduledGame<B>>,
    next_game_id: usize,
}

pub struct Tournament<B: PgnPosition> {
    position_settings: B::Settings,
    pub games_schedule: Mutex<GamesSchedule<B>>,
    finished_games: Mutex<Vec<Option<Game<B>>>>,
    pgn_writer: Mutex<PgnWriter<B>>,
    tournament_type: TournamentType,
    sprt: Option<SprtParameters>,
    visualize: bool,
}

impl<B> Tournament<B>
where
    B: PgnPosition + Clone + Send + 'static + visualize::Visualize,
    B::Move: Send,
    B::Settings: Send + Sync,
{
    pub fn new(settings: TournamentSettings<B>) -> Self {
        let scheduled_games = settings.schedule();

        Tournament {
            position_settings: settings.position_settings,
            games_schedule: Mutex::new(GamesSchedule {
                scheduled_games,
                next_game_id: 0,
            }),
            finished_games: Mutex::new(vec![None; settings.num_games]),
            pgn_writer: settings.pgn_writer,
            tournament_type: settings.tournament_type,
            sprt: settings.sprt,
            visualize: settings.visualize,
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

        if engine.supports_options_from_builder() {
            engine.set_options_from_builder().unwrap();
        } else {
            exit_with_error(&format!(
                "Engine \"{}\" does not support given options",
                engine.name(),
            ));
        }
        engine
    }

    pub fn play(
        self,
        threads: usize,
        is_shutting_down: &'static AtomicBool,
        engine_builders: &[EngineBuilder],
    ) {
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

        // Channel for sending per-game move `Receiver`s to the WebSocket server thread.
        let (tx, rx) = std::sync::mpsc::channel();
        if self.visualize {
            B::run_websocket_server(rx);
        }
        let visualize_tx = Arc::new(tx);
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
                let thread_visualize_tx = visualize_tx.clone();
                let engine_names = engine_names.clone();
                Builder::new()
                    .name(format!("#{}", worker.id)) // Note: The threads' names are used for logging
                    .spawn(move || {
                        while let Some(scheduled_game) = thread_tournament.next_unplayed_game() {
                            if is_shutting_down.load(atomic::Ordering::SeqCst) {
                                break;
                            }
                            let round_number = scheduled_game.round_number;
                            let game = match scheduled_game.play_game(
                                &mut worker,
                                &thread_tournament.position_settings,
                                thread_tournament
                                    .visualize
                                    .then_some(thread_visualize_tx.clone()),
                            ) {
                                Ok(game) => game,
                                // If an error occurs that wasn't handled in play_game(), soft-abort the match
                                // and write a dummy game to the pgn output, so that later games won't be held up
                                Err(err) => {
                                    println!(
                                        "Fatal IO error in worker thread #{}: {}",
                                        worker.id, err
                                    );
                                    log::error!(
                                        "Fatal IO error in worker thread #{}: {}",
                                        worker.id,
                                        err
                                    );
                                    if !is_shutting_down.swap(true, atomic::Ordering::SeqCst) {
                                        println!(
                                            "Match aborted, waiting for running games to finish..."
                                        )
                                    }

                                    Game {
                                        start_position: B::start_position(),
                                        moves: vec![],
                                        game_result_str: None,
                                        tags: vec![],
                                    }
                                }
                            };
                            {
                                let mut finished_games =
                                    thread_tournament.finished_games.lock().unwrap();
                                finished_games[round_number] = Some(game.clone());
                            }
                            {
                                let writer = &thread_tournament.pgn_writer;
                                writer.lock().unwrap().submit_game(round_number, game);
                            }
                            thread_tournament.print_score(&engine_names, is_shutting_down);
                        }
                        for engine in worker.engines.iter_mut() {
                            engine.shutdown().unwrap();
                        }
                    })
                    .unwrap()
            })
            .collect();
        for thread_handle in thread_handles {
            thread_handle.join().unwrap();
        }
        tournament_arc.print_score(&engine_names, is_shutting_down);
    }

    fn print_score(&self, engine_names: &[String], is_shutting_down: &'static AtomicBool) {
        let (schedule, finished_games) = loop {
            if let Ok(schedule) = self.games_schedule.try_lock() {
                if let Ok(finished_games) = self.finished_games.try_lock() {
                    break (schedule, finished_games);
                }
            }
        };

        // Each engine's number of wins vs each other engine
        let mut engine_wins: Vec<Vec<u64>> =
            vec![vec![0; self.tournament_type.num_engines()]; self.tournament_type.num_engines()];
        // Each engine's number of draws vs each other engine
        let mut engine_draws: Vec<Vec<u64>> =
            vec![vec![0; self.tournament_type.num_engines()]; self.tournament_type.num_engines()];
        let mut engine_losses: Vec<Vec<u64>> =
            vec![vec![0; self.tournament_type.num_engines()]; self.tournament_type.num_engines()];

        let mut white_wins = 0;
        let mut black_wins = 0;
        let mut draws = 0;

        for (scheduled_game, game) in schedule
            .scheduled_games
            .iter()
            .zip(finished_games.iter())
            .filter_map(|(a, b)| b.as_ref().map(|c| (a, c)))
        {
            match game.game_result() {
                Some(WhiteWin) => {
                    engine_wins[scheduled_game.white_engine_id.0]
                        [scheduled_game.black_engine_id.0] += 1;
                    engine_losses[scheduled_game.black_engine_id.0]
                        [scheduled_game.white_engine_id.0] += 1;
                    white_wins += 1;
                }
                Some(BlackWin) => {
                    engine_wins[scheduled_game.black_engine_id.0]
                        [scheduled_game.white_engine_id.0] += 1;
                    engine_losses[scheduled_game.white_engine_id.0]
                        [scheduled_game.black_engine_id.0] += 1;
                    black_wins += 1;
                }

                None | Some(Draw) => {
                    engine_draws[scheduled_game.white_engine_id.0]
                        [scheduled_game.black_engine_id.0] += 1;
                    engine_draws[scheduled_game.black_engine_id.0]
                        [scheduled_game.white_engine_id.0] += 1;
                    draws += 1;
                }
            }
        }

        println!(
            "Played {} games. {} white wins, {} black wins, {} draws.",
            finished_games.iter().filter(|a| a.is_some()).count(),
            white_wins,
            black_wins,
            draws
        );

        assert_eq!(
            engine_wins.iter().flatten().sum::<u64>() + draws,
            finished_games.iter().flatten().count() as u64
        );
        assert_eq!(
            engine_losses.iter().flatten().sum::<u64>() + draws,
            finished_games.iter().flatten().count() as u64
        );
        assert_eq!(
            white_wins + black_wins + draws,
            finished_games.iter().flatten().count() as u64
        );

        assert_eq!(draws, engine_draws.iter().flatten().sum::<u64>() / 2);

        match self.tournament_type {
            TournamentType::RoundRobin(2) => {
                print_head_to_head_score(&engine_wins, &engine_draws, engine_names, 0, 1)
            }
            // For gauntlet tournament, prints the challengers' scores vs the champion,
            // instead of the other way around
            TournamentType::Gauntlet(num_challengers) => {
                for engine2_id in 1..=num_challengers.get() {
                    print_head_to_head_score(
                        &engine_wins,
                        &engine_draws,
                        engine_names,
                        engine2_id,
                        0,
                    )
                }
            }
            TournamentType::RoundRobin(num_engines)
            | TournamentType::BookTest(num_engines @ 2..) => {
                println!(
                    "{:16} {:>4} {:>4} {:>4} {:>7}",
                    "Name", "+", "-", "=", "Score"
                );
                for id in 0..num_engines {
                    // The engine's results against every engine except itself:
                    let num_wins = engine_wins[id].iter().sum::<u64>() - engine_wins[id][id];
                    let num_draws: u64 =
                        engine_draws[id].iter().sum::<u64>() - engine_draws[id][id];
                    let num_losses: u64 =
                        engine_losses[id].iter().sum::<u64>() - engine_losses[id][id];
                    let num_games = num_wins + num_draws + num_losses;

                    println!(
                        "{:16} {:4} {:4} {:4} {:>6.1}%",
                        engine_names[id],
                        num_wins,
                        num_losses,
                        num_draws,
                        100.0 * (num_wins as f32 + num_draws as f32 / 2.0) / num_games as f32
                    );
                }
            }
            TournamentType::BookTest(_) => (),
            TournamentType::Sprt => {
                println!("Base engine : {}", engine_names[0]);
                println!("Under test  : {}", engine_names[1]);

                let score = MatchScore {
                    wins: engine_wins[1][0],
                    draws: engine_draws[1][0],
                    losses: engine_losses[1][0],
                };
                let full_simulation = simulation::FullWinstonSimulation::run_simulation(score);
                let lower = full_simulation.result_for_p(0.025);
                let expected = score.score();
                let upper = full_simulation.result_for_p(0.975);
                let lower_elo = simulation::to_elo_string(lower);
                let expected_elo = simulation::to_elo_string(expected);
                let upper_elo = simulation::to_elo_string(upper);
                println!(
                    "Elo         : {} [{}, {}] (95%)",
                    expected_elo, lower_elo, upper_elo
                );
                println!(
                    "WDL         : W: {}, D: {}, L: {}",
                    score.wins, score.draws, score.losses
                );

                let penta = Self::sprt_penta_stats(&finished_games);
                println!(
                    "Penta(0-2)  : {}, {}, {}, {}, {}",
                    penta.ll,
                    penta.dl,
                    penta.dd + penta.wl,
                    penta.wd,
                    penta.ww
                );

                if let Some(sprt) = self.sprt {
                    let (elo0, elo1) = sprt.elo_bounds();
                    let (lower_bound, upper_bound) = sprt.llr_bounds();
                    let llr = sprt.llr(penta);

                    let meet = if llr <= lower_bound {
                        format!("(<= {:.2})", lower_bound)
                    } else if llr >= upper_bound {
                        format!("(>= {:.2})", upper_bound)
                    } else {
                        "".to_string()
                    };
                    println!(
                        "LLR         : {:.2} {:10} [{:.2} {:.2}]",
                        llr, meet, elo0, elo1
                    );

                    if llr <= lower_bound || llr >= upper_bound {
                        is_shutting_down.store(true, atomic::Ordering::SeqCst);
                    }

                    if llr <= lower_bound {
                        println!("SPRT failed");
                    }
                    if llr >= upper_bound {
                        println!("SPRT passed");
                    }
                }
            }
        }
    }

    fn sprt_penta_stats(finished_games: &Vec<Option<Game<B>>>) -> PentanomialResult {
        let mut result = PentanomialResult {
            ww: 0,
            wd: 0,
            wl: 0,
            dd: 0,
            dl: 0,
            ll: 0,
        };
        for game_pair in finished_games
            .chunks(2)
            .filter(|p| p.iter().all(|g| g.is_some()))
        {
            let result1 = game_pair[0].clone().unwrap().game_result().unwrap_or(Draw);
            let result2 = game_pair[1].clone().unwrap().game_result().unwrap_or(Draw);
            match (result1, result2) {
                (WhiteWin, BlackWin) => result.ll += 1,
                (WhiteWin, Draw) => result.dl += 1,
                (Draw, BlackWin) => result.dl += 1,
                (WhiteWin, WhiteWin) => result.wl += 1,
                (BlackWin, BlackWin) => result.wl += 1,
                (Draw, Draw) => result.dd += 1,
                (BlackWin, Draw) => result.wd += 1,
                (Draw, WhiteWin) => result.wd += 1,
                (BlackWin, WhiteWin) => result.ww += 1,
            }
        }
        result
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

fn print_head_to_head_score(
    engine_wins: &[Vec<u64>],
    engine_draws: &[Vec<u64>],
    engine_names: &[String],
    engine1_id: usize,
    engine2_id: usize,
) {
    let score = MatchScore {
        wins: engine_wins[engine1_id][engine2_id],
        draws: engine_draws[engine1_id][engine2_id],
        losses: engine_wins[engine2_id][engine1_id],
    };

    let full_simulation = simulation::FullWinstonSimulation::run_simulation(score);

    let lower = full_simulation.result_for_p(0.025);
    let expected = score.score();
    let upper = full_simulation.result_for_p(0.975);

    let lower_elo = simulation::to_elo_string(lower);
    let expected_elo = simulation::to_elo_string(expected);
    let upper_elo = simulation::to_elo_string(upper);

    println!(
        "{} vs {}: {}, {} elo [{}, {}] (95% confidence).",
        engine_names[engine1_id],
        engine_names[engine2_id],
        score,
        expected_elo,
        lower_elo,
        upper_elo,
    );
}

pub(crate) struct Worker {
    pub id: usize,
    pub engines: Vec<Engine>,
}
