use crate::engine::Engine;
use crate::TournamentSettings;
use board_game_traits::board::{Board as BoardTrait, Color};
use pgn_traits::pgn::PgnBoard;
use std::fmt::Write as WriteFmt;
use std::io;
use std::io::Result;
use taik::pgn_writer;
use taik::pgn_writer::Game;

pub fn play_game<'a, B: BoardTrait + PgnBoard + Clone>(
    settings: &TournamentSettings,
    mut white: &'a mut Engine,
    mut black: &'a mut Engine,
    opening: &[B::Move],
    round: u64,
) -> Result<Game<B>>
where
    B::Move: Clone,
{
    let mut board = B::start_board();
    let mut moves: Vec<B::Move> = opening.to_vec();
    for mv in moves.iter() {
        board.do_move(mv.clone());
    }
    white.uci_write_line("utinewgame")?;
    black.uci_write_line("utinewgame")?;
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
        for mv in moves.iter() {
            write!(position_string, "{} ", position_board.move_to_lan(mv)).unwrap();
            position_board.do_move(mv.clone());
        }
        engine_to_move.uci_write_line(&position_string)?;

        engine_to_move.uci_write_line(&format!(
            "go movetime {}",
            settings.time_per_move.as_millis()
        ))?;

        loop {
            let input = engine_to_move.uci_read_line()?;
            if input.starts_with("bestmove") {
                let move_string = input.split_whitespace().nth(1).unwrap();
                let mv = board.move_from_lan(move_string).unwrap();
                moves.push(mv.clone());
                board.do_move(mv);
                break;
            }
        }
    }

    let moves_with_comments: Vec<_> = moves.into_iter().map(|mv| (mv, String::new())).collect();

    let game = Game {
        start_board: B::start_board(),
        moves: moves_with_comments.clone(),
        game_result: board.game_result(),
        tags: vec![
            ("White".to_string(), white.name().to_string()),
            ("Black".to_string(), black.name().to_string()),
            ("Round".to_string(), round.to_string()),
        ],
    };

    pgn_writer::game_to_pgn(
        &mut game.start_board.clone(),
        &moves_with_comments,
        "",
        "",
        "",
        &round.to_string(),
        &white.name(),
        &black.name(),
        board.game_result(),
        &[],
        &mut io::stdout(),
    )?;

    Ok(game)
}
