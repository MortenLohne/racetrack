use clap::{App, Arg, ArgGroup, SubCommand};

pub struct CliOptions {
    pub concurrency: usize,
    pub rounds: Option<u64>,
    pub games: Option<u64>,
    pub engine_paths: Vec<String>,
    pub pgnout: Option<String>,
}

pub fn parse_cli_arguments() -> CliOptions {
    let matches = App::new("Tak match")
        .version("0.0.1")
        .author("Morten Lohne")
        .about("Play a match between two or more Tak engines")
        .arg(Arg::with_name("engine-path")
            .help("Add an engine to the tournament.")
            .short("e")
            .long("engine")
            .multiple(true)
            .takes_value(true)
            .min_values(2)
        )
        .arg(Arg::with_name("concurrency")
            .help("Number of games to run in parallel")
            .default_value("1")
            .short("c")
            .long("concurrency"))
        .arg(Arg::with_name("rounds")
            .help("Number of rounds to play.")
            .short("r")
            .long("rounds")
            .required(true)
            .takes_value(true)
            .conflicts_with("games"))
        .arg(Arg::with_name("games")
            .help("Number of games to play.")
            .short("g")
            .long("games")
            .required(true)
            .takes_value(true)
            .conflicts_with("rounds"))
        .arg(Arg::with_name("file")
            .help("Output file for all game PGNs.\nIf the file already exists, new games will be appended.")
            .long("pgnout")
            .takes_value(true))

        .subcommand(SubCommand::with_name("head2head")
            .arg(Arg::with_name("")))
        .subcommand(SubCommand::with_name("roundrobin"))
        .get_matches();

    CliOptions {
        concurrency: matches.value_of("concurrency").unwrap().parse().unwrap(),
        rounds: matches.value_of("rounds").map(|r| r.parse().unwrap()),
        games: matches.value_of("games").map(|r| r.parse().unwrap()),
        engine_paths: matches
            .values_of("engine-path")
            .map(|values| values.map(|s| s.to_string()).collect())
            .unwrap_or(vec![]),
        pgnout: matches.value_of("file").map(|s| s.to_string()),
    }
}
