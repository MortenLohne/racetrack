use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use pgn_traits::PgnPosition;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tiltak::position::Position;

use crate::{
    game::{ExternalGameState, ExternalMove},
    uci::UciInfo,
};

type AppState<const S: usize> = Vec<Arc<Mutex<ExternalGameState<Position<S>>>>>;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GameStateOutput {
    white_player: String,
    black_player: String,
    size: usize,
    half_komi: i8,
    opening_tps: String,
    opening_moves: Vec<String>,
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

pub async fn http_server<const S: usize>(external_game_states: AppState<S>) {
    println!("Starting HTTP server...");
    let shared_state = external_game_states;

    let app = Router::new()
        .route("/{id}", get(get_game_state))
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

async fn get_game_state<const S: usize>(
    State(external_game_states): State<AppState<S>>,
    Path(id): Path<String>,
) -> Result<Json<GameStateOutput>, StatusCode> {
    let id = id.parse::<usize>().map_err(|_| StatusCode::BAD_REQUEST)?;
    let game_state_clone: ExternalGameState<_> = external_game_states
        .get(id)
        .ok_or(StatusCode::NOT_FOUND)?
        .lock()
        .unwrap()
        .clone();
    let game_state_output: GameStateOutput = game_state_clone.into();
    Ok(Json(game_state_output))
}
