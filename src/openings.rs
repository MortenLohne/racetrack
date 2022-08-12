use crate::exit_with_error;
use pgn_traits::PgnPosition;
use std::fs;
use std::io;
use std::io::BufRead;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Opening<B: PgnPosition> {
    Start(Vec<B::Move>),
    FromFen(String, Vec<B::Move>),
}

impl<B> Opening<B>
where
    B: PgnPosition,
{
    pub fn start_position(&self) -> B {
        match self {
            Self::Start(_) => B::start_position(),
            Self::FromFen(fen, _) => B::from_fen(fen).unwrap(),
        }
    }
}

impl<B> Default for Opening<B>
where
    B: PgnPosition,
{
    fn default() -> Self {
        Self::Start(vec![])
    }
}

pub fn openings_from_file<B: PgnPosition>(path: &str) -> io::Result<Vec<Opening<B>>> {
    let reader = io::BufReader::new(fs::File::open(path).unwrap_or_else(|err| {
        exit_with_error(&format!("Couldn't open opening book \"{}\": {}", path, err))
    }));
    let mut openings = vec![];
    for line in reader.lines() {
        let line = line?;
        let mut fen = None;
        // Todo investigate other syntax for opening
        let (mut position, str_moves) = if line.contains(";") {
            let mut iter = line.split(";");
            fen = iter.next();
            let str_moves = iter.next().unwrap().split_whitespace();
            let pos = B::from_fen(fen.unwrap()).unwrap_or_else(|err| {
                exit_with_error(&format!(
                    "Opening book contained invalid fen string \"{}\": {}",
                    line, err
                ))
            });
            (pos, str_moves)
        } else {
            (B::start_position(), line.split_whitespace())
        };

        let mut moves = vec![];
        for mv_string in str_moves {
            let mv = position.move_from_san(mv_string).unwrap_or_else(|err| {
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
        if let Some(pos) = fen {
            openings.push(Opening::FromFen(pos.to_string(), moves))
        } else {
            openings.push(Opening::Start(moves));
        }
    }
    Ok(openings)
}
