use crate::engine::EngineBuilder;
use crate::game::play_game;
use board_game_traits::board::Board;
use board_game_traits::board::GameResult::*;
use pgn_traits::pgn::PgnBoard;
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use taik::pgn_writer::Game;

pub fn play_match<B>(
    settings: &TournamentSettings<B>,
    engine1: EngineBuilder,
    engine2: EngineBuilder,
) -> Result<Vec<Game<B>>, io::Error>
where
    B: PgnBoard + Clone + Send,
    <B as Board>::Move: Send + Sync,
{
    let engines: Vec<_> = (0..settings.concurrency)
        .map(|_| {
            let engine1 = engine1.init().unwrap();
            let engine2 = engine2.init().unwrap();
            (Arc::new(Mutex::new(engine1)), Arc::new(Mutex::new(engine2)))
        })
        .collect();

    ThreadPoolBuilder::new()
        .num_threads(settings.concurrency)
        .build_global()
        .unwrap();

    let games: Vec<_> = (0..settings.num_minimatches)
        .into_par_iter()
        .map(|round| {
            let thread_index = rayon::current_thread_index().unwrap();

            let mut white = engines[thread_index].0.try_lock().unwrap();
            let mut black = engines[thread_index].1.try_lock().unwrap();

            vec![
                play_game(
                    &settings,
                    &mut white,
                    &mut black,
                    &settings.openings[round as usize % settings.openings.len()],
                    round,
                ),
                play_game(
                    &settings,
                    &mut black,
                    &mut white,
                    &settings.openings[round as usize % settings.openings.len()],
                    round,
                ),
            ]
        })
        .flatten()
        .map(Result::unwrap)
        .collect();

    println!("Played {} games.", games.len());

    let mut engine1_wins = 0;
    let mut draws = 0;
    let mut engine2_wins = 0;

    let engine1_name = &engines[0].0.lock().unwrap().name().to_string();
    let engine2_name = &engines[0].1.lock().unwrap().name().to_string();

    for game in games.iter() {
        let (_, white_name) = game.tags.iter().find(|(tag, _val)| tag == "White").unwrap();
        match (white_name == engine1_name, game.game_result) {
            (true, Some(WhiteWin)) => engine1_wins += 1,
            (true, Some(BlackWin)) => engine2_wins += 1,
            (false, Some(WhiteWin)) => engine2_wins += 1,
            (false, Some(BlackWin)) => engine1_wins += 1,
            (_, None) | (_, Some(Draw)) => draws += 1,
        }
    }

    println!(
        "{} vs {}: +{}-{}={}",
        engine1_name, engine2_name, engine1_wins, engine2_wins, draws
    );

    Ok(games)
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TournamentSettings<B: Board> {
    pub concurrency: usize,
    pub time_per_move: Duration,
    pub num_minimatches: u64,
    pub openings: Vec<Vec<B::Move>>,
}
