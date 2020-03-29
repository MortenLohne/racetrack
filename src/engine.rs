use std::io;
use std::io::Result;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::string::ToString;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EngineBuilder<'a> {
    pub path: &'a str,
}

impl<'a> EngineBuilder<'a> {
    pub(crate) fn init(&self) -> Result<Engine> {
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
                    name: self.path.to_string(),
                }
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
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub fn uci_read_line(&mut self) -> Result<String> {
        self.read_line()
    }

    pub(crate) fn uci_write_line(&mut self, line: &str) -> Result<()> {
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

    pub(crate) fn initialize(&mut self) -> Result<()> {
        writeln!(self.stdin, "uti")?;
        self.stdin.flush()?;

        while self.read_line()?.trim() != "utiok" {}

        writeln!(self.stdin, "isready")?;
        self.stdin.flush()?;

        while self.read_line()?.trim() != "readyok" {}

        writeln!(self.stdin, "utinewgame")?;
        self.stdin.flush()?;
        Ok(())
    }
}
