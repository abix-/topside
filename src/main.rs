//! App entry: `topside` runs the 2D game. Each launch rolls a new world
//! from the clock; `--seed N` rolls that one instead (the game tests use
//! it so every run is the same world).

use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let given = args
        .iter()
        .position(|a| a == "--seed")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok());
    let seed = given.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64)
    });
    topside::build_app(false, seed).run();
}
