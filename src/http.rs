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
    uci::UciInfo,
};

#[derive(Clone)]
// Represents the shared state for the HTTP server
// The app may contain state for one or mote concurrent games
struct AppState<const S: usize> {
    game_states: Vec<GameState<S>>,
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
    pub fn new(external_game_states: &[Arc<Mutex<ExternalGameState<Position<S>>>>]) -> Self {
        AppState {
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
                let external_game_state = game_state.external_game_state.lock().unwrap();
                let data: serde_json::Value =
                    serde_json::to_value(GameStateOutput::from(external_game_state.clone()))
                        .unwrap();
                let mut sse_clients = game_state.sse_clients.lock().unwrap();
                // Remove clients where the send fails. The receiver has been dropped, probably because they disconnected.
                sse_clients.retain(|client| client.sender.unbounded_send(data.clone()).is_ok());
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GameStateOutput {
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
}

impl<const S: usize> From<ExternalGameState<Position<S>>> for GameStateOutput {
    fn from(external_game_state: ExternalGameState<Position<S>>) -> Self {
        GameStateOutput {
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
                .map(|ExternalMove { mv, uci_info }| OutputMove {
                    mv: mv.to_string(),
                    uci_info: uci_info.clone(),
                })
                .collect(),
            current_move_uci_info: external_game_state.current_move_uci_info.clone(),
            white_time_left: external_game_state.white_time_left,
            black_time_left: external_game_state.black_time_left,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputMove {
    #[serde(rename = "move")]
    pub mv: String,
    pub uci_info: UciInfo,
}

pub async fn http_server<const S: usize>(
    external_game_states: &[Arc<Mutex<ExternalGameState<Position<S>>>>],
) {
    let shared_state = AppState::new(external_game_states);
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
    let game_state_output: GameStateOutput = game_state_clone.into();
    Ok(Json(game_state_output))
}
