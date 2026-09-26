//! How many living people the world can hold (life.md: a thousand is
//! the target). The same world is filled to each size through the one
//! settling path (a home at a monument, its boxes known), everyone with
//! different needs so they go and eat, drink, and sleep; ten seconds
//! run; the fixed step's cost and what the people did are written to
//! logs/scale.txt. People must eat at every size: proof they live.

use crate::common;

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use common::{at_a_site, game};
use topside::SimTick;
use topside::actor::{Npc, SpawnActor, Survival, home_knowledge, settle};
use topside::ai::LifeStats;
use topside::content::ActorRegistryRes;
use topside::interact::{Container, WorldItem};
use topside::profile::Laps;
use topside::structure::MonumentsRes;

/// The world filled to `total` people, run ten seconds: people, the
/// step's cost in ms, its three costliest parts, what they did.
fn run_with(total: usize) -> (usize, f64, String, LifeStats) {
    let (mut app, player) = game();
    at_a_site(&mut app, player);
    let world = app.world_mut();
    let have = world.query_filtered::<(), With<Npc>>().iter(world).count();
    let extra = total.saturating_sub(have);
    app.world_mut()
        .run_system_once(
            move |mut spawn: SpawnActor,
                  registry: Res<ActorRegistryRes>,
                  monuments: Res<MonumentsRes>,
                  tick: Res<SimTick>,
                  boxes: Query<(Entity, &Transform, &WorldItem), With<Container>>| {
                let def = registry.0.def("wanderer").unwrap().clone();
                let n = monuments.0.len();
                for k in 0..extra {
                    let i = k % n;
                    let (at, monument) = &monuments.0[i];
                    let known = home_knowledge(i, *at, &monument.kind, &boxes, tick.0);
                    let jitter = Vec2::new(((k * 37) % 11) as f32 - 5.0, ((k * 53) % 11) as f32 - 5.0);
                    settle(&mut spawn, &def, *at + jitter, &known);
                }
            },
        )
        .unwrap();
    app.update();
    // Everyone with different needs, some pressing.
    let world = app.world_mut();
    let mut people = world.query_filtered::<&mut Survival, With<Npc>>();
    for (k, mut needs) in people.iter_mut(world).enumerate() {
        needs.0.hunger = ((k * 7919) % 80 + 20) as f32;
        needs.0.thirst = ((k * 104_729) % 80 + 20) as f32;
        needs.0.rest = ((k * 1_299_709) % 80 + 20) as f32;
    }
    for _ in 0..600 {
        app.update();
    }
    let world = app.world_mut();
    let alive = world.query_filtered::<(), With<Npc>>().iter(world).count();
    let laps = app.world().resource::<Laps>();
    let step = laps.step_ms;
    let top: Vec<String> = laps.sorted().iter().take(3).map(|(n, ms)| format!("{n} {ms:.1}")).collect();
    (alive, step, top.join(", "), *app.world().resource::<LifeStats>())
}

#[test]
fn how_many_can_live() {
    let mut rows = vec![format!(
        "{:>6} {:>9}  {:<38} {:>5} {:>6} {:>5} {:>6} {:>5}",
        "people", "step ms", "costliest", "ate", "looked", "slept", "fights", "died"
    )];
    let mut all_ate = true;
    for total in [250, 500, 1000, 2000] {
        let (alive, step, top, life) = run_with(total);
        all_ate &= life.ate > 0;
        rows.push(format!(
            "{alive:>6} {step:>9.2}  {top:<38} {:>5} {:>6} {:>5} {:>6} {:>5}",
            life.ate, life.looked, life.slept, life.fights, life.died
        ));
        // Written after every size, so a slow run still leaves numbers.
        let _ = std::fs::create_dir_all("../logs");
        let _ = std::fs::write(
            "../logs/scale.txt",
            format!("budget {:.2} ms a step\n{}\n", 1000.0 / 60.0, rows.join("\n")),
        );
    }
    eprintln!("{}", rows.join("\n"));
    assert!(all_ate, "people ate at every size:\n{}", rows.join("\n"));
}
