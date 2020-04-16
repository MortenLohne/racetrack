use crate::uci_parser::parse_option;
use std::error::Error;
use std::fmt::Formatter;
use std::io::Result;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::string::ToString;
use std::{fmt, io};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EngineBuilder<'a> {
    pub path: &'a str,
}

impl<'a> EngineBuilder<'a> {
    /// Initialize the engine, including starting the binary and reading the engine's available uci commands.
    pub fn init(&self) -> Result<Engine> {
        let mut child = Command::new(&self.path)
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

#[derive(Debug)]
pub struct UciError {
    kind: UciErrorKind,
    desc: String,
    source: Option<Box<dyn Error>>,
}

impl UciError {
    pub fn new_parse_error(desc: String) -> UciError {
        UciError {
            kind: UciErrorKind::ParseError,
            desc,
            source: None,
        }
    }

    pub fn new_root(kind: UciErrorKind, desc: String) -> UciError {
        UciError {
            kind,
            source: None,
            desc,
        }
    }

    pub fn new_caused_by(kind: UciErrorKind, desc: String, source: Box<dyn Error>) -> UciError {
        UciError {
            kind,
            source: Some(source),
            desc,
        }
    }
}

impl fmt::Display for UciError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.kind {
            UciErrorKind::ParseError => write!(f, "Uci parser error. {}", self.desc)?,
            UciErrorKind::InvalidOption => write!(f, "invalid option description. {}", self.desc)?,
        }
        if let Some(ref source) = self.source {
            write!(f, ". Caused by: {}", source)?
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum UciErrorKind {
    ParseError,
    InvalidOption,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq)]
pub struct UciOption {
    pub name: String,
    pub option_type: UciOptionType,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq)]
pub enum UciOptionType {
    Check(bool),
    Spin(i64, i64, i64), // Contains current value, minimum value, maximum value
    Combo(String, Vec<String>), // Contains current value, predefined values
    Button,
    String(String), // Contains the current value
}
