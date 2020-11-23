use clap::{App, Arg, SubCommand};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliOptions {
    pub concurrency: usize,
    pub games: u64,
    pub engine_paths: Vec<String>,
    pub pgnout: Option<String>,
    pub book_path: Option<String>,
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
            .help("Start each game from an opening from the file. Each opening is played twice, with different colors.")
            .short("b")
            .long("book")
            .takes_value(true)
            .value_name("file.txt"))

        .subcommand(SubCommand::with_name("head2head")
            .arg(Arg::with_name("")))
        .subcommand(SubCommand::with_name("roundrobin"))
        .get_matches();

    CliOptions {
        concurrency: matches.value_of("concurrency").unwrap().parse().unwrap(),
        games: matches.value_of("games").unwrap().parse().unwrap(),
        engine_paths: matches
            .values_of("engine-path")
            .map(|values| values.map(|s| s.to_string()).collect())
            .unwrap_or(vec![]),
        pgnout: matches.value_of("file").map(|s| s.to_string()),
        book_path: matches.value_of("book").map(|s| s.to_string()),
    }
}
