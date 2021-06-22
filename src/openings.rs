use crate::exit_with_error;
use board_game_traits::Position as PositionTrait;
use pgn_traits::PgnPosition;
use std::fs;
use std::io;
use std::io::BufRead;
use tiltak::position::{Move, Position};

pub fn openings_from_file<const S: usize>(path: &str) -> io::Result<Vec<Vec<Move>>> {
    let reader = io::BufReader::new(fs::File::open(path).unwrap_or_else(|err| {
        exit_with_error(&format!("Couldn't open opening book \"{}\": {}", path, err))
    }));
    let mut openings = vec![];

    for line in reader.lines() {
        let line = line?;
        let mut position = <Position<S>>::start_position();
        let mut moves = vec![];
        for mv_string in line.split_whitespace() {
            let mv = position.move_from_san(&mv_string).unwrap_or_else(|err| {
                exit_with_error(&format!(
                    "Opening book contained invalid opening \"{}\": {}",
                    line, err
                ))
            });
            let mut legal_moves = vec![];
            position.generate_moves(&mut legal_moves);
            if !legal_moves.contains(&mv) {
                exit_with_error(&format!(
                    "Opening book contained illegal opening \"{}\"",
                    line
                ));
            }
            position.do_move(mv.clone());
            moves.push(mv);
        }
        openings.push(moves);
    }
    Ok(openings)
}
