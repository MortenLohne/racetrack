use std::sync::mpsc::Receiver;
use std::thread::Builder;

use pgn_traits::PgnPosition;

// The value must be the same as in `visualizer.html`.
pub const PORT: u16 = 30564;

pub enum Message<P: PgnPosition> {
    Start {
        white: String,
        black: String,
        root_position: P,
    },
    Ply(P::Move),
}

pub fn run_websocket_server<P>(rx: Receiver<Receiver<Message<P>>>)
where
    P: PgnPosition + Send + 'static,
    P::Move: std::fmt::Display + Send,
{
    // TODO: Maybe move this to `visualizer.rs` for easier to read code?
    Builder::new()
        .name("WebSocket Server".to_string())
        .spawn(move || {
            let server = std::net::TcpListener::bind(format!("127.0.0.1:{PORT}")).unwrap();
            // Every time the visualizer window is opened, we get a new connection.
            for (i, stream) in server.incoming().enumerate() {
                // Get the move receiver for that game.
                let move_rx: std::sync::mpsc::Receiver<Message<P>> =
                    rx.recv().unwrap();
                Builder::new()
                    .name(format!("WebSocket Move Relay #{i}"))
                    .spawn(move || {
                        let mut websocket = tungstenite::accept(stream.unwrap()).unwrap();
                        let Ok(Message::Start {
                            white,
                            black,
                            root_position,
                        }) = move_rx.recv()
                        else {
                            // TODO: Maybe do this with types? Have a one-shot that sends a Start, and with that a new channel that only sends moves?
                            panic!("We should have received a Start message.");
                        };
                        let tps = root_position.to_fen();
                        websocket
                            .send(tungstenite::Message::Text(
                                json::object! {
                                    action: "SET_CURRENT_PTN",
                                    value: format!("[Player1 \"{white}\"]\n[Player2 \"{black}\"]\n[TPS \"{tps}\"]"),
                                }.to_string().into()
                            ))
                            .unwrap();
                        while let Ok(Message::Ply(mv)) = move_rx.recv() {
                            // TODO: Evals
                            websocket.send(tungstenite::Message::Text(
                                json::object! {
                                    action: "INSERT_PLY",
                                    value: mv.to_string(),
                                }.to_string().into()
                            )).unwrap();
                        }
                    })
                    .unwrap();
            }
        })
        .unwrap();
}
