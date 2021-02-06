use crate::uci::parser::parse_option;
use log::{debug, info, warn};
use std::io;
use std::io::Result;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::string::ToString;
use std::time::Duration;
use std::{env, thread};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EngineBuilder {
    pub path: String,
    pub args: Option<String>,
}

impl EngineBuilder {
    /// Initialize the engine, including starting the binary and reading the engine's available uci commands.
    pub fn init(&self) -> Result<Engine> {
        // TODO: Error for not permission to current directory
        let mut absolute_path = env::current_dir()?;
        absolute_path.push(&self.path);

        // TODO: More helpful error message if engine binary is not found.
        // For example, print contents of directory searched?
        let mut child = match &self.args {
            Some(args) => Command::new(&absolute_path)
                .args(args.split_whitespace())
                .stdout(Stdio::piped())
                .stdin(Stdio::piped())
                .spawn()?,
            None => Command::new(&absolute_path)
                .stdout(Stdio::piped())
                .stdin(Stdio::piped())
                .spawn()?,
        };

        let stdout = BufReader::new(child.stdout.take().unwrap());
        let stdin = child.stdin.take().unwrap();

        let mut engine = Engine {
            child,
            stdout,
            stdin,
            name: self.path.to_string(),
            builder: self.clone(),
        };

        engine.uci_write_line("tei")?;

        let mut options = vec![];
        loop {
            let input = engine.uci_read_line()?;
            match input.split_whitespace().next() {
                Some("teiok") => {
                    break;
                }
                Some("option") => {
                    options.push(parse_option(&input).unwrap()); // TODO: Handle error
                }
                s => info!("Unexpected message \"{}\", ignoring", s.unwrap_or_default()),
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
    builder: EngineBuilder,
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
        debug!("> {}: {}", self.name, line);
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
            debug!("< {}: {}", self.name, input.trim());
            Ok(input)
        }
    }

    /// Restart the engine from scratch
    pub fn restart(&mut self) -> Result<()> {
        self.shutdown()?;
        *self = self.builder.init()?;
        Ok(())
    }

    /// Shuts down the engine process. If the engine does not respond to a `quit` command, kill it.
    pub fn shutdown(&mut self) -> Result<ExitStatus> {
        info!("Shutting down {}", self.name);
        if let Some(exit_status) = self.child.try_wait()? {
            info!("{} has already exited", self.name);
            return Ok(exit_status);
        }
        self.uci_write_line("quit")?;
        thread::sleep(Duration::from_secs(1));
        match self.child.try_wait()? {
            Some(exit_status) => {
                info!("{} shut down successfully", self.name);
                Ok(exit_status)
            }
            None => {
                warn!("{} failed to shut down, killing", self.name);
                self.child.kill()?;
                thread::sleep(Duration::from_secs(1));
                let result = self.child.wait()?;
                warn!("{} killed successfully", self.name);
                Ok(result)
            }
        }
    }
}
