//! App entry: `topside` runs the 2D game. Each launch rolls a new world
//! from the clock; `--seed N` rolls that one instead (the game tests use
//! it so every run is the same world). `--world small` (the default,
//! design.md "The small world first") or `--world big` picks the world.

use std::time::{SystemTime, UNIX_EPOCH};

use topside::content::WorldKind;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1));
    let seed = value("--seed").and_then(|s| s.parse().ok()).unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64)
    });
    let world = match value("--world") {
        None => WorldKind::Small,
        Some(name) => WorldKind::from_name(name).unwrap_or_else(|| panic!("no world '{name}': small or big")),
    };
    topside::build_app(false, seed, world).run();
}
