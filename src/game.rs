use crate::openings::Opening;
use crate::tournament::{EngineId, Worker};
use crate::uci::parser::parse_info_string;
use crate::uci::UciInfo;
use board_game_traits::Color;
use chrono::{Datelike, Local};
use log::{error, warn};
use pgn_traits::PgnPosition;
use std::fmt::Write;
use std::io;
use std::time::{Duration, Instant};
use tiltak::position::Komi;
use tiltak::ptn::{Game, PtnMove};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduledGame<B: PgnPosition> {
    pub round_number: usize,
    pub opening: Opening<B>,
    pub white_engine_id: EngineId,
    pub black_engine_id: EngineId,
    pub time: Duration,
    pub increment: Duration,
    pub size: usize,
}

fn forfeit_win_str(color: Color) -> &'static str {
    match color {
        Color::White => "1-0",
        Color::Black => "0-1",
    }
}

impl<B: PgnPosition> ScheduledGame<B> {
    pub(crate) fn play_game(
        self,
        worker: &mut Worker,
        position_settings: &B::Settings,
    ) -> io::Result<Game<B>> {
        let (mut position, mut moves, start_string) = match self.opening {
            Opening::Start(ref moves) => (
                B::start_position_with_settings(position_settings),
                moves
                    .iter()
                    .map(|mv| PtnMove {
                        mv: mv.clone(),
                        annotations: vec![],
                        comment: String::new(),
                    })
                    .collect::<Vec<_>>(),
                "startpos".to_string(),
            ),
            Opening::FromFen(ref fen, ref moves) => (
                B::from_fen_with_settings(&fen, position_settings).unwrap(),
                moves
                    .iter()
                    .map(|mv| PtnMove {
                        mv: mv.clone(),
                        annotations: vec![],
                        comment: String::new(),
                    })
                    .collect(),
                format!("tps {}", fen),
            ), // We should have already checked that the starting fen is valid, so unwrap should be okay
        };
        let (mut white, mut black) = worker
            .get_engines(self.white_engine_id, self.black_engine_id)
            .unwrap();

        for PtnMove { mv, .. } in moves.iter() {
            position.do_move(mv.clone());
        }
        white.uci_write_line(&format!("teinewgame {}", self.size))?;
        white.uci_write_line("isready")?;
        black.uci_write_line(&format!("teinewgame {}", self.size))?;
        black.uci_write_line("isready")?;

        while white.uci_read_line()?.trim() != "readyok" {}
        while black.uci_read_line()?.trim() != "readyok" {}

        let mut white_time = self.time;
        let mut black_time = self.time;

        let (result, result_description) = 'gameloop: loop {
            // TODO: Choose max game length
            if moves.len() > 200 {
                break (
                    None,
                    format!("Game terminated after reaching {} moves.", moves.len()),
                );
            }

            let result = position.pgn_game_result();
            if result.is_some() {
                break (result, String::new());
            }
            let engine_to_move = match position.side_to_move() {
                Color::White => &mut white,
                Color::Black => &mut black,
            };

            let mut position_string = String::new();
            write!(position_string, "position {} moves ", start_string).unwrap();
            let mut position_board = B::start_position();
            for PtnMove { mv, .. } in moves.iter() {
                write!(position_string, "{} ", position_board.move_to_lan(mv)).unwrap();
                position_board.do_move(mv.clone());
            }

            engine_to_move.uci_write_line(&position_string)?;

            engine_to_move.uci_write_line(&format!(
                "go wtime {} btime {} winc {} binc {}",
                white_time.as_millis(),
                black_time.as_millis(),
                self.increment.as_millis(),
                self.increment.as_millis(),
            ))?;

            let start_time_for_move = Instant::now();

            let mut last_uci_info: Option<UciInfo<B>> = None;

            loop {
                let read_result = engine_to_move.uci_read_line();

                if let Err(err) = read_result {
                    if err.kind() == io::ErrorKind::UnexpectedEof {
                        warn!("{} disconnected or crashed during game {}. Game is counted as a loss, engine will be restarted.", engine_to_move.name(), self.round_number);
                        engine_to_move.restart()?;
                        break 'gameloop (
                            Some(forfeit_win_str(!position.side_to_move())),
                            format!("{} disconnected or crashed", position.side_to_move()),
                        );
                    } else {
                        error!(
                            "Fatal io error from {} during game {}",
                            engine_to_move.name(),
                            self.round_number
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
                    if let Some(mv) = input
                        .split_whitespace()
                        .nth(1)
                        .and_then(|s| position.move_from_lan(s).ok())
                    {
                        let mut legal_moves = vec![];
                        position.generate_moves(&mut legal_moves);
                        // Check that the move is legal
                        if !legal_moves.contains(&mv) {
                            break 'gameloop (
                                Some(forfeit_win_str(!position.side_to_move())),
                                format!("{} made an illegal move", position.side_to_move()),
                            );
                        }
                        position.do_move(mv.clone());

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
                        moves.push(PtnMove {
                            mv,
                            annotations: vec![],
                            comment: score_string,
                        });
                        break;
                    } else {
                        break 'gameloop (
                            Some(forfeit_win_str(!position.side_to_move())),
                            format!("{} sent a malformed move", position.side_to_move()),
                        );
                    }
                }
            }
            let time_taken = start_time_for_move.elapsed();
            match !position.side_to_move() {
                Color::White => {
                    if time_taken <= white_time {
                        white_time -= time_taken;
                        white_time += self.increment;
                    } else {
                        break (Some("0-1"), "Black wins on time".to_string());
                    }
                }
                Color::Black => {
                    if time_taken <= black_time {
                        black_time -= time_taken;
                        black_time += self.increment;
                    } else {
                        break (Some("1-0"), "White wins on time".to_string());
                    }
                }
            }
        };

        let date = Local::today();

        let mut tags = vec![
            ("Site".to_string(), "Racetrack".to_string()),
            ("Player1".to_string(), white.name().to_string()),
            ("Player2".to_string(), black.name().to_string()),
            ("Round".to_string(), self.round_number.to_string()),
            ("Size".to_string(), self.size.to_string()),
            (
                "Date".to_string(),
                format!("{}.{:0>2}.{:0>2}", date.year(), date.month(), date.day()),
            ),
            (
                "Clock".to_string(),
                format!(
                    "{}:{} +{:.1}",
                    self.time.as_secs() / 60,
                    self.time.as_secs() % 60,
                    self.increment.as_secs_f32()
                ),
            ),
        ];

        // Write Komi tag for non-zero komi
        if let Some((_, komi_value_string)) = white
            .builder()
            .desired_uci_options
            .iter()
            .find(|(komi_string, _value)| komi_string == "HalfKomi")
        {
            if komi_value_string != "0" {
                tags.push((
                    "Komi".to_string(),
                    komi_value_string
                        .parse()
                        .ok()
                        .and_then(Komi::from_half_komi)
                        .unwrap()
                        .to_string(),
                ))
            }
        }

        if !result_description.is_empty() {
            tags.push(("Termination".to_string(), result_description));
        }

        let game = Game {
            start_position: self.opening.start_position(),
            moves,
            game_result_str: result,
            tags,
        };
        Ok(game)
    }
}
