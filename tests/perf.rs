//! The fixed step's cost, measured on the full world with every NPC
//! (profile.rs laps): the whole step must fit in one sixtieth of a
//! second, or the game cannot keep 60 steps a second. The table of
//! every part, costliest first, is in the message either way.

mod common;

use bevy::prelude::*;
use common::game;
use topside::actor::Npc;
use topside::profile::Laps;

/// One fixed step at 60 a second, in ms.
const BUDGET_MS: f64 = 1000.0 / 60.0;

#[test]
fn the_fixed_step_fits_its_budget() {
    let (mut app, _) = game();
    for _ in 0..300 {
        app.update();
    }
    let world = app.world_mut();
    let npcs = world.query_filtered::<(), With<Npc>>().iter(world).count();
    let laps = app.world().resource::<Laps>();
    let table: Vec<String> = laps.sorted().iter().map(|(n, ms)| format!("{ms:>9.3} ms  {n}")).collect();
    let total = laps.step_ms;
    let report = format!("{npcs} NPCs, fixed step {total:.3} ms of {BUDGET_MS:.3}\n{}", table.join("\n"));
    eprintln!("{report}");
    // Readable after a passing run too (build.ps1 shows only results).
    let _ = std::fs::create_dir_all("logs");
    let _ = std::fs::write("logs/perf.txt", &report);
    assert!(total < BUDGET_MS, "{report}");
}
