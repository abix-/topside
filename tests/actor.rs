//! NPCs enter through the one spawn path at every monument's NPC
//! spots, and think with modforge's brain: a hostile hunter that sees
//! the player walks toward it through the same action queue the
//! player's keys fill.

mod common;

use bevy::prelude::*;
use common::game;
use modforge::actor::Behaviour;
use topside::actor::{Home, Npc};
use topside::content::{ActorRegistryRes, FactionRegistryRes, PLAYER_FACTION, def_for_danger};
use topside::structure::MonumentsRes;

#[test]
fn every_npc_spot_gets_its_person() {
    let (mut app, _) = game();
    let spots: usize = app
        .world()
        .resource::<MonumentsRes>()
        .0
        .iter()
        .map(|(_, m)| m.npc_spots.len())
        .sum();
    assert!(spots > 0, "the world has NPC spots");
    let world = app.world_mut();
    let npcs = world.query_filtered::<(), (With<Npc>, With<Home>)>().iter(world).count();
    assert_eq!(npcs, spots);
}

#[test]
fn a_hostile_hunter_walks_toward_the_player_it_sees() {
    let (mut app, player) = game();
    // A hunter hostile to the player.
    let hunter_def = ["scavenger", "raider"]
        .into_iter()
        .chain([def_for_danger(0), def_for_danger(5)])
        .find(|name| {
            let world = app.world();
            let def = world.resource::<ActorRegistryRes>().0.def(name).unwrap();
            def.behaviour == Behaviour::Hunter
                && world
                    .resource::<FactionRegistryRes>()
                    .0
                    .hostile(def.faction.as_deref(), Some(PLAYER_FACTION))
        })
        .expect("a hostile hunter is registered");
    let world = app.world_mut();
    let hunter = world
        .query::<(Entity, &Npc)>()
        .iter(world)
        .find(|(_, npc)| npc.def == hunter_def)
        .map(|(e, _)| e)
        .expect("one stands at some monument");
    // Both on open ground by the bunker, 8 m apart.
    let bunker = app.world().resource::<topside::terrain::WorldRes>().0.def.bunker;
    let start = bunker + Vec2::new(20.0, 0.0);
    app.world_mut().get_mut::<Transform>(hunter).unwrap().translation = start.extend(0.9);
    app.world_mut().get_mut::<Transform>(player).unwrap().translation = (start + Vec2::new(8.0, 0.0)).extend(1.0);
    for _ in 0..60 {
        app.update();
    }
    let hunter_at = app.world().get::<Transform>(hunter).unwrap().translation.truncate();
    let player_at = app.world().get::<Transform>(player).unwrap().translation.truncate();
    let gap = hunter_at.distance(player_at);
    assert!(gap < 6.0, "the hunter closed in: {gap} m apart");
}
