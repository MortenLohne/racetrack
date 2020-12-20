use crate::uci::parser;
use clap::{App, Arg};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliOptions {
    pub concurrency: usize,
    pub games: u64,
    pub time: Duration,
    pub increment: Duration,
    pub engine_paths: Vec<String>,
    pub engine_args: Vec<Option<String>>,
    pub pgnout: Option<String>,
    pub book_path: Option<String>,
}

pub fn parse_cli_arguments() -> CliOptions {
    let matches = App::new("Tak match")
        .version("0.0.1")
        .author("Morten Lohne")
        .about("Play a match between two or more Tak engines")
        .arg(Arg::with_name("engine-path")
            .help("Specify the file path of an engine. Can be used several times to add more engines.")
            .short("e")
            .long("engine")
            .multiple(true)
            .takes_value(true)
            .min_values(2)
        )
        .arg(Arg::with_name("concurrency")
            .help("Number of games to run in parallel.")
            .default_value("1")
            .short("c")
            .long("concurrency")
            .value_name("1"))
        .arg(Arg::with_name("games")
            .help("Number of games to play.")
            .short("g")
            .long("games")
            .required(true)
            .takes_value(true))
        .arg(Arg::with_name("file")
            .help("Output file for all game PGNs.\nIf the file already exists, new games will be appended.")
            .long("pgnout")
            .takes_value(true))
        .arg(Arg::with_name("book")
            .help("Start each game from an opening from the file. Each opening is played twice, with different colors. If there are more game pairs than openings, the openings will start to repeat. An opening file is included in the git repository.")
            .short("b")
            .long("book")
            .takes_value(true)
            .value_name("file.txt"))
        .arg(Arg::with_name("tc")
            .help("Time control for each game, in seconds. Format is time+increment, where the increment is optional.")
            .long("tc")
            .takes_value(true)
            .required(true))
        .arg(Arg::with_name("engine1-args")
            .help("Command-line argument string to pass to engine 1")
            .long("engine1-args")
            .takes_value(true)
            .value_name("args")
        )
        .arg(Arg::with_name("engine2-args")
            .help("Command-line argument string to pass to engine 2")
            .long("engine2-args")
            .takes_value(true)
            .value_name("args")
        )
        .get_matches();

    let (time, increment) =
        parser::parse_tc(matches.value_of("tc").unwrap()).unwrap_or_else(|err| panic!("{}", err));

    CliOptions {
        concurrency: matches.value_of("concurrency").unwrap().parse().unwrap(),
        games: matches.value_of("games").unwrap().parse().unwrap(),
        time,
        increment,
        engine_paths: matches
            .values_of("engine-path")
            .map(|values| values.map(|s| s.to_string()).collect())
            .unwrap_or_default(),
        engine_args: vec![
            matches.value_of("engine1-args").map(ToOwned::to_owned),
            matches.value_of("engine2-args").map(ToOwned::to_owned),
        ],
        pgnout: matches.value_of("file").map(|s| s.to_string()),
        book_path: matches.value_of("book").map(|s| s.to_string()),
    }
}
