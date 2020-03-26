use std::io::{BufRead, BufReader, Result, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

use board_game_traits::board::{Board as BoardTrait, Color};
use taik::board::{Board, Move};
use pgn_traits::pgn::PgnBoard;

fn main() -> Result<()> {
    let mut engine1 = (EngineBuilder { path: "./main" }).init().unwrap();
    let mut engine2 = (EngineBuilder { path: "./main" }).init().unwrap();

    engine1.initialize()?;
    engine2.initialize()?;

    let settings = Settings {
        concurrency: 1,
        time_per_move: Duration::from_millis(1000),
    };
    play_game(settings, engine1, engine2)
}

fn play_game(settings: Settings, mut white: Engine, mut black: Engine) -> Result<()> {
    let mut board = Board::start_board();
    let mut moves: Vec<Move> = vec![];
    while board.game_result().is_none() {
        let engine_to_move = match board.side_to_move() {
            Color::White => &mut white,
            Color::Black => &mut black,
        };
        write!(engine_to_move.stdin, "position startpos moves ")?;
        for mv in moves.iter() {
            write!(engine_to_move.stdin, "{} ", mv)?;
        }
        writeln!(engine_to_move.stdin)?;
        writeln!(engine_to_move.stdin, "go movetime {}", settings.time_per_move.as_millis())?;
        engine_to_move.stdin.flush()?;

        let mut input = String::new();
        loop {
            engine_to_move.stdout.read_line(&mut input)?;
            if input.starts_with("bestmove") {
                let move_string = input.split_whitespace().nth(1).unwrap();
                let mv = board.move_from_san(move_string).unwrap();
                moves.push(mv.clone());
                board.do_move(mv);
                break;
            }
            input.clear();
        }
    }
    println!("Result: {:?}", board.game_result());
    Ok(())
}

struct EngineBuilder<'a> {
    path: &'a str,
}

impl<'a> EngineBuilder<'a> {
    fn init(&self) -> Result<Engine> {
        Command::new(&self.path)
            .stdout(Stdio::piped())
            .stdin(Stdio::piped())
            .spawn()
            .map(|mut child| {
                let stdout = BufReader::new(child.stdout.take().unwrap());
                let stdin = child.stdin.take().unwrap();
                Engine {
                    child,
                    stdout,
                    stdin,
                }
            })
    }
}

struct Engine {
    child: Child,
    stdout: BufReader<ChildStdout>,
    stdin: ChildStdin,
}

impl Engine {
    fn initialize(&mut self) -> Result<()> {
        writeln!(self.stdin, "uti")?;
        self.stdin.flush()?;

        let mut input = String::new();

        while input.trim() != "utiok" {
            input.clear();
            self.stdout.read_line(&mut input)?;
        }
        input.clear();

        writeln!(self.stdin, "isready")?;
        self.stdin.flush()?;

        while input.trim() != "readyok" {
            input.clear();
            self.stdout.read_line(&mut input)?;
        }

        writeln!(self.stdin, "utinewgame")?;
        self.stdin.flush()?;
        Ok(())
    }
}

struct Settings {
    concurrency: usize,
    time_per_move: Duration,
}
