use std::sync::mpsc::Receiver;
use std::thread::Builder;

use pgn_traits::PgnPosition;
use tiltak::position::Position;

// The value must be the same as in `visualizer.html`.
const PORT: u16 = 30564;

pub enum Message<P: PgnPosition> {
    Start {
        white: String,
        black: String,
        root_position: P,
    },
    Ply {
        mv: P::Move,
        eval: Option<i64>,
    },
}

// HACK: Workaround to get access to komi.
pub trait Visualize: PgnPosition {
    fn run_websocket_server(rx: Receiver<Receiver<Message<Self>>>);
}

impl<const S: usize> Visualize for Position<S> {
    fn run_websocket_server(rx: Receiver<Receiver<Message<Position<S>>>>) {
        Builder::new()
        .name("WebSocket Server".to_string())
        .spawn(move || {
            let server = std::net::TcpListener::bind(format!("127.0.0.1:{PORT}"))
                .expect("The port should be available for creating a TCP listener.");
            // Every time the visualizer window is opened, we get a new connection.
            for (i, stream) in server.incoming().enumerate() {
                // Get the move receiver for that game.
                let move_rx = rx.recv().expect("The game thread should send the move receiver soon after opening the window.");
                Builder::new()
                    .name(format!("WebSocket Move Relay #{i}"))
                    .spawn(move || -> Result<(), tungstenite::Error> {
                        let mut websocket = tungstenite::accept(stream.unwrap())
                            .expect("The incoming connection should be using `ws` instead of `wss`.");

                        // Initialize the game from the first message.
                        let (white, black, tps, komi) = match move_rx.recv() {
                            Ok(Message::Start {
                            white,
                            black,
                            root_position,
                        }) => (white, black, root_position.to_fen(), root_position.komi()),
                            Ok(_) => panic!("The first message sent should be a Start message."),
                            Err(_) => panic!("The game thread should still be alive.")
                        };
                        // TODO: Get Komi somehow
                        websocket
                            .send(tungstenite::Message::Text(
                                json::object! {
                                    action: "SET_CURRENT_PTN",
                                    value: format!("[Player1 \"{white}\"][Player2 \"{black}\"][TPS \"{tps}\"][Komi \"{komi}\"]"),
                                }.to_string().into()
                            ))?;

                        // Forward moves from the game to the browser.
                        while let Ok(Message::Ply{ mv, eval }) = move_rx.recv() {
                            websocket.send(tungstenite::Message::Text(
                                json::object! {
                                    action: "INSERT_PLY",
                                    value: mv.to_string(),
                                }.to_string().into()
                            ))?;
                            if let Some(cp) = eval {
                                websocket.send(tungstenite::Message::Text(
                                    json::object! {
                                        action: "SET_EVAL",
                                        value: cp.clamp(-100, 100),
                                    }.to_string().into()
                                ))?
                            }
                        }

                        Ok(())
                    })
                    .unwrap();
            }
        })
        .unwrap();
    }
}
