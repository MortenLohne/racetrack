use crate::{
    openings::{self, BookFormat},
    uci::parser,
};
use clap::{self, Arg, ArgAction, Command};
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
    pub engine_cli_args: [Option<String>; 2],
    pub engine_tei_args: [Vec<(String, String)>; 2],
    pub pgnout: Option<String>,
    pub book_path: Option<String>,
    pub book_format: openings::BookFormat,
    pub shuffle_book: bool,
    pub book_start_index: usize,
    pub log_file_name: Option<String>,
    pub komi: Komi,
}

#[derive(Debug)]
pub struct CliEngine {
    path: String,
    cli_args: Option<String>,
    time: Duration,
    increment: Duration,
    tei_settings: Vec<(String, String)>,
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
                .num_args(1)
                .default_value("5")
                .value_parser(clap::value_parser!(u64).range(4..=8)),
        )
        .arg(Arg::new("engine-flag")
            .help("Specify the file path of an engine. Must be used twice to add both engines.")
            .short('e')
            .long("engine")
            .num_args(0..)
            .action(ArgAction::Append))
        .arg(Arg::new("engine-flag-all")
            .help("Specify the file path of an engine. Must be used twice to add both engines.")
            .long("all-engines")
            .num_args(0..))
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
            .num_args(1)
            .value_parser(clap::value_parser!(usize)))
        .arg(Arg::new("file")
            .help("Output file for all game PTNs.\nIf the file already exists, new games will be appended.")
            .long("ptnout")
            .num_args(1))
        .arg(Arg::new("book")
            .help("Start each game from an opening from the file. Each opening is played twice, with different colors. If there are more game pairs than openings, the openings will start to repeat. An opening file is included in the git repository.")
            .short('b')
            .long("book")
            .num_args(1)
            .value_name("file.txt"))
        .arg(Arg::new("book-format")
            .long("book-format")
            .help("Opening book format. The included books are in the default 'move-list' format.")
            .num_args(1)
            .requires("book")
            .default_value("move-list")
            .value_parser(["move-list", "tps", "ptn"]))
        .arg(Arg::new("book-start")
            .long("book-start")
            .help("Start from the opening with the specified index. Starts at 1.")
            .num_args(1)
            .requires("book")
            .conflicts_with("shuffle-book")
            .value_parser(clap::value_parser!(u64).range(1..)))
        .arg(Arg::new("shuffle-book")
            .long("shuffle-book")
            .help("Shuffle the provided opening book.")
            .num_args(0)
            .requires("book"))
        .arg(Arg::new("log")
            .short('l')
            .long("log")
            .value_name("racetrack.log")
            .help("Name of debug logfile. If not set, no debug log will be written.")
            .num_args(1),
        )
        .arg(Arg::new("komi")
            .long("komi")
            .help("Play with komi, if the engines support it.")
            .num_args(1)
            .allow_hyphen_values(true)
            .default_value("0")
            .value_parser(|input: &str| {
                input.parse::<Komi>()
            }),
        )
        .try_get_matches_from(itr)?;

    let engines: Vec<CliEngine> = matches
        .get_occurrences::<String>("engine-flag")
        .unwrap()
        .enumerate()
        .map(|(id, engine)| {
            let mut engine_path = None;
            let mut engine_arg = None;
            let mut engine_tc_str = None;
            let mut tei_settings: Vec<(String, String)> = vec![];

            for arg in engine.chain(
                matches
                    .get_many::<String>("engine-flag-all")
                    .into_iter()
                    .flatten(),
            ) {
                if let Some((arg, value)) = arg.split_once('=') {
                    if let Some(option_arg) = arg.strip_prefix("option.") {
                        if tei_settings
                            .iter()
                            .any(|(a, _)| a.eq_ignore_ascii_case(option_arg))
                        {
                            panic!(
                                "Duplicate value for tei argument {} for engine #{}",
                                option_arg,
                                id + 1
                            )
                        } else {
                            assert!(!option_arg.eq_ignore_ascii_case("HalfKomi"));
                            tei_settings.push((option_arg.to_string(), value.to_string()));
                        }
                    } else {
                        match arg {
                            "path" if engine_path.is_some() => {
                                panic!(
                                    "Duplicate path arguments \"{}\" and \"{}\" for engine #{}",
                                    engine_path.unwrap(),
                                    value,
                                    id + 1
                                )
                            }
                            "path" => engine_path = Some(value),
                            "arg" if engine_arg.is_some() => {
                                panic!(
                                    "Duplicate arg arguments \"{}\" and \"{}\" for engine #{}",
                                    engine_arg.unwrap(),
                                    value,
                                    id + 1
                                )
                            }
                            "arg" => engine_arg = Some(value),
                            "tc" if engine_tc_str.is_some() => {
                                panic!(
                                    "Duplicate tc arguments \"{}\" and \"{}\" for engine #{}",
                                    engine_tc_str.unwrap(),
                                    value,
                                    id + 1
                                )
                            }
                            "tc" => engine_tc_str = Some(value),
                            _ => panic!("Unknown argument {} for engine #{}", arg, id + 1),
                        }
                    }
                } else {
                    panic!("Invalid input {}", arg); // TODO: Don't panic, better error message
                }
            }

            let (time, increment) = parser::parse_tc(engine_tc_str.unwrap()).unwrap(); // TODO: Error message

            CliEngine {
                path: engine_path.unwrap().to_string(), // TODO: Error message
                cli_args: engine_arg.map(ToString::to_string),
                time,
                increment,
                tei_settings,
            }
        })
        .collect();

    println!("Engines: ");
    for engine in engines.iter() {
        println!("{:?}", engine)
    }
    println!();

    assert_eq!(
        engines.len(),
        2,
        "Got {} engines, only 2 is supported",
        engines.len()
    );

    assert!(engines.iter().all(|engine| engine.time == engines[0].time)); // TODO: Error message
    assert!(engines
        .iter()
        .all(|engine| engine.increment == engines[0].increment)); // TODO: Error message

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
        time: engines[0].time,           // TODO: Support asymmetric TC
        increment: engines[0].increment, // TODO: Support asymmetric TC
        engine_paths: [engines[0].path.to_string(), engines[1].path.to_string()],
        engine_cli_args: [engines[0].cli_args.clone(), engines[1].cli_args.clone()],
        engine_tei_args: [
            engines[0].tei_settings.clone(),
            engines[1].tei_settings.clone(),
        ],
        pgnout: matches.get_one("file").cloned(),
        book_path: matches.get_one("book").cloned(),
        book_format,
        shuffle_book: *matches.get_one::<bool>("shuffle-book").unwrap(),
        book_start_index: *matches.get_one::<u64>("book-start").unwrap_or(&1) as usize - 1,
        log_file_name: matches.get_one::<String>("log").cloned(),
        komi: *matches.get_one::<Komi>("komi").unwrap(),
    })
}
