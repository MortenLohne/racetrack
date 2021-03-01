use crate::pgn_writer::Game;
use board_game_traits::board::Board;
use pgn_traits::pgn::PgnBoard;
use std::fmt::Formatter;
use std::sync::Mutex;
use std::time::Duration;
use std::{fmt, io};

pub struct TournamentSettings<B: PgnBoard> {
    pub size: usize,
    pub concurrency: usize,
    pub time: Duration,
    pub increment: Duration,
    pub num_minimatches: usize,
    pub openings: Vec<Vec<B::Move>>,
    pub pgn_writer: Mutex<PgnWriter<B>>,
}

impl<B: PgnBoard> fmt::Debug for TournamentSettings<B> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "Concurrency: {}", self.concurrency)?;
        writeln!(f, "Total time: {:.1}s", self.time.as_secs_f32())?;
        writeln!(f, "num_minimatches: {}", self.num_minimatches)?;
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
