use log::warn;

use crate::engine::Engine;
use crate::pgn_writer::Game;
use crate::r#match::TournamentSettings;
use crate::uci::parser::parse_info_string;
use crate::uci::UciInfo;
use board_game_traits::board::{Color, GameResult};
use pgn_traits::pgn::PgnBoard;
use std::fmt::Write as WriteFmt;
use std::io::Result;
use std::time::Instant;
use taik::board::Board;

pub fn play_game<'a, B: PgnBoard + Clone>(
    settings: &TournamentSettings<B>,
    mut white: &'a mut Engine,
    mut black: &'a mut Engine,
    opening: &[B::Move],
    round: u64,
) -> Result<Game<B>>
where
    B::Move: Clone,
{
    let mut board = B::start_board();
    let mut moves: Vec<(B::Move, String)> = opening
        .iter()
        .map(|mv| (mv.clone(), String::new()))
        .collect();
    for (mv, _comment) in moves.iter() {
        board.do_move(mv.clone());
    }
    white.uci_write_line("teinewgame 5")?;
    white.uci_write_line("isready")?;
    black.uci_write_line("teinewgame 5")?;
    black.uci_write_line("isready")?;

    while white.uci_read_line()?.trim() != "readyok" {}
    while black.uci_read_line()?.trim() != "readyok" {}

    let mut white_time = settings.time;
    let mut black_time = settings.time;

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

        /*
        engine_to_move.uci_write_line(&format!(
            "go wtime {} btime {}",
            white_time.as_millis(),
            black_time.as_millis()
        ))?;
        */
        match board.side_to_move() {
            Color::White => {
                engine_to_move
                    .uci_write_line(&format!("go movetime {}", white_time.as_millis() / 10))?;
            }
            Color::Black => {
                engine_to_move
                    .uci_write_line(&format!("go movetime {}", black_time.as_millis() / 10))?;
            }
        }

        let start_time_for_move = Instant::now();

        let mut last_uci_info: Option<UciInfo<Board>> = None;

        loop {
            let input = engine_to_move.uci_read_line()?;
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
                    white_time += settings.increment;
                } else {
                    break (Some(GameResult::BlackWin), "Black wins on time".to_string());
                }
            }
            Color::Black => {
                if time_taken <= black_time {
                    black_time -= time_taken;
                    black_time += settings.increment;
                } else {
                    break (Some(GameResult::WhiteWin), "White wins on time".to_string());
                }
            }
        }
    };

    let mut tags = vec![
        ("White".to_string(), white.name().to_string()),
        ("Black".to_string(), black.name().to_string()),
        ("Round".to_string(), round.to_string()),
        ("Size".to_string(), "5".to_string()),
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
