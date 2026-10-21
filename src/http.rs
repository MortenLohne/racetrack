use std::{
    convert::Infallible,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use futures::{channel::mpsc::UnboundedSender, Stream};
use pgn_traits::PgnPosition;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{sse, Sse},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tiltak::position::Position;
use tokio_stream::StreamExt as _;

use crate::{
    game::{ExternalGameState, ExternalMove},
    history::{GameResultSummary, MatchHistory},
    uci::UciInfo,
};

#[derive(Clone)]
// Represents the shared state for the HTTP server
// The app may contain state for one or mote concurrent games
struct AppState<const S: usize> {
    game_states: Vec<GameState<S>>,
    history: Arc<Mutex<MatchHistory<Position<S>>>>,
}

#[derive(Clone)]
struct GameState<const S: usize> {
    pub external_game_state: Arc<Mutex<ExternalGameState<Position<S>>>>,
    pub sse_clients: Arc<Mutex<Vec<SseClient>>>,
}

#[derive(Clone)]
struct SseClient {
    pub sender: UnboundedSender<serde_json::Value>,
}

impl<const S: usize> AppState<S> {
    pub fn new(
        external_game_states: &[Arc<Mutex<ExternalGameState<Position<S>>>>],
        history: &Arc<Mutex<MatchHistory<Position<S>>>>,
    ) -> Self {
        AppState {
            history: history.clone(),
            game_states: external_game_states
                .iter()
                .map(|external_game_state| GameState {
                    external_game_state: external_game_state.clone(),
                    sse_clients: Arc::new(Mutex::new(Vec::new())),
                })
                .collect::<Vec<_>>(),
        }
    }

    fn broadcast_updates_loop(&self) {
        let interval = Duration::from_millis(200);
        loop {
            thread::sleep(interval);
            for game_state in &self.game_states {
                let external_game_state = game_state.external_game_state.lock().unwrap().clone();
                let output = {
                    let history = self.history.lock().unwrap();
                    GameStateOutput::new(external_game_state, &history)
                };
                let data: serde_json::Value = serde_json::to_value(output).unwrap();
                let mut sse_clients = game_state.sse_clients.lock().unwrap();
                // Remove clients where the send fails. The receiver has been dropped, probably because they disconnected.
                sse_clients.retain(|client| client.sender.unbounded_send(data.clone()).is_ok());
            }
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameStateOutput {
    white_player: String,
    black_player: String,
    size: usize,
    half_komi: i8,
    opening_tps: String,
    opening_moves: Vec<String>,
    round_number: usize,
    pub moves: Vec<OutputMove>,
    pub current_move_uci_info: Option<UciInfo>,
    pub white_time_left: Duration,
    pub black_time_left: Duration,
    white_time: Duration,
    white_increment: Duration,
    black_time: Duration,
    black_increment: Duration,
    engines: Vec<String>,
    results: Vec<GameResultSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_rounds: Option<usize>,
    /// Time since the engine to move was sent `go`; only while a move is in
    /// progress. The side to move's clock is `*TimeLeft` minus this.
    #[serde(skip_serializing_if = "Option::is_none")]
    current_move_elapsed: Option<Duration>,
}

impl GameStateOutput {
    pub(crate) fn new<const S: usize>(
        external_game_state: ExternalGameState<Position<S>>,
        history: &MatchHistory<Position<S>>,
    ) -> Self {
        GameStateOutput {
            engines: history.engines().to_vec(),
            results: history.results(),
            total_rounds: history.total_rounds(),
            white_player: external_game_state.white_player,
            black_player: external_game_state.black_player,
            size: S,
            half_komi: external_game_state.opening.root_position.komi().half_komi(),
            opening_tps: external_game_state.opening.root_position.to_fen(),
            opening_moves: external_game_state
                .opening
                .moves
                .iter()
                .map(|m| m.to_string())
                .collect(),
            round_number: external_game_state.round_number,
            moves: external_game_state
                .moves
                .iter()
                .map(
                    |ExternalMove {
                         mv,
                         uci_info,
                         white_time_left,
                         black_time_left,
                     }| OutputMove {
                        mv: mv.to_string(),
                        uci_info: uci_info.clone(),
                        white_time_left: *white_time_left,
                        black_time_left: *black_time_left,
                    },
                )
                .collect(),
            current_move_uci_info: external_game_state.current_move_uci_info.clone(),
            white_time_left: external_game_state.white_time_left,
            black_time_left: external_game_state.black_time_left,
            white_time: external_game_state.white_time,
            white_increment: external_game_state.white_increment,
            black_time: external_game_state.black_time,
            black_increment: external_game_state.black_increment,
            current_move_elapsed: external_game_state
                .move_started
                .map(|started| started.elapsed()),
        }
    }
}

/// The full record of a finished game, or `None` if that round hasn't finished.
pub(crate) fn finished_game_output<const S: usize>(
    history: &MatchHistory<Position<S>>,
    round: usize,
) -> Option<GameStateOutput> {
    let state = history.game(round)?.clone();
    Some(GameStateOutput::new(state, history))
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputMove {
    #[serde(rename = "move")]
    pub mv: String,
    pub uci_info: UciInfo,
    pub white_time_left: Duration,
    pub black_time_left: Duration,
}

pub async fn http_server<const S: usize>(
    external_game_states: &[Arc<Mutex<ExternalGameState<Position<S>>>>],
    history: &Arc<Mutex<MatchHistory<Position<S>>>>,
) {
    let shared_state = AppState::new(external_game_states, history);
    let shared_state_clone = shared_state.clone();

    thread::spawn(move || {
        shared_state_clone.broadcast_updates_loop();
    });

    println!("Starting HTTP server...");

    let app = Router::new()
        .route("/{id}", get(get_game_state))
        .layer(tower_http::cors::CorsLayer::permissive())
        .route("/{id}/sse", get(sse_handler))
        .layer(tower_http::cors::CorsLayer::permissive())
        .route("/games/{round}", get(get_finished_game))
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(shared_state);
    println!("HTTP app created");

    // run our app with hyper, listening globally on port 23456
    let listener = tokio::net::TcpListener::bind("0.0.0.0:23456")
        .await
        .unwrap();
    println!("HTTP server running on http://0.0.0.0:23456");
    axum::serve(listener, app).await.unwrap();
}

async fn sse_handler<const S: usize>(
    State(state): State<AppState<S>>,
    Path(id): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<sse::Event, Infallible>>>, StatusCode> {
    let id = id.parse::<usize>().map_err(|_| StatusCode::BAD_REQUEST)?;
    let (sender, receiver) = futures::channel::mpsc::unbounded();

    let client = SseClient { sender };

    state
        .game_states
        .get(id)
        .ok_or(StatusCode::NOT_FOUND)?
        .sse_clients
        .lock()
        .unwrap()
        .push(client.clone());

    let mut last_state_sent = serde_json::Value::Null;

    let stream = receiver.filter_map(move |data| {
        let json_patch = json_patch::diff(&last_state_sent, &data);
        if json_patch.is_empty() {
            return None; // Skip sending if no changes
        }
        let event: sse::Event = sse::Event::default().data(json_patch.to_string());
        last_state_sent = data;
        Some(Ok(event))
    });

    Ok(Sse::new(stream).keep_alive(sse::KeepAlive::default()))
}

async fn get_game_state<const S: usize>(
    State(state): State<AppState<S>>,
    Path(id): Path<String>,
) -> Result<Json<GameStateOutput>, StatusCode> {
    let id = id.parse::<usize>().map_err(|_| StatusCode::BAD_REQUEST)?;
    let game_state_clone: ExternalGameState<_> = state
        .game_states
        .get(id)
        .ok_or(StatusCode::NOT_FOUND)?
        .external_game_state
        .lock()
        .unwrap()
        .clone();
    let game_state_output = {
        let history = state.history.lock().unwrap();
        GameStateOutput::new(game_state_clone, &history)
    };
    Ok(Json(game_state_output))
}

async fn get_finished_game<const S: usize>(
    State(state): State<AppState<S>>,
    Path(round): Path<String>,
) -> Result<Json<GameStateOutput>, StatusCode> {
    let round = round
        .parse::<usize>()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let history = state.history.lock().unwrap();
    finished_game_output(&history, round)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}
