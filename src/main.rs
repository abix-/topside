//! App entry: `topside` runs the 2D game. Each launch rolls a new world
//! from the clock.

use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64);
    topside::build_app(false, seed).run();
}
