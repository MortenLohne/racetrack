use crate::exit_with_error;
use pgn_traits::PgnPosition;
use rand::seq::SliceRandom;
use std::fmt;
use std::fs;
use std::io;
use std::io::{BufRead, Seek};
use std::iter::FromIterator;
use std::marker::PhantomData;
use std::sync::Mutex;
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

/// An opening book that can be indexed and shuffled.
///
/// `Fen` and `MoveList` books are kept on disk and parsed on demand, so only a
/// list of byte offsets is held in memory. Shuffling permutes those offsets
/// rather than the (much larger) parsed openings, so an arbitrarily large book
/// can be loaded and shuffled without reading the whole file into memory.
///
/// `Pgn` books are loaded eagerly, since the PTN parser needs the whole file at
/// once. This format is rarely used for opening books.
pub enum OpeningBook<B: PgnPosition> {
    InMemory(Vec<Opening<B>>),
    Lazy(LazyBook<B>),
}

impl<B: PgnPosition> OpeningBook<B> {
    pub fn len(&self) -> usize {
        match self {
            OpeningBook::InMemory(openings) => openings.len(),
            OpeningBook::Lazy(book) => book.offsets.len(),
        }
    }

    #[allow(dead_code)] // Paired with `len` to satisfy clippy::len_without_is_empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn shuffle<R: rand::Rng + ?Sized>(&mut self, rng: &mut R) {
        match self {
            OpeningBook::InMemory(openings) => openings.shuffle(rng),
            OpeningBook::Lazy(book) => book.offsets.shuffle(rng),
        }
    }
}

impl<B: PgnPosition + Clone> OpeningBook<B> {
    pub fn get(&self, index: usize) -> Opening<B> {
        match self {
            OpeningBook::InMemory(openings) => openings[index].clone(),
            OpeningBook::Lazy(book) => book.get(index),
        }
    }
}

impl<B: PgnPosition> FromIterator<Opening<B>> for OpeningBook<B> {
    fn from_iter<I: IntoIterator<Item = Opening<B>>>(iter: I) -> Self {
        OpeningBook::InMemory(iter.into_iter().collect())
    }
}

/// A line-based opening book ([`BookFormat::Fen`] or [`BookFormat::MoveList`])
/// that is parsed lazily from disk.
pub struct LazyBook<B: PgnPosition> {
    path: String,
    format: BookFormat,
    /// Byte offset of the start of each opening's line. Permuted by `shuffle`.
    offsets: Vec<u64>,
    reader: Mutex<io::BufReader<fs::File>>,
    _phantom: PhantomData<fn() -> B>,
}

impl<B: PgnPosition + Clone> LazyBook<B> {
    fn get(&self, index: usize) -> Opening<B> {
        let offset = self.offsets[index];
        let mut reader = self.reader.lock().unwrap();
        reader
            .seek(io::SeekFrom::Start(offset))
            .and_then(|_| {
                let mut line = String::new();
                reader.read_line(&mut line).map(|_| line)
            })
            .map(|line| match self.format {
                BookFormat::MoveList => parse_move_list_opening(&line),
                BookFormat::Fen => parse_fen_opening(line.trim()),
                BookFormat::Pgn => unreachable!("PGN books are never loaded lazily"),
            })
            .unwrap_or_else(|err| {
                exit_with_error(&format!(
                    "Couldn't read opening book \"{}\": {}",
                    self.path, err
                ))
            })
    }
}

fn open_book_file(path: &str) -> fs::File {
    fs::File::open(path).unwrap_or_else(|err| {
        exit_with_error(&format!("Couldn't open opening book \"{}\": {}", path, err))
    })
}

pub fn openings_from_file<B: PgnPosition>(
    path: &str,
    format: BookFormat,
) -> io::Result<OpeningBook<B>> {
    match format {
        // PGN books need the whole file to parse, so they are loaded eagerly.
        BookFormat::Pgn => {
            let reader = io::BufReader::new(open_book_file(path));
            openings_from_ptn(reader).map(OpeningBook::InMemory)
        }
        // Line-based books are indexed by byte offset and parsed on demand.
        BookFormat::Fen | BookFormat::MoveList => {
            let offsets = index_offsets(path, format)?;
            let reader = Mutex::new(io::BufReader::new(open_book_file(path)));
            Ok(OpeningBook::Lazy(LazyBook {
                path: path.to_string(),
                format,
                offsets,
                reader,
                _phantom: PhantomData,
            }))
        }
    }
}

/// Scan a line-based book once, recording the byte offset where each opening
/// begins. Only the offsets are kept in memory, not the parsed openings.
fn index_offsets(path: &str, format: BookFormat) -> io::Result<Vec<u64>> {
    let mut reader = io::BufReader::new(open_book_file(path));
    let mut offsets = vec![];
    let mut offset: u64 = 0;
    let mut line = String::new();
    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            break;
        }
        // FEN books skip blank lines; move-list books treat them as the empty
        // (start) opening, matching the eager parsers.
        let skip = matches!(format, BookFormat::Fen) && line.trim().is_empty();
        if !skip {
            offsets.push(offset);
        }
        offset += bytes_read as u64;
    }
    Ok(offsets)
}

fn parse_move_list_opening<B: PgnPosition>(line: &str) -> Opening<B> {
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
    Opening {
        root_position: B::start_position(),
        moves,
    }
}

fn parse_fen_opening<B: PgnPosition>(line: &str) -> Opening<B> {
    let position = B::from_fen(line).unwrap_or_else(|err| {
        exit_with_error(&format!("Failed to parse opening \"{}\": {}", line, err))
    });
    Opening {
        root_position: position,
        moves: vec![],
    }
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
