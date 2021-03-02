use board_game_traits::board::{Board as BoardTrait, Board};
use board_game_traits::board::{Color, GameResult};
use pgn_traits::pgn::PgnBoard;
use std::io;
use std::io::Write;

#[derive(Debug, Clone, PartialEq)]
pub struct Game<B: BoardTrait> {
    pub start_board: B,
    pub moves: Vec<(B::Move, String)>,
    pub game_result: Option<GameResult>,
    pub tags: Vec<(String, String)>,
}

impl<B: PgnBoard + Clone> Game<B> {
    pub fn game_to_pgn<W: Write>(&self, f: &mut W) -> Result<(), io::Error> {
        // Write the 7 required tags first, in the correct order
        // Fill in default value if they are not available
        let required_tag_pairs = [
            ("Event", "?"),
            ("Site", "?"),
            ("Date", "????.??.??"),
            ("Round", "?"),
            ("Player1", "?"),
            ("Player2", "?"),
        ];

        // We must ensure that all required tags are included, and written in the correct order
        let mut tags = self.tags.clone();

        for (required_tag, default_value) in required_tag_pairs.iter() {
            let position = tags
                .iter()
                .position(|(tag, _value)| tag.eq_ignore_ascii_case(required_tag));
            if let Some(position) = position {
                let (_tag, value) = tags.remove(position);
                // Write the tag with correct capitalization
                writeln!(f, "[{} \"{}\"]", required_tag, value)?;
            } else {
                writeln!(f, "[{} \"{}\"]", required_tag, default_value)?;
            }
        }

        writeln!(
            f,
            "[Result \"{}\"]",
            match self.game_result {
                None => "*",
                Some(GameResult::WhiteWin) => "1-0",
                Some(GameResult::BlackWin) => "0-1",
                Some(GameResult::Draw) => "1/2-1/2",
            }
        )?;

        if self.start_board != B::start_board()
            && tags
                .iter()
                .find(|(tag, _)| tag.eq_ignore_ascii_case("FEN"))
                .is_none()
        {
            writeln!(f, "[FEN \"{}\"", self.start_board.to_fen())?;
        }

        // Write any remaining tags
        for (tag, value) in tags.iter() {
            writeln!(f, "[{} \"{}\"]", tag, value)?;
        }

        let mut board = self.start_board.clone();

        for (i, (mv, comment)) in self.moves.iter().enumerate() {
            if i % 12 == 0 {
                writeln!(f)?;
            }
            if i == 0 && board.side_to_move() == Color::Black {
                write!(f, "1... {} {{{}}} ", board.move_to_san(&mv), comment)?;
            } else if board.side_to_move() == Color::White {
                write!(
                    f,
                    "{}. {} {}",
                    (i + 1) / 2 + 1,
                    board.move_to_san(&mv),
                    if comment.is_empty() {
                        "".to_string()
                    } else {
                        "{".to_string() + comment + "} "
                    }
                )?;
            } else {
                write!(
                    f,
                    "{} {}",
                    board.move_to_san(&mv),
                    if comment.is_empty() {
                        "".to_string()
                    } else {
                        "{".to_string() + comment + "} "
                    }
                )?;
            }
            board.do_move(mv.clone());
        }

        write!(
            f,
            "{}",
            match self.game_result {
                None => "*",
                Some(GameResult::WhiteWin) => "1-0",
                Some(GameResult::BlackWin) => "0-1",
                Some(GameResult::Draw) => "1/2-1/2",
            }
        )?;
        writeln!(f)?;
        writeln!(f)?;
        Ok(())
    }
}

/// A wrapper around a `Write` instance, to ensure that PGNs are written in order
///
pub struct PgnWriter<B: Board> {
    pgn_out: Box<dyn io::Write + Send>,
    pending_games: Vec<(usize, Game<B>)>,
    next_game_number: usize,
}

impl<B: PgnBoard + Clone> PgnWriter<B> {
    pub fn new<W: io::Write + Send + 'static>(pgn_out: W) -> Self {
        PgnWriter {
            pgn_out: Box::new(pgn_out),
            pending_games: vec![],
            next_game_number: 0,
        }
    }

    pub fn submit_game(&mut self, game_number: usize, game: Game<B>) {
        self.pending_games.push((game_number, game));
        self.pending_games
            .sort_by_key(|(game_number, _)| *game_number);
        self.try_write_games().unwrap();
    }

    fn try_write_games(&mut self) -> io::Result<()> {
        while !self.pending_games.is_empty() && self.pending_games[0].0 == self.next_game_number {
            let game = self.pending_games.remove(0).1;
            game.game_to_pgn(&mut self.pgn_out)?;
            self.next_game_number += 1;
        }
        self.pgn_out.flush()?;
        Ok(())
    }
}
