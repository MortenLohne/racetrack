use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};

use board_game_traits::Position as PositionTrait;
use pgn_traits::PgnPosition;
use tiltak::position::Position;

use crate::openings::{openings_from_file, BookFormat, Opening, OpeningBook};

/// Writes `contents` to a uniquely-named temp file and returns its path. The
/// file is left on disk for the duration of the test process.
fn temp_book(contents: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut path = std::env::temp_dir();
    path.push(format!(
        "racetrack_book_{}_{}.txt",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(contents.as_bytes()).unwrap();
    path.to_str().unwrap().to_string()
}

fn opening_from_move_list(line: &str) -> Opening<Position<5>> {
    let mut position = Position::<5>::start_position();
    let mut moves = vec![];
    for san in line.split_whitespace() {
        let mv = position.move_from_san(san).unwrap();
        position.do_move(mv);
        moves.push(mv);
    }
    Opening {
        root_position: Position::start_position(),
        moves,
    }
}

#[test]
fn lazy_move_list_book_is_indexed_not_loaded() {
    let book: OpeningBook<Position<5>> =
        openings_from_file("5s_4ply_balanced_openings.txt", BookFormat::MoveList).unwrap();

    // Line-based books are kept on disk, only offsets are held in memory.
    assert!(matches!(book, OpeningBook::Lazy(_)));
    assert_eq!(book.len(), 1000);

    // Every opening in this book is four plies deep.
    for i in [0, 1, 499, 998, 999] {
        assert_eq!(book.get(i).moves.len(), 4);
    }
}

#[test]
fn lazy_get_matches_eager_parse_and_handles_blank_lines() {
    // A move-list book treats a blank line as the empty (start) opening, and a
    // trailing space on a line must not confuse parsing.
    let lines = ["c4 a5 e1 b3 ", "b5 a5", "", "a1"];
    let book: OpeningBook<Position<5>> =
        openings_from_file(&temp_book(&lines.join("\n")), BookFormat::MoveList).unwrap();

    assert_eq!(book.len(), 4);

    // Access out of order to exercise seeking back and forth in the file.
    for &i in &[2usize, 0, 3, 1] {
        assert_eq!(book.get(i), opening_from_move_list(lines[i]));
    }
    assert!(book.get(2).moves.is_empty());
}

#[test]
fn shuffle_permutes_offsets_and_preserves_the_multiset() {
    let mut book: OpeningBook<Position<5>> =
        openings_from_file("5s_4ply_balanced_openings.txt", BookFormat::MoveList).unwrap();

    let before: Vec<_> = (0..book.len()).map(|i| book.get(i)).collect();
    book.shuffle(&mut rand::thread_rng());
    let after: Vec<_> = (0..book.len()).map(|i| book.get(i)).collect();

    assert_eq!(before.len(), after.len());
    // Order changed (vanishingly unlikely to fail for 1000 distinct openings)...
    assert_ne!(before, after);
    // ...but every opening is still present exactly as before.
    for opening in &before {
        assert_eq!(
            before.iter().filter(|o| *o == opening).count(),
            after.iter().filter(|o| *o == opening).count(),
        );
    }
}
