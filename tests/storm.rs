//! The storm (design.md "The storm"): hide or die, then the world
//! shifts to another reality; the bunkers and the bunker people stay,
//! the topside people die with their world.

mod common;

use bevy::prelude::*;
use common::{alone, game, out_in_the_open};
use topside::actor::{Dead, Home, Npc};
use topside::combat::Health;
use topside::content::{OTHER_BUNKERS, STORM};
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

/// The world shifts: the bunkers, the bunker people, and the bunker's
/// pipe stay; every topside person dies with the old world and the new
/// world's are found at its monuments (design.md "Bunker people and
/// topside people").
#[test]
fn when_the_storm_passes_the_world_shifts_but_the_bunkers_and_their_people_stay() {
    let (mut app, _) = game();
    let bunker_people = |app: &mut App| {
        let world = app.world_mut();
        let mut people: Vec<Entity> = world.query_filtered::<Entity, (With<Npc>, With<Home>)>().iter(world).collect();
        people.sort();
        people
    };
    let topside_people = |app: &mut App| {
        let world = app.world_mut();
        world.query_filtered::<Entity, (With<Npc>, Without<Home>)>().iter(world).collect::<Vec<_>>()
    };
    let count = |app: &mut App| {
        let world = app.world_mut();
        let pipes = world.query::<&WorldItem>().iter(world).filter(|w| w.stack.item == "pipe").count();
        let bunkers = world
            .query_filtered::<(), (With<StructureRoot>, Without<WorldPiece>)>()
            .iter(world)
            .count();
        (pipes, bunkers)
    };
    let before = count(&mut app);
    assert_eq!(before.1, 1 + OTHER_BUNKERS.len(), "the player's bunker and the others");
    let people_before = bunker_people(&mut app);
    let topside_before = topside_people(&mut app);
    assert!(!people_before.is_empty() && !topside_before.is_empty());
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
    assert_eq!(count(&mut app), before, "the bunkers and the bunker's pipe");
    assert_eq!(bunker_people(&mut app), people_before, "the same bunker people");
    let topside_after = topside_people(&mut app);
    assert!(!topside_after.is_empty(), "the new world has its topside people");
    assert!(
        topside_before.iter().all(|e| app.world().get_entity(*e).is_err()),
        "every topside person of the old world is dead"
    );
}

/// A bunker person caught outside by the storm dies there and respawns
/// at its bunker; one at home is safe under the roof.
#[test]
fn a_bunker_person_caught_outside_dies_and_respawns_at_home() {
    let (mut app, _) = game();
    let world = app.world_mut();
    let mut people: Vec<(Entity, Vec2)> = world.query::<(Entity, &Home)>().iter(world).map(|(e, h)| (e, h.0)).collect();
    people.sort_by_key(|(e, _)| *e);
    let (caught, home) = people[0];
    let (safe, _) = people[1];
    // Twenty metres outside its bunker, one hit from death.
    app.world_mut().get_mut::<Transform>(caught).unwrap().translation = (home + Vec2::new(20.0, 0.0)).extend(0.9);
    app.world_mut().get_mut::<Health>(caught).unwrap().0.current = 1.0;
    let safe_health = app.world().get::<Health>(safe).unwrap().0.current;
    at_secs(&mut app, STORM.every_secs + 1.0);
    let mut died = false;
    for _ in 0..180 {
        app.update();
        died |= app.world().get::<Dead>(caught).is_some();
    }
    assert!(died, "caught outside, it died");
    assert!(app.world().get::<Dead>(caught).is_none(), "and respawned");
    let at = app.world().get::<Transform>(caught).unwrap().translation.truncate();
    assert!(at.distance(home) < 1.0, "at home, not where it fell: {at}");
    assert_eq!(app.world().get::<Health>(safe).unwrap().0.current, safe_health, "the one at home is unhurt");
}
