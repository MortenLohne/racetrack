use crate::exit_with_error;
use pgn_traits::PgnPosition;
use std::fmt;
use std::fs;
use std::io;
use std::io::BufRead;
use tiltak::ptn::ptn_parser;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BookFormat {
    Pgn,
    Fen,
    MoveList,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Opening<B: PgnPosition> {
    pub root_position: B,
    pub moves: Vec<B::Move>,
}

impl<B: PgnPosition + fmt::Debug> fmt::Debug for Opening<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Opening")
            .field("moves", &self.moves)
            .finish()
    }
}

pub fn openings_from_file<B: PgnPosition>(
    path: &str,
    format: BookFormat,
) -> io::Result<Vec<Opening<B>>> {
    let reader = io::BufReader::new(fs::File::open(path).unwrap_or_else(|err| {
        exit_with_error(&format!("Couldn't open opening book \"{}\": {}", path, err))
    }));

    match format {
        BookFormat::Pgn => openings_from_ptn(reader),
        BookFormat::Fen => openings_from_fen(reader),
        BookFormat::MoveList => openings_from_move_list(reader),
    }
}

pub fn openings_from_move_list<B: PgnPosition, R: BufRead>(
    reader: R,
) -> io::Result<Vec<Opening<B>>> {
    let mut openings = vec![];

    for line in reader.lines() {
        let line = line?;

        let mut moves = vec![];
        let mut position = B::start_position();
        for mv_string in line.split_whitespace() {
            let mv = position.move_from_san(mv_string).unwrap_or_else(|err| {
                exit_with_error(&format!(
                    "Opening book contained invalid opening \"{}\": {}",
                    line, err
                ))
            });
            moves.push(mv.clone());
            let mut legal_moves = vec![];
            position.generate_moves(&mut legal_moves);
            if !legal_moves.contains(&mv) {
                exit_with_error(&format!(
                    "Opening book contained illegal opening \"{}\"",
                    line
                ));
            }
            position.do_move(mv.clone());
        }
        openings.push(Opening {
            root_position: B::start_position(),
            moves,
        });
    }
    Ok(openings)
}

pub fn openings_from_fen<B: PgnPosition, R: BufRead>(reader: R) -> io::Result<Vec<Opening<B>>> {
    let mut openings = vec![];

    for line in reader.lines() {
        let line = line?;
        if line.chars().all(|ch| ch.is_whitespace()) {
            continue;
        }
        let position = B::from_fen(&line).unwrap_or_else(|err| {
            exit_with_error(&format!("Failed to parse opening \"{}\": {}", line, err))
        });
        openings.push(Opening {
            root_position: position,
            moves: vec![],
        });
    }
    Ok(openings)
}

pub fn openings_from_ptn<B: PgnPosition, R: BufRead>(mut reader: R) -> io::Result<Vec<Opening<B>>> {
    let mut input = String::new();
    reader.read_to_string(&mut input)?;
    match ptn_parser::parse_ptn(&input) {
        Err(err) => Err(io::Error::new(io::ErrorKind::Other, err)),
        Ok(games) => Ok({
            games
                .into_iter()
                .map(|game| Opening {
                    root_position: game.start_position,
                    moves: game.moves.into_iter().map(|mv| mv.mv).collect(),
                })
                .collect()
        }),
    }
}
