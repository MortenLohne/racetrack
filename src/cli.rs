use crate::{
    openings::{self, BookFormat},
    sprt::SprtParameters,
    tournament::TournamentType,
    uci::parser,
};
use clap::{self, Arg, ArgAction, Command};
use std::{env, ffi::OsString, num::NonZeroUsize, process, time::Duration};
use tiltak::position::Komi;

#[derive(Clone, Debug, PartialEq)]
pub struct CliOptions {
    pub size: usize,
    pub concurrency: usize,
    pub games: usize,
    pub engines: Vec<CliEngine>,
    pub pgnout: Option<String>,
    pub book_path: Option<String>,
    pub book_format: openings::BookFormat,
    pub shuffle_book: bool,
    pub book_start_index: usize,
    pub log_file_name: Option<String>,
    pub komi: Komi,
    pub tournament_type: TournamentType,
    pub sprt: Option<SprtParameters>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliEngine {
    pub path: String,
    pub cli_args: Option<String>,
    pub time: Duration,
    pub increment: Duration,
    pub tei_settings: Vec<(String, String)>,
}

pub fn parse_cli_arguments() -> CliOptions {
    parse_cli_arguments_from(&mut env::args_os()).unwrap_or_else(|err| err.exit())
}

pub fn parse_cli_arguments_from(
    itr: impl Iterator<Item = OsString>,
) -> Result<CliOptions, clap::Error> {
    let after_help: &'static str = color_print::cstr!(
        r#"<bold><underline>Per-engine options:</underline></bold>
        These options are set on each individual engine following a `--engine` argument, or to <italic>all</italic> engines following an `--all-engines` argument

        <bold>path=PATH</bold>
            File path to engine binary.
        <bold>tc=TC</bold>
            Time control for each game, in seconds. Format is time+increment, where the increment is optional.
        <bold>arg=ARGS</bold>
            Command-line arguments to pass to the engine.
        <bold>option.OPTION=VALUE</bold>
            Set tei <italic>option</italic> to <italic>value</italic> for the engine.
        "#
    );

