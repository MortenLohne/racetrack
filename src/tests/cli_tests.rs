use std::time::Duration;

use tiltak::position::Komi;

use crate::cli;
use crate::openings;

#[test]
fn cli_test() {
    let input: &str = "./racetrack -s 6 --games 2000 --all-engines tc=60+0.6 --concurrency 10 --book 6s_4ply_balanced_openings.txt --ptnout tako_vs_tiltak.ptn -l racetrack.log --shuffle-book --engine path=tiltak --engine path=taktician";

    // "tei -multi-cut -table-mem 512000000" is quoted in the real input, which becomes one element in the argument list
    let tail_input = ["arg=tei -multi-cut -table-mem 512000000"];

    let cli_options = cli::parse_cli_arguments_from(
        input
            .split_whitespace()
            .chain(tail_input)
            .map(|word| word.into()),
    );

    let expected = cli::CliOptions {
        size: 6,
        concurrency: 10,
        games: 2000,
        time: Duration::from_secs(60),
        increment: Duration::from_millis(600),
        engine_paths: ["tiltak".to_string(), "taktician".to_string()],
        engine_cli_args: [
            None,
            Some("tei -multi-cut -table-mem 512000000".to_string()),
        ],
        engine_tei_args: [vec![], vec![]],
        pgnout: Some("tako_vs_tiltak.ptn".to_string()),
        book_path: Some("6s_4ply_balanced_openings.txt".to_string()),
        book_format: openings::BookFormat::MoveList,
        shuffle_book: true,
        book_start_index: 0,
        log_file_name: Some("racetrack.log".to_string()),
        komi: Komi::default(),
    };

    if let Err(err) = &cli_options {
        eprintln!("{err}")
    }

    assert_eq!(cli_options.unwrap(), expected)
}

#[test]
fn shuffle_book_test() {
    let input: &str =
        "./racetrack -s 5 --games 100 --book openings.ptn --engine path=tiltak --engine path=taktician --komi 2.5 --book-start 10 --book-format ptn --all-engines tc=60+0.6";

    let cli_options =
        cli::parse_cli_arguments_from(input.split_whitespace().map(|word| word.into()));

    let expected = cli::CliOptions {
        size: 5,
        concurrency: 1,
        games: 100,
        time: Duration::from_secs(60),
        increment: Duration::from_millis(600),
        engine_paths: ["tiltak".to_string(), "taktician".to_string()],
        engine_cli_args: [None, None],
        engine_tei_args: [vec![], vec![]],
        pgnout: None,
        book_path: Some("openings.ptn".to_string()),
        book_format: openings::BookFormat::Pgn,
        shuffle_book: false,
        book_start_index: 9,
        log_file_name: None,
        komi: Komi::from_half_komi(5).unwrap(),
    };

    if let Err(err) = &cli_options {
        eprintln!("{err}")
    }

    assert_eq!(cli_options.unwrap(), expected)
}
