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
