use board_game_traits::Position;
use pgn_traits::PgnPosition;
use std::io;
use std::io::Write;
use tiltak::ptn::Game;

/// A wrapper around a `Write` instance, to ensure that PGNs are written in order
///
pub struct PgnWriter<B: Position> {
    pgn_out: Box<dyn io::Write + Send>,
    pending_games: Vec<(usize, Game<B>)>,
    next_game_number: usize,
}

impl<B: PgnPosition + Clone> PgnWriter<B> {
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
            game.game_to_ptn(&mut self.pgn_out)?;
            self.next_game_number += 1;
        }
        self.pgn_out.flush()?;
        Ok(())
    }
}
