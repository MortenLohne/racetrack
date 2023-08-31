use crate::{
    openings::{self, BookFormat},
    uci::parser,
};
use clap::{self, App, Arg};
use std::{env, ffi::OsString, num::NonZeroUsize, time::Duration};
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
    let matches = App::new("Racetrack")
        .version("0.2.1")
        .author("Morten Lohne")
        .about("Play a match between two or more Tak engines")
        .arg(
            Arg::with_name("size")
                .short("s")
                .long("size")
                .help("Board size.")
                .takes_value(true)
                .default_value("5")
                .possible_values(&["4", "5", "6", "7", "8"]),
        )
        .arg(Arg::with_name("engine-path")
            .help("Specify the file path of an engine. Must be used twice to add both engines.")
            .short("e")
            .long("engine")
            .required(true)
            .multiple(true)
            .takes_value(true)
            .min_values(2)
            .max_values(2)
        )
        .arg(Arg::with_name("concurrency")
            .help("Number of games to run in parallel.")
            .default_value("1")
            .short("c")
            .long("concurrency")
            .value_name("n")
            .validator(|input| {
                match input.parse::<usize>() {
                    Ok(num) => {
                        if num == 0 || num > 1024 {
                            Err("Must be between 1 and 1024".to_string())
                        } else { Ok(()) }
                    }
                    Err(err) => {
                        Err(err.to_string())
                    }
                }
            }))
        .arg(Arg::with_name("games")
            .help("Number of games to play.")
            .short("g")
            .long("games")
            .required(true)
            .takes_value(true))
        .arg(Arg::with_name("file")
            .help("Output file for all game PTNs.\nIf the file already exists, new games will be appended.")
            .long("ptnout")
            .takes_value(true))
        .arg(Arg::with_name("book")
            .help("Start each game from an opening from the file. Each opening is played twice, with different colors. If there are more game pairs than openings, the openings will start to repeat. An opening file is included in the git repository.")
            .short("b")
            .long("book")
            .takes_value(true)
            .value_name("file.txt"))
        .arg(Arg::with_name("book-format")
            .long("--book-format")
            .help("Opening book format. The included books are in the default 'move-list' format.")
            .takes_value(true)
            .requires("book")
            .default_value("move-list")
            .possible_values(&["move-list", "tps", "ptn"]))
        .arg(Arg::with_name("book-start")
            .long("--book-start")
            .help("Start from the opening with the specified index. Starts at 1.")
            .takes_value(true)
            .requires("book")
            .conflicts_with("shuffle-book")
            .validator(|input|
                input.parse::<NonZeroUsize>()
                    .map(|_| ())
                    .map_err(|err| err.to_string())
                )
            )
        .arg(Arg::with_name("shuffle-book")
            .long("--shuffle-book")
            .help("Shuffle the provided opening book.")
            .takes_value(false)
            .requires("book"))
        .arg(Arg::with_name("tc")
            .help("Time control for each game, in seconds. Format is time+increment, where the increment is optional.")
            .long("tc")
            .takes_value(true)
            .required(true))
        .arg(Arg::with_name("engine1-args")
            .help("Command-line argument string to pass to engine 1.")
            .long("engine1-args")
            .takes_value(true)
            .value_name("args")
            .allow_hyphen_values(true)
        )
        .arg(Arg::with_name("engine2-args")
            .help("Command-line argument string to pass to engine 2.")
            .long("engine2-args")
            .takes_value(true)
            .value_name("args")
            .allow_hyphen_values(true)
        )
        .arg(Arg::with_name("log")
            .short("l")
            .long("log")
            .value_name("racetrack.log")
            .help("Name of debug logfile. If not set, no debug log will be written.")
            .takes_value(true),
        )
        .arg(Arg::with_name("komi")
            .long("komi")
            .help("Play with komi, if the engines support it.")
            .takes_value(true)
            .allow_hyphen_values(true)
            .default_value("0")
            .validator(|input| {
                input.parse::<Komi>().map(|_| ())
            }),
        )
        .get_matches_from_safe(itr)?;

    let (time, increment) =
        parser::parse_tc(matches.value_of("tc").unwrap()).unwrap_or_else(|err| panic!("{}", err));

    let engine_path_matches = matches
        .values_of("engine-path")
        .unwrap()
        .collect::<Vec<_>>();

    let book_format = match matches.value_of("book-format").unwrap() {
        "move-list" => BookFormat::MoveList,
        "tps" => BookFormat::Fen,
        "ptn" => BookFormat::Pgn,
        s => panic!("Unsupported book format {}", s),
    };

    Ok(CliOptions {
        size: matches.value_of("size").unwrap().parse().unwrap(),
        concurrency: matches.value_of("concurrency").unwrap().parse().unwrap(),
        games: matches.value_of("games").unwrap().parse().unwrap(),
        time,
        increment,
        engine_paths: [
            engine_path_matches[0].to_string(),
            engine_path_matches[1].to_string(),
        ],
        engine_args: [
            matches.value_of("engine1-args").map(ToOwned::to_owned),
            matches.value_of("engine2-args").map(ToOwned::to_owned),
        ],
        pgnout: matches.value_of("file").map(|s| s.to_string()),
        book_path: matches.value_of("book").map(|s| s.to_string()),
        book_format,
        shuffle_book: matches.is_present("shuffle-book"),
        book_start_index: matches
            .value_of("book-start")
            .unwrap_or("1")
            .parse::<usize>()
            .unwrap()
            - 1,
        log_file_name: matches.value_of("log").map(|s| s.to_string()),
        komi: matches.value_of("komi").unwrap().parse().unwrap(),
    })
}
