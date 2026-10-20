use std::collections::BTreeMap;

use pgn_traits::PgnPosition;
#[cfg(feature = "http")]
use serde::Serialize;

use crate::game::ExternalGameState;

/// In-memory record of the finished games of the current match.
pub struct MatchHistory<B: PgnPosition> {
    engines: Vec<String>,
    total_rounds: Option<usize>,
    games: BTreeMap<usize, FinishedGame<B>>,
}

struct FinishedGame<B: PgnPosition> {
    white_engine: usize,
    black_engine: usize,
    result: String,
    state: ExternalGameState<B>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "http", derive(Serialize))]
#[cfg_attr(feature = "http", serde(rename_all = "camelCase"))]
pub struct GameResultSummary {
    pub round_number: usize,
    pub white_engine: usize,
    pub black_engine: usize,
    pub result: String,
    pub plies: usize,
}

impl<B: PgnPosition> MatchHistory<B> {
    pub fn new(total_rounds: Option<usize>) -> Self {
        Self {
            engines: Vec::new(),
            total_rounds,
            games: BTreeMap::new(),
        }
    }

    pub fn set_engines(&mut self, names: Vec<String>) {
        self.engines = names;
    }

    pub fn engines(&self) -> &[String] {
        &self.engines
    }

    pub fn total_rounds(&self) -> Option<usize> {
        self.total_rounds
    }

    /// Records a finished game, keyed by its (1-based) round number.
    pub fn record(
        &mut self,
        white_engine: usize,
        black_engine: usize,
        result: Option<&str>,
        mut state: ExternalGameState<B>,
    ) {
        state.current_move_uci_info = None;
        self.games.insert(
            state.round_number,
            FinishedGame {
                white_engine,
                black_engine,
                result: result.unwrap_or("*").to_string(),
                state,
            },
        );
    }

    /// Summaries of all finished games, in ascending round order.
    pub fn results(&self) -> Vec<GameResultSummary> {
        self.games
            .iter()
            .map(|(&round_number, game)| GameResultSummary {
                round_number,
                white_engine: game.white_engine,
                black_engine: game.black_engine,
                result: game.result.clone(),
                plies: game.state.opening.moves.len() + game.state.moves.len(),
            })
            .collect()
    }

    pub fn game(&self, round_number: usize) -> Option<&ExternalGameState<B>> {
        self.games.get(&round_number).map(|game| &game.state)
    }
}
