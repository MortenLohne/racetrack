# Racetrack

Racetrack is a cli to play a match between two Tak engines, to see how they play and measure their relative strength.

Racetrack uses the text-based TEI (Tak Engine Interface) to communicate with the engine binaries, very similar to [UCI](https://en.wikipedia.org/wiki/Universal_Chess_Interface) for Chess. The engines communicate with Racetrack by simply sending string commands through stdin/stdout, see the TEI section for more.

## TEI

Tak Engine Interface is a protocol based on [Universal Chess Interface](https://ucichessengine.wordpress.com/2011/03/16/description-of-uci-protocol/), intended to be as similar as possible. The key difference are:
* The protocol uses `tei` everywhere `uci` would be used, like `teiok` and `teinewgame`
* Additionally, `teinewgame` *requires* the GUI to send a size parameter. So `teinewgame 5` for size 5.  
* Move and position notations are obviously different. [Portable Tak Notation](https://www.reddit.com/r/Tak/wiki/portable_tak_notation) and [Tak Position System](https://www.reddit.com/r/Tak/wiki/tak_positional_system) are used, respectively.

## Build

Building the project from source requires the Rust compiler and Cargo (Rust's package manager) installed, both included in the [Rust downloads.](https://www.rust-lang.org/tools/install)

To build and run:
```
cargo build --release
cargo run --release
```

This command will automatically fetch and build dependencies. The resulting binaries are written to `racetrack/target/release`.

## Usage

Run `racetrack --help` to see a full list of options.

Play two games between engine1 and engine2, with 60 seconds each for each game:

````
racetrack --engine engine1 engine2 --games 2 --tc 60
````