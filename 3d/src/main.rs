//! App entry. `topside` runs the game; `topside --test` runs the
//! boot test headless and exits 0/1; `topside --world <name>` rolls
//! that reality (`content::WORLDS`) instead of the first.

// One window, the default for every program: no console window beside it.
// Debug builds keep the console for their output.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::time::{Duration, Instant};

use bevy::prelude::{Entity, Transform};
use modforge::survival::FULL;
use topside::{
    SimTick, WorldName, actor_map::ActorMapRes, build_app, content::world_def, player::Player,
    survival::Survival,
};

/// A windowed program has no console of its own, so when it is started from
/// a terminal (`--test`) it writes to that terminal, as Alacritty does.
#[cfg(windows)]
fn attach_console() {
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    unsafe extern "system" {
        fn AttachConsole(process_id: u32) -> i32;
    }
    unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}

#[cfg(not(windows))]
fn attach_console() {}

fn main() {
    attach_console();
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--test") {
        std::process::exit(run_boot_test());
    }
    let mut app = build_app(false);
    if let Some(i) = args.iter().position(|a| a == "--world") {
        let name = args.get(i + 1).cloned().unwrap_or_default();
        if let Err(e) = world_def(&name) {
            eprintln!("{e}");
            std::process::exit(2);
        }
        app.insert_resource(WorldName(name));
    }
    app.run();
}

/// Phased boot test, Endless-style: each phase checks one layer and
/// fails fast with the observed value.
fn run_boot_test() -> i32 {
    let mut app = build_app(true);
    println!("phase 1 pass: headless app built");

    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(500) {
        app.update();
    }
    let ticks = app.world().resource::<SimTick>().0;
    if ticks < 10 {
        println!("phase 2 FAIL: {ticks} simulation ticks in 500ms (expected >= 10 at 60 UPS)");
        return 1;
    }
    println!("phase 2 pass: {ticks} simulation ticks in 500ms");

    let mut map = ActorMapRes::default().0;
    let (first_id, first_row) = map.spawn(Entity::PLACEHOLDER).expect("empty map must spawn");
    map.remove(first_id);
    let (second_id, second_row) = map
        .spawn(Entity::PLACEHOLDER)
        .expect("map with a freed row must spawn");
    if first_row != second_row || first_id == second_id {
        println!(
            "phase 3 FAIL: row {first_row} -> {second_row}, id {first_id:?} -> {second_id:?} (row reused, id never)"
        );
        return 1;
    }
    println!("phase 3 pass: actor map reuses rows and never ids");

    let player = app
        .world_mut()
        .spawn((
            Transform::from_xyz(0.0, 1.7, 0.0),
            Player,
        ))
        .id();
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(200) {
        app.update();
    }
    let stats = app.world().get::<Survival>(player).expect("player carries survival").0;
    if !(stats.hunger < FULL && stats.hunger > 0.0 && stats.thirst < FULL) {
        println!("phase 4 FAIL: survival stats not draining: {stats:?}");
        return 1;
    }
    println!(
        "phase 4 pass: survival draining (hunger {:.3}, thirst {:.3})",
        stats.hunger, stats.thirst
    );

    println!("boot test PASS");
    0
}
