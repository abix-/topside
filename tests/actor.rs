//! NPCs enter through the one spawn path at every monument's NPC
//! spots, and think with modforge's brain: a hostile hunter that sees
//! the player walks toward it through the same action queue the
//! player's keys fill.

use crate::common;

use bevy::prelude::*;
use common::{at_a_site, game};
use modforge::actor::Behaviour;
use topside::actor::{Home, Npc};
use topside::content::{
    ActorRegistryRes, BUNKER_PEOPLE, FactionRegistryRes, OTHER_BUNKERS, PLAYER_FACTION, bunker_faction,
    def_for_danger,
};
use topside::structure::MonumentsRes;

#[test]
fn every_npc_spot_gets_its_person() {
    let (mut app, player) = game();
    at_a_site(&mut app, player);
    let spots: usize = app
        .world()
        .resource::<MonumentsRes>()
        .0
        .iter()
        .map(|(_, m)| m.npc_spots.len())
        .sum();
    assert!(spots > 0, "the world has NPC spots");
    // Topside people have no home: one per spot, and the scavenger at
    // the door.
    let world = app.world_mut();
    let topside = world.query_filtered::<(), (With<Npc>, Without<Home>)>().iter(world).count();
    assert_eq!(topside, spots + 1);
}

/// Every other bunker is its own faction with its bunker people living
/// in it (design.md "Bunker people and topside people").
#[test]
fn every_other_bunker_has_its_people() {
    let (mut app, _) = game();
    let world = app.world_mut();
    let people: Vec<(Npc, Vec2, Vec2)> = world
        .query::<(&Npc, &Home, &Transform)>()
        .iter(world)
        .map(|(n, h, t)| (n.clone(), h.0, t.translation.truncate()))
        .collect();
    assert_eq!(people.len(), OTHER_BUNKERS.len() * BUNKER_PEOPLE);
    for (i, bunker) in OTHER_BUNKERS.iter().enumerate() {
        let faction = bunker_faction(i);
        let theirs: Vec<_> = people.iter().filter(|(n, _, _)| n.faction.as_deref() == Some(&faction)).collect();
        assert_eq!(theirs.len(), BUNKER_PEOPLE, "{faction} has its people");
        for (_, home, at) in theirs {
            assert!(home.distance(*bunker) < 5.0, "{faction}'s home is its bunker");
            assert!(at.distance(*home) < 0.1, "{faction}'s people start at home");
        }
    }
}

#[test]
fn a_scavenger_waits_outside_the_bunker_door() {
    let (mut app, _) = game();
    let (name, [x, z]) = topside::content::DOOR_ENEMY;
    let bunker = app.world().resource::<topside::terrain::WorldRes>().bunker;
    let spot = bunker + Vec2::new(x, z);
    let world = app.world_mut();
    let found = world
        .query::<(&Npc, &Transform)>()
        .iter(world)
        .any(|(npc, t)| npc.def == name && t.translation.truncate().distance(spot) < 1.0);
    assert!(found, "a {name} stands outside the door");
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
    let bunker = app.world().resource::<topside::terrain::WorldRes>().bunker;
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
