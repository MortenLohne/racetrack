use crate::engine::{Engine, EngineBuilder};
use crate::pgn_writer::Game;
use crate::uci::parser::parse_info_string;
use crate::uci::UciInfo;
use board_game_traits::board::{Color, GameResult};
use chrono::{Datelike, Local};
use log::{error, warn};
use pgn_traits::pgn::PgnBoard;
use std::fmt::Write;
use std::io;
use std::io::Result;
use std::sync::{Arc, Mutex};
use std::thread::{Builder, Thread};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq)]
struct EngineId(usize);

#[derive(Clone, Debug, PartialEq, Eq)]
struct ScheduledGame<B: PgnBoard> {
    round_number: usize,
    opening: Vec<B::Move>,
    white_engine_id: EngineId,
    black_engine_id: EngineId,
    time: Duration,
    increment: Duration,
    size: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GamesSchedule<B: PgnBoard> {
    scheduled_games: Vec<ScheduledGame<B>>,
    next_game_id: usize,
}

struct Tournament<B: PgnBoard> {
    games_schedule: Mutex<GamesSchedule<B>>,
    finished_games: Mutex<Vec<Option<Game<B>>>>,
    total_games: usize,
}

impl<B: PgnBoard + Clone + Send + 'static> Tournament<B>
where
    B::Move: Send,
{
    fn new(
        threads: usize,
        engine_builders: &[EngineBuilder],
        games: &[ScheduledGame<B>],
    ) -> Tournament<B> {
        let workers: Vec<Worker> = (0..threads)
            .map(|id| Worker {
                id,
                engines: engine_builders
                    .iter()
                    .map(|builder| builder.init().unwrap())
                    .collect(),
            })
            .collect();
        let tournament = Tournament {
            games_schedule: Mutex::new(GamesSchedule {
                scheduled_games: games.to_vec(),
                next_game_id: 0,
            }),
            finished_games: Mutex::new(vec![None; games.len()]),
            total_games: games.len(),
        };

        let tournament_arc = Arc::new(tournament);

        for mut worker in workers {
            let thread_tournament = tournament_arc.clone();
            Builder::new()
                .name(format!("Worker #{}", worker.id))
                .spawn(move || {
                    while let Some(scheduled_game) = thread_tournament.next_unplayed_game() {
                        let game = worker.play_game(scheduled_game.clone()).unwrap();
                        {
                            let mut finished_games =
                                thread_tournament.finished_games.lock().unwrap();
                            finished_games[scheduled_game.round_number] = Some(game);
                        }
                    }
                })
                .unwrap();
        }
        unimplemented!()
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

struct Worker {
    id: usize,
    engines: Vec<Engine>,
}

impl Worker {
    fn get_engines<'a>(
        &'a mut self,
        white_id: EngineId,
        black_id: EngineId,
    ) -> Option<(&'a mut Engine, &'a mut Engine)> {
        if white_id == black_id {
            None
        } else {
            let white_ptr: *mut Engine = &mut self.engines[white_id.0];
            let black_ptr: *mut Engine = &mut self.engines[black_id.0];
            unsafe { Some((&mut *white_ptr, &mut *black_ptr)) }
        }
    }

    fn play_game<B: PgnBoard>(&mut self, game: ScheduledGame<B>) -> Result<Game<B>> {
        let mut board = B::start_board();
        let (mut white, mut black) = self
            .get_engines(game.white_engine_id, game.black_engine_id)
            .unwrap();

        let mut moves: Vec<(B::Move, String)> = game
            .opening
            .iter()
            .map(|mv| (mv.clone(), String::new()))
            .collect();
        for (mv, _comment) in moves.iter() {
            board.do_move(mv.clone());
        }
        white.uci_write_line(&format!("teinewgame {}", game.size))?;
        white.uci_write_line("isready")?;
        black.uci_write_line(&format!("teinewgame {}", game.size))?;
        black.uci_write_line("isready")?;

        while white.uci_read_line()?.trim() != "readyok" {}
        while black.uci_read_line()?.trim() != "readyok" {}

        let mut white_time = game.time;
        let mut black_time = game.time;

        let (result, result_description) = 'gameloop: loop {
            // TODO: Choose max game length
            if moves.len() > 200 {
                break (
                    None,
                    format!("Game terminated after reaching {} moves.", moves.len()),
                );
            }

            let result = board.game_result();
            if result.is_some() {
                break (result, String::new());
            }
            let engine_to_move = match board.side_to_move() {
                Color::White => &mut white,
                Color::Black => &mut black,
            };

            let mut position_string = String::new();
            write!(position_string, "position startpos moves ").unwrap();
            let mut position_board = B::start_board();
            for (mv, _comment) in moves.iter() {
                write!(position_string, "{} ", position_board.move_to_lan(mv)).unwrap();
                position_board.do_move(mv.clone());
            }

            engine_to_move.uci_write_line(&position_string)?;

            engine_to_move.uci_write_line(&format!(
                "go wtime {} btime {} winc {} binc {}",
                white_time.as_millis(),
                black_time.as_millis(),
                game.increment.as_millis(),
                game.increment.as_millis(),
            ))?;

            let start_time_for_move = Instant::now();

            let mut last_uci_info: Option<UciInfo<B>> = None;

            loop {
                let read_result = engine_to_move.uci_read_line();

                if let Err(err) = read_result {
                    if err.kind() == io::ErrorKind::UnexpectedEof {
                        warn!("{} disconnected or crashed during game {}. Game is counted as a loss, engine will be restarted.", engine_to_move.name(), game.round_number);
                        let loop_result = match board.side_to_move() {
                            Color::White => (
                                Some(GameResult::BlackWin),
                                "White disconnected or crashed".to_string(),
                            ),
                            Color::Black => (
                                Some(GameResult::WhiteWin),
                                "Black disconnected or crashed".to_string(),
                            ),
                        };
                        engine_to_move.restart()?;
                        break 'gameloop loop_result;
                    } else {
                        error!(
                            "Fatal io error from {} during game {}",
                            engine_to_move.name(),
                            game.round_number
                        );
                        return Err(err);
                    }
                }

                let input = read_result.unwrap();

                if input.starts_with("info") {
                    match parse_info_string(&input) {
                        Ok(uci_info) => last_uci_info = Some(uci_info),
                        Err(err) => warn!("Error in uci string \"{}\", ignoring. {}", input, err),
                    }
                }
                if input.starts_with("bestmove") {
                    let move_string = input.split_whitespace().nth(1).unwrap();
                    let mv = board.move_from_lan(move_string).unwrap();
                    let mut legal_moves = vec![];
                    board.generate_moves(&mut legal_moves);
                    // Check that the move is legal
                    if !legal_moves.contains(&mv) {
                        match board.side_to_move() {
                            Color::White => {
                                break 'gameloop (
                                    Some(GameResult::BlackWin),
                                    "White made an illegal move".to_string(),
                                )
                            }
                            Color::Black => {
                                break 'gameloop (
                                    Some(GameResult::WhiteWin),
                                    "Black made an illegal move".to_string(),
                                )
                            }
                        }
                    }
                    board.do_move(mv.clone());

                    let score_string = match last_uci_info {
                        Some(uci_info) => format!(
                            "{}{:.2}/{} {:.2}s",
                            if uci_info.cp_score > 0 { "+" } else { "" },
                            uci_info.cp_score as f64 / 100.0,
                            uci_info.depth,
                            start_time_for_move.elapsed().as_secs_f32(),
                        ),
                        None => String::new(),
                    };
                    moves.push((mv, score_string));
                    break;
                }
            }
            let time_taken = start_time_for_move.elapsed();
            match !board.side_to_move() {
                Color::White => {
                    if time_taken <= white_time {
                        white_time -= time_taken;
                        white_time += game.increment;
                    } else {
                        break (Some(GameResult::BlackWin), "Black wins on time".to_string());
                    }
                }
                Color::Black => {
                    if time_taken <= black_time {
                        black_time -= time_taken;
                        black_time += game.increment;
                    } else {
                        break (Some(GameResult::WhiteWin), "White wins on time".to_string());
                    }
                }
            }
        };

        let date = Local::today();

        let mut tags = vec![
            ("Player1".to_string(), white.name().to_string()),
            ("Player2".to_string(), black.name().to_string()),
            ("Round".to_string(), game.round_number.to_string()),
            ("Size".to_string(), game.size.to_string()),
            (
                "Date".to_string(),
                format!("{}.{:0>2}.{:0>2}", date.year(), date.month(), date.day()),
            ),
        ];
        if !result_description.is_empty() {
            tags.push(("Termination".to_string(), result_description));
        }

        let game = Game {
            start_board: B::start_board(),
            moves,
            game_result: result,
            tags,
        };
        Ok(game)
    }
}
