use log::warn;

use crate::engine::Engine;
use crate::pgn_writer::Game;
use crate::r#match::TournamentSettings;
use crate::uci::parser::parse_info_string;
use crate::uci::UciInfo;
use board_game_traits::board::Color;
use pgn_traits::pgn::PgnBoard;
use std::fmt::Write as WriteFmt;
use std::io::Result;
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

    for _ in 0..200 {
        // TODO: Choose max game length
        if board.game_result().is_some() {
            break;
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
            "go movetime {}",
            settings.time_per_move.as_millis()
        ))?;

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
                board.do_move(mv.clone());

                let score_string = match last_uci_info {
                    Some(uci_info) => format!(
                        "{}{:.2}/{}",
                        if uci_info.cp_score > 0 { "+" } else { "" },
                        uci_info.cp_score as f64 / 100.0,
                        uci_info.depth
                    ),
                    None => String::new(),
                };
                moves.push((mv, score_string));
                break;
            }
        }
    }

    let game = Game {
        start_board: B::start_board(),
        moves,
        game_result: board.game_result(),
        tags: vec![
            ("White".to_string(), white.name().to_string()),
            ("Black".to_string(), black.name().to_string()),
            ("Round".to_string(), round.to_string()),
        ],
    };
    Ok(game)
}
