use crate::engine::Engine;
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
use std::time::Instant;
use tiltak::position::Komi;
use tiltak::ptn::{Game, PtnMove};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduledGame<B: PgnPosition> {
    pub round_number: usize,
    pub opening: Opening<B>,
    pub white_engine_id: EngineId,
    pub black_engine_id: EngineId,
    pub size: usize,
}

fn forfeit_win_str(color: Color) -> &'static str {
    match color {
        Color::White => "1-0",
        Color::Black => "0-1",
    }
}

impl<B: PgnPosition + Clone> ScheduledGame<B> {
    pub(crate) fn play_game(
        self,
        worker: &mut Worker,
        position_settings: &B::Settings,
    ) -> io::Result<Game<B>> {
        let mut position =
            B::from_fen_with_settings(&self.opening.root_position.to_fen(), position_settings)
                .unwrap();

        let (mut white, mut black) = worker
            .get_engines(self.white_engine_id, self.black_engine_id)
            .unwrap();

        let mut moves: Vec<PtnMove<B::Move>> = self
            .opening
            .moves
            .iter()
            .map(|mv| PtnMove {
                mv: mv.clone(),
                annotations: vec![],
                comment: String::new(),
            })
            .collect();

        for PtnMove { mv, .. } in moves.iter() {
            position.do_move(mv.clone());
        }

        white.uci_write_line(&format!("teinewgame {}", self.size))?;
        white.uci_write_line("isready")?;
        black.uci_write_line(&format!("teinewgame {}", self.size))?;
        black.uci_write_line("isready")?;

        while white.uci_read_line()?.trim() != "readyok" {}
        while black.uci_read_line()?.trim() != "readyok" {}

        let mut white_time = white.builder().game_time;
        let mut black_time = black.builder().game_time;

        let white_inc = white.builder().increment;
        let black_inc = black.builder().increment;

        let (result, result_description) = loop {
            // TODO: Choose max game length
            if moves.len() > 1000 {
                break (
                    None,
                    format!("Game terminated after reaching {} moves.", moves.len() / 2),
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

            let start_time_for_move = Instant::now();

            let mut position_string = String::new();

            if self.opening.root_position == B::start_position() {
                write!(position_string, "position startpos moves ").unwrap();
            } else {
                let tps = self.opening.root_position.to_fen();
                write!(position_string, "position tps {} moves ", tps).unwrap();
            }

            let mut position_board = self.opening.root_position.clone();
            for PtnMove { mv, .. } in moves.iter() {
                write!(position_string, "{} ", position_board.move_to_lan(mv)).unwrap();
                position_board.do_move(mv.clone());
            }

            let go_string = format!(
                "go wtime {} btime {} winc {} binc {}",
                white_time.as_millis(),
                black_time.as_millis(),
                white_inc.as_millis(),
                black_inc.as_millis(),
            );

            let (move_string, last_uci_info) = match Self::play_move(
                engine_to_move,
                &position_string,
                &go_string,
            ) {
                Ok(mv) => mv,
                Err(err) => {
                    if err.kind() == io::ErrorKind::UnexpectedEof
                        || err.kind() == io::ErrorKind::BrokenPipe
                    {
                        warn!("{} disconnected or crashed during game {}. Game is counted as a loss, engine will be restarted.", engine_to_move.name(), self.round_number);
                        engine_to_move.restart()?;
                        break (
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
            };

            let time_taken = start_time_for_move.elapsed();

            let Ok(mv) = position.move_from_lan(&move_string) else {
                break (
                    Some(forfeit_win_str(!position.side_to_move())),
                    format!("{} sent a malformed move", position.side_to_move()),
                );
            };
            let mut legal_moves = vec![];
            position.generate_moves(&mut legal_moves);
            // Check that the move is legal
            if !legal_moves.contains(&mv) {
                break (
                    Some(forfeit_win_str(!position.side_to_move())),
                    format!("{} made an illegal move", position.side_to_move()),
                );
            }
            position.do_move(mv.clone());

            let score_string = match last_uci_info {
                Some(uci_info) => format!(
                    "{:+.2}/{} {:.2}s",
                    match position.side_to_move() {
                        // Flip sign if last move was black's
                        Color::White => uci_info.cp_score as f64 / -100.0,
                        Color::Black => uci_info.cp_score as f64 / 100.0,
                    },
                    uci_info.depth,
                    time_taken.as_secs_f32(),
                ),
                None => String::new(),
            };
            moves.push(PtnMove {
                mv,
                annotations: vec![],
                comment: score_string,
            });

            match !position.side_to_move() {
                Color::White => {
                    if time_taken <= white_time {
                        white_time -= time_taken;
                        white_time += white_inc;
                    } else {
                        break (Some("0-1"), "Black wins on time".to_string());
                    }
                }
                Color::Black => {
                    if time_taken <= black_time {
                        black_time -= time_taken;
                        black_time += black_inc;
                    } else {
                        break (Some("1-0"), "White wins on time".to_string());
                    }
                }
            }
        };

        let date = Local::now();

        let mut tags = vec![
            ("Site".to_string(), "Racetrack".to_string()),
            ("Player1".to_string(), white.name().to_string()),
            ("Player2".to_string(), black.name().to_string()),
            ("Round".to_string(), (self.round_number + 1).to_string()),
            ("Size".to_string(), self.size.to_string()),
            (
                "Date".to_string(),
                format!("{}.{:0>2}.{:0>2}", date.year(), date.month(), date.day()),
            ),
            (
                "Clock".to_string(),
                if white.builder().game_time == black.builder().game_time && white_inc == black_inc
                {
                    format!(
                        "{}:{} +{:.1}",
                        white.builder().game_time.as_secs() / 60,
                        white.builder().game_time.as_secs() % 60,
                        white_inc.as_secs_f32()
                    )
                } else {
                    format!(
                        "{}:{} +{:.1} vs {}:{} +{:.1}",
                        white.builder().game_time.as_secs() / 60,
                        white.builder().game_time.as_secs() % 60,
                        white_inc.as_secs_f32(),
                        black.builder().game_time.as_secs() / 60,
                        black.builder().game_time.as_secs() % 60,
                        black_inc.as_secs_f32()
                    )
                },
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
            start_position: self.opening.root_position,
            moves,
            game_result_str: result,
            tags,
        };
        Ok(game)
    }

    fn play_move(
        engine_to_move: &mut Engine,
        position_string: &str,
        go_string: &str,
    ) -> io::Result<(String, Option<UciInfo<B>>)> {
        engine_to_move.uci_write_line(position_string)?;

        engine_to_move.uci_write_line(go_string)?;

        let mut last_uci_info: Option<UciInfo<B>> = None;

        loop {
            let input = engine_to_move.uci_read_line()?;

            if input.starts_with("info") {
                match parse_info_string(&input) {
                    Ok(uci_info) => last_uci_info = Some(uci_info),
                    Err(err) => warn!("Error in uci string \"{}\", ignoring. {}", input, err),
                }
            }
            if input.starts_with("bestmove") {
                return Ok((
                    input
                        .split_whitespace()
                        .nth(1)
                        .unwrap_or_default()
                        .to_string(),
                    last_uci_info,
                ));
            }
        }
    }
}