    let matches = Command::new("Racetrack")
        .after_help(after_help)
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
            .help("Add an engine to the tournament, followed by configuration options for that engine, see below. Must be used once per engine.")
            .short('e')
            .long("engine")
            .value_name("options")
            .num_args(0..)
            .action(ArgAction::Append))
        .arg(Arg::new("engine-flag-all")
            .help("Set engine configuration options that apply to all engines.")
            .long("all-engines")
            .value_name("options")
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
            }))
        .arg(Arg::new("format")
            .long("format")
            .help("Choose tournament format. See the README for details.")
            .num_args(1)
            .allow_hyphen_values(true)
            .default_value("round-robin")
            .value_parser(clap::builder::PossibleValuesParser::new(["gauntlet", "round-robin", "book-test", "sprt"]))
        )
        .arg(Arg::new("sprt-flag")
            .long("sprt")
            .help("Perform a sequential probability ratio test.")
            .value_name("options")
            .num_args(0..)
            .action(ArgAction::Append))
        .try_get_matches_from(itr)?;

    let engines: Vec<CliEngine> = matches
        .get_occurrences::<String>("engine-flag")
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(id, engine)| {
            let mut engine_path = None;
            let mut engine_arg = None;
            let mut engine_tc_str = None;
            let mut tei_settings: Vec<(String, String)> = vec![];

            for full_arg in engine.chain(
                matches
                    .get_many::<String>("engine-flag-all")
                    .into_iter()
                    .flatten(),
            ) {
                if let Some((arg, value)) = full_arg.split_once('=') {
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
                            _ => {
                                eprintln!(
                                    "Error: unknown argument {} for engine #{}",
                                    full_arg,
                                    id + 1
                                );
                                process::exit(1)
                            }
                        }
                    }
                } else {
                    eprintln!("Error: Expected key=val, found {}", full_arg);
                    process::exit(1)
                }
            }
            let Some(path) = engine_path else {
                eprintln!("Error: Missing binary path for engine #{}", id + 1);
                process::exit(1)
            };
            let Some(tc_str) = engine_tc_str else {
                eprintln!("Error: Missing time control for engine {}", path);
                process::exit(1)
            };
            let (time, increment) = parser::parse_tc(tc_str).unwrap_or_else(|err| {
                eprintln!("{} for engine {}", err, path);
                process::exit(1)
            });

            CliEngine {
                path: path.to_string(),
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

    if engines.is_empty() {
        eprintln!("Error: No engines added to tournament, use the --engine argument",);
        process::exit(1);
    }

    let tournament_type = match (
        matches.get_one::<String>("format").unwrap().as_str(),
        engines.len(),
    ) {
        ("gauntlet", n @ 3..) => TournamentType::Gauntlet(NonZeroUsize::new(n - 1).unwrap()),
        ("gauntlet", n) => {
            eprintln!("Error: Got {} engines, at least 3 is required", n);
            process::exit(1);
        }
        ("round-robin", n @ 2..) => TournamentType::RoundRobin(n),
        ("round-robin", n) => {
            eprintln!("Error: Got {} engines, at least 2 is required", n);
            process::exit(1);
        }
        ("book-test", n @ 1..) => TournamentType::BookTest(n),
        ("book-test", n) => {
            eprintln!("Error: Got {} engines, at least 1 is required", n);
            process::exit(1);
        }
        ("sprt", 2) => TournamentType::Sprt,
        ("sprt", n) => {
            eprintln!("Error: Got {} engines, require exactly 2", n);
            process::exit(1);
        }
        (s, _) => panic!("Unsupported tournament format {}", s),
    };

    let num_games = *matches.get_one::<usize>("games").unwrap();

    if num_games % tournament_type.alignment() != 0 {
        let format_name = match tournament_type {
            TournamentType::Gauntlet(_) => "gauntlet",
            TournamentType::RoundRobin(_) => "round robin",
            TournamentType::BookTest(_) => "book-test",
            TournamentType::Sprt => "sprt",
        };
        eprintln!(
            "Warning: The tournament will not give all engines an equal number of white and black games.\nFor a {} tournament with {} engines, the total number of games should be divisible by {}",
            format_name, tournament_type.num_engines(), tournament_type.alignment()
        );
        eprintln!();
    }

    let book_format = match matches.get_one::<String>("book-format").unwrap().as_str() {
        "move-list" => BookFormat::MoveList,
        "tps" => BookFormat::Fen,
        "ptn" => BookFormat::Pgn,
        s => panic!("Unsupported book format {}", s),
    };

    let mut sprt = None;
    let sprt_options = matches.get_many::<String>("sprt-flag");
    if let Some(sprt_options) = sprt_options {
        match tournament_type {
            TournamentType::Sprt => {}
            _ => {
                eprintln!(
                    "Error: sprt option present but tournament type is {:?}",
                    tournament_type
                );
                process::exit(1);
            }
        }

        let mut elo0 = None;
        let mut elo1 = None;
        let mut alpha = None;
        let mut beta = None;

        for option in sprt_options {
            if let Some((arg, value)) = option.split_once('=') {
                match arg {
                    "elo0" if elo0.is_some() => panic!(
                        "Duplicate elo0 arguments \"{}\" and \"{}\" for sprt",
                        elo0.unwrap(),
                        value
                    ),
                    "elo0" => elo0 = Some(value),
                    "elo1" if elo1.is_some() => panic!(
                        "Duplicate elo1 arguments \"{}\" and \"{}\" for sprt",
                        elo1.unwrap(),
                        value
                    ),
                    "elo1" => elo1 = Some(value),
                    "alpha" if alpha.is_some() => panic!(
                        "Duplicate alpha arguments \"{}\" and \"{}\" for sprt",
                        alpha.unwrap(),
                        value
                    ),
                    "alpha" => alpha = Some(value),
                    "beta" if beta.is_some() => panic!(
                        "Duplicate beta arguments \"{}\" and \"{}\" for sprt",
                        beta.unwrap(),
                        value
                    ),
                    "beta" => beta = Some(value),
                    _ => {
                        eprintln!("Error: unknown argument {} for sprt", option);
                        process::exit(1)
                    }
                }
            } else {
                eprintln!("Error: Expected key=val, found {}", option);
                process::exit(1)
            }
        }

        let Some(elo0) = elo0 else {
            eprintln!("Error: Missing elo0 for sprt");
            process::exit(1)
        };
        let Some(elo1) = elo1 else {
            eprintln!("Error: Missing elo1 for sprt");
            process::exit(1)
        };
        let alpha = alpha.unwrap_or("0.05");
        let beta = beta.unwrap_or("0.05");

        let elo0 = elo0.parse::<f64>().unwrap_or_else(|err| {
            eprintln!("{} for sprt elo0", err);
            process::exit(1)
        });
        let elo1 = elo1.parse::<f64>().unwrap_or_else(|err| {
            eprintln!("{} for sprt elo1", err);
            process::exit(1)
        });
        let alpha = alpha.parse::<f64>().unwrap_or_else(|err| {
            eprintln!("{} for sprt alpha", err);
            process::exit(1)
        });
        let beta = beta.parse::<f64>().unwrap_or_else(|err| {
            eprintln!("{} for sprt beta", err);
            process::exit(1)
        });

        if elo0 >= elo1 {
            eprintln!("elo1 ({}) must be greater than elo0 ({})", elo1, elo0);
            process::exit(1)
        }
        if alpha <= 0.0 || alpha >= 0.5 {
            eprintln!("invalid value {} for sprt alpha", alpha);
            process::exit(1)
        }
        if beta <= 0.0 || beta >= 0.5 {
            eprintln!("invalid value {} for sprt beta", beta);
            process::exit(1)
        }

        sprt = Some(SprtParameters::new(elo0, elo1, alpha, beta));
    }

    Ok(CliOptions {
        size: *matches.get_one::<u64>("size").unwrap() as usize,
        concurrency: *matches.get_one::<u64>("concurrency").unwrap() as usize,
        games: num_games,
        engines,
        pgnout: matches.get_one("file").cloned(),
        book_path: matches.get_one("book").cloned(),
        book_format,
        shuffle_book: *matches.get_one::<bool>("shuffle-book").unwrap(),
        book_start_index: *matches.get_one::<u64>("book-start").unwrap_or(&1) as usize - 1,
        log_file_name: matches.get_one::<String>("log").cloned(),
        komi: *matches.get_one::<Komi>("komi").unwrap(),
        tournament_type,
        sprt,
    })
}
