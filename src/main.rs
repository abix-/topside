//! App entry: `topside` runs the 2D game. Each launch rolls a new world
//! from the clock; `--seed N` rolls that one instead (the game tests use
//! it so every run is the same world). `--world small` (the default,
//! design.md "The small world first") or `--world big` picks the world.
//! `--port N` is the control plane's port (15703 when not given), so two
//! games can run side by side.

// One window, the default for every program: no console window beside it.
// Debug builds keep the console for their output.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::time::{SystemTime, UNIX_EPOCH};

use topside::content::WorldKind;

fn main() {
    // Every panic with its whole backtrace in topside.err.log, however the
    // game was launched (the launcher sets no RUST_BACKTRACE): a crash is
    // read from where it happened, not guessed at.
    let earlier = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        earlier(info);
        eprintln!("backtrace:\n{}", std::backtrace::Backtrace::force_capture());
    }));
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
    let port = value("--port").map(|p| p.parse().unwrap_or_else(|_| panic!("--port {p}: a port number"))).unwrap_or(topside::control_plane::DEFAULT_PORT);
    topside::build_app(false, seed, world, port).run();
}
