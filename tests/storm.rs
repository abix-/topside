//! The storm (design.md "The storm"): hide or die, then the world
//! shifts to another reality; the bunker and every person stay.

mod common;

use bevy::prelude::*;
use common::{alone, game, out_in_the_open};
use topside::actor::Npc;
use topside::combat::Health;
use topside::content::STORM;
use topside::interact::WorldItem;
use topside::storm::StormRes;
use topside::structure::StructureRoot;
use topside::terrain::WorldPiece;
use topside::{SimTick, WorldSeed};

/// Put the clock at `secs` since the world began.
fn at_secs(app: &mut App, secs: f32) {
    app.world_mut().resource_mut::<SimTick>().0 = (secs * 60.0) as u64;
}

fn storm_damage_taken(outside: bool) -> f32 {
    let (mut app, player) = game();
    alone(&mut app);
    if outside {
        out_in_the_open(&mut app, player);
    }
    // One second into the first storm.
    at_secs(&mut app, STORM.every_secs + 1.0);
    for _ in 0..180 {
        app.update();
    }
    100.0 - app.world().get::<Health>(player).unwrap().0.current
}

#[test]
fn the_storm_hurts_in_the_open_and_not_in_the_bunker() {
    assert!(storm_damage_taken(true) > 0.0, "caught in the open");
    assert_eq!(storm_damage_taken(false), 0.0, "safe under the bunker's roof");
}

#[test]
fn when_the_storm_passes_the_world_shifts_but_the_bunker_and_the_people_stay() {
    let (mut app, _) = game();
    let count = |app: &mut App| {
        let world = app.world_mut();
        let npcs = world.query_filtered::<(), With<Npc>>().iter(world).count();
        let pipes = world.query::<&WorldItem>().iter(world).filter(|w| w.stack.item == "pipe").count();
        let bunkers = world
            .query_filtered::<(), (With<StructureRoot>, Without<WorldPiece>)>()
            .iter(world)
            .count();
        (npcs, pipes, bunkers)
    };
    let before = count(&mut app);
    let seed_before = app.world().resource::<WorldSeed>().0;
    // The last tick of the first storm; the next one ends it.
    at_secs(&mut app, STORM.every_secs + STORM.storm_secs - 1.0 / 60.0);
    app.update();
    app.update();
    assert_eq!(app.world().resource::<StormRes>().shifts, 1, "the world shifted once");
    assert_ne!(app.world().resource::<WorldSeed>().0, seed_before, "from a new seed");
    let world = app.world_mut();
    let pieces = world.query_filtered::<(), With<WorldPiece>>().iter(world).count();
    assert!(pieces > 0, "the new reality stands");
    assert_eq!(count(&mut app), before, "the same people, the bunker, its pipe");
}
