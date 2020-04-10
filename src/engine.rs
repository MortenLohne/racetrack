use crate::uci_parser::parse_option;
use std::fmt::Formatter;
use std::io::Result;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::str::FromStr;
use std::string::ToString;
use std::{fmt, io, result};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EngineBuilder<'a> {
    pub path: &'a str,
}

#[derive(Debug, PartialEq, Eq)]
pub struct UciError {
    kind: UciErrorKind,
    desc: String,
}

impl UciError {
    pub fn new_parse_error(desc: String) -> UciError {
        UciError {
            kind: UciErrorKind::ParseError,
            desc,
        }
    }
}

impl fmt::Display for UciError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.desc)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum UciErrorKind {
    ParseError,
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct UciOption {
    pub name: String,
    pub option_type: UciOptionType,
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum UciOptionType {
    Check(bool),
    Spin(i64, i64, i64), // Contains current value, minimum value, maximum value
    Combo(String, Vec<String>), // Contains current value, predefined values
    Button,
    String(String), // Contains the current value
}

impl<'a> EngineBuilder<'a> {
    pub(crate) fn init(&self) -> Result<Engine> {
        let mut child = Command::new(&self.path)
            .stdout(Stdio::piped())
            .stdin(Stdio::piped())
            .spawn()?;

        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut stdin = child.stdin.take().unwrap();

        writeln!(stdin, "uti")?;
        stdin.flush()?;

        let mut input = String::new();
        let mut options = vec![];
        loop {
            stdout.read_line(&mut input)?;
            if input.trim() == "utiok" {
                input.clear();
                break;
            }
            // TODO: Do not crash on id lines/other non-option lines
            options.push(parse_option(&input).unwrap()); // TODO: Handle error
            input.clear();
        }

        writeln!(stdin, "isready")?;
        stdin.flush()?;

        loop {
            stdout.read_line(&mut input)?;
            if input.trim() == "utiok" {
                input.clear();
                break;
            }
            input.clear();
        }

        writeln!(stdin, "utinewgame")?;
        stdin.flush()?;

        Ok(Engine {
            child,
            stdout,
            stdin,
            name: self.path.to_string(),
        })
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
        self.stdin.flush()
    }

    fn read_line(&mut self) -> Result<String> {
        let mut input = String::new();
        if let Ok(0) = self.stdout.read_line(&mut input) {
            Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Read 0 bytes from engine",
            ))
        } else {
            Ok(input)
        }
    }
}
