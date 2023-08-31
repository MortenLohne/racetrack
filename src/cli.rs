use crate::{
    openings::{self, BookFormat},
    uci::parser,
};
use clap::{self, Arg, Command};
use std::{env, ffi::OsString, time::Duration};
use tiltak::position::Komi;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliOptions {
    pub size: usize,
    pub concurrency: usize,
    pub games: usize,
    pub time: Duration,
    pub increment: Duration,
    pub engine_paths: [String; 2],
    pub engine_args: [Option<String>; 2],
    pub pgnout: Option<String>,
    pub book_path: Option<String>,
    pub book_format: openings::BookFormat,
    pub shuffle_book: bool,
    pub book_start_index: usize,
    pub log_file_name: Option<String>,
    pub komi: Komi,
}

pub fn parse_cli_arguments() -> CliOptions {
    parse_cli_arguments_from(&mut env::args_os()).unwrap_or_else(|err| err.exit())
}

pub fn parse_cli_arguments_from(
    itr: impl Iterator<Item = OsString>,
) -> Result<CliOptions, clap::Error> {
    let matches = Command::new("Racetrack")
        .version("0.2.1")
        .author("Morten Lohne")
        .about("Play a match between two or more Tak engines")
        .arg(
            Arg::new("size")
                .short('s')
                .long("size")
                .help("Board size.")
                .takes_value(true)
                .default_value("5")
                .value_parser(clap::value_parser!(u64).range(4..=8)),
        )
        .arg(Arg::new("engine-path")
            .help("Specify the file path of an engine. Must be used twice to add both engines.")
            .short('e')
            .long("engine")
            .required(true)
            .takes_value(true)
            .min_values(2)
            .max_values(2)
        )
        .arg(Arg::new("concurrency")
            .help("Number of games to run in parallel.")
            .default_value("1")
            .short('c')
            .long("concurrency")
            .value_name("n")
            .value_parser(clap::value_parser!(u64).range(1..=1024)))
        .arg(Arg::new("games")
            .help("Number of games to play.")
            .short('g')
            .long("games")
            .required(true)
            .takes_value(true)
            .value_parser(clap::value_parser!(usize)))
        .arg(Arg::new("file")
            .help("Output file for all game PTNs.\nIf the file already exists, new games will be appended.")
            .long("ptnout")
            .takes_value(true))
        .arg(Arg::new("book")
            .help("Start each game from an opening from the file. Each opening is played twice, with different colors. If there are more game pairs than openings, the openings will start to repeat. An opening file is included in the git repository.")
            .short('b')
            .long("book")
            .takes_value(true)
            .value_name("file.txt"))
        .arg(Arg::new("book-format")
            .long("--book-format")
            .help("Opening book format. The included books are in the default 'move-list' format.")
            .takes_value(true)
            .requires("book")
            .default_value("move-list")
            .value_parser(["move-list", "tps", "ptn"]))
        .arg(Arg::new("book-start")
            .long("--book-start")
            .help("Start from the opening with the specified index. Starts at 1.")
            .takes_value(true)
            .requires("book")
            .conflicts_with("shuffle-book")
            .value_parser(clap::value_parser!(u64).range(1..)))
        .arg(Arg::new("shuffle-book")
            .long("--shuffle-book")
            .help("Shuffle the provided opening book.")
            .takes_value(false)
            .requires("book"))
        .arg(Arg::new("tc")
            .help("Time control for each game, in seconds. Format is time+increment, where the increment is optional.")
            .long("tc")
            .takes_value(true)
            .required(true))
        .arg(Arg::new("engine1-args")
            .help("Command-line argument string to pass to engine 1.")
            .long("engine1-args")
            .takes_value(true)
            .value_name("args")
            .allow_hyphen_values(true)
        )
        .arg(Arg::new("engine2-args")
            .help("Command-line argument string to pass to engine 2.")
            .long("engine2-args")
            .takes_value(true)
            .value_name("args")
            .allow_hyphen_values(true)
        )
        .arg(Arg::new("log")
            .short('l')
            .long("log")
            .value_name("racetrack.log")
            .help("Name of debug logfile. If not set, no debug log will be written.")
            .takes_value(true),
        )
        .arg(Arg::new("komi")
            .long("komi")
            .help("Play with komi, if the engines support it.")
            .takes_value(true)
            .allow_hyphen_values(true)
            .default_value("0")
            .value_parser(|input: &str| {
                input.parse::<Komi>()
            }),
        )
        .try_get_matches_from(itr)?;

    let (time, increment) = parser::parse_tc(matches.get_one::<String>("tc").unwrap())
        .unwrap_or_else(|err| panic!("{}", err));

    let engine_path_matches = matches
        .get_many::<String>("engine-path")
        .unwrap()
        .collect::<Vec<_>>();

    let book_format = match matches.get_one::<String>("book-format").unwrap().as_str() {
        "move-list" => BookFormat::MoveList,
        "tps" => BookFormat::Fen,
        "ptn" => BookFormat::Pgn,
        s => panic!("Unsupported book format {}", s),
    };

    Ok(CliOptions {
        size: *matches.get_one::<u64>("size").unwrap() as usize,
        concurrency: *matches.get_one::<u64>("concurrency").unwrap() as usize,
        games: *matches.get_one::<usize>("games").unwrap(),
        time,
        increment,
        engine_paths: [
            engine_path_matches[0].to_string(),
            engine_path_matches[1].to_string(),
        ],
        engine_args: [
            matches.get_one("engine1-args").cloned(),
            matches.get_one("engine2-args").cloned(),
        ],
        pgnout: matches.get_one("file").cloned(),
        book_path: matches.get_one("book").cloned(),
        book_format,
        shuffle_book: matches.contains_id("shuffle-book"),
        book_start_index: *matches.get_one::<u64>("book-start").unwrap_or(&1) as usize - 1,
        log_file_name: matches.get_one::<String>("log").cloned(),
        komi: *matches.get_one::<Komi>("komi").unwrap(),
    })
}
