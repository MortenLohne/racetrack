use crate::uci::parser::parse_option;
use std::env;
use std::io;
use std::io::Result;
use std::io::{BufRead, BufReader, Write};
use std::path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::string::ToString;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EngineBuilder<'a> {
    pub path: &'a str,
}

impl<'a> EngineBuilder<'a> {
    /// Initialize the engine, including starting the binary and reading the engine's available uci commands.
    pub fn init(&self) -> Result<Engine> {
        // TODO: Error for not permission to current directory
        let mut absolute_path = env::current_dir()?;

        absolute_path.push(self.path);
        // TODO: More helpful error message if engine binary is not found.
        // For example, print contents of directory searched?
        let mut child = Command::new(&absolute_path)
            .stdout(Stdio::piped())
            .stdin(Stdio::piped())
            .spawn()?;

        let stdout = BufReader::new(child.stdout.take().unwrap());
        let stdin = child.stdin.take().unwrap();

        let mut engine = Engine {
            child,
            stdout,
            stdin,
            name: self.path.to_string(),
        };

        engine.uci_write_line("uti")?;

        let mut options = vec![];
        loop {
            let input = engine.uci_read_line()?;
            match input.split_whitespace().next() {
                Some("utiok") => {
                    break;
                }
                Some("option") => {
                    options.push(parse_option(&input).unwrap()); // TODO: Handle error
                }
                None | Some(_) => {
                    // TODO: Print debug message
                }
            }
        }

        engine.uci_write_line("isready")?;

        loop {
            let input = engine.uci_read_line()?;
            if input.trim() == "readyok" {
                break;
            }
        }
        Ok(engine)
    }
}

pub struct Engine {
    child: Child,
    stdout: BufReader<ChildStdout>,
    stdin: ChildStdin,
    name: String,
}

impl Engine {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn uci_read_line(&mut self) -> Result<String> {
        self.read_line()
    }

    pub fn uci_write_line(&mut self, line: &str) -> Result<()> {
        writeln!(self.stdin, "{}", line)?;
        println!("> {}: {}", self.name, line);
        self.stdin.flush()
    }

    fn read_line(&mut self) -> Result<String> {
        let mut input = String::new();
        if self.stdout.read_line(&mut input)? == 0 {
            Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Read 0 bytes from engine",
            ))
        } else {
            print!("< {}: {}", self.name, input);
            Ok(input)
        }
    }
}
