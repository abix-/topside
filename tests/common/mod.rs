//! The headless game every test drives: 60 fixed ticks a second, one
//! tick per update, the world rolled, the player in the bunker.

#![allow(dead_code)]

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use topside::actor::Npc;
use topside::input::{Action, Actions};
use topside::player::Player;
use topside::{SimTick, build_app};

pub fn game() -> (App, Entity) {
    let mut app = build_app(true, 1);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 60.0)));
    // Startup rolls the world, builds the bunker, and spawns the player.
    app.update();
    let player = app
        .world_mut()
        .query_filtered::<Entity, With<Player>>()
        .single(app.world())
        .expect("one player");
    (app, player)
}

/// Take every NPC out of the world, for a test about something other
/// than a fight (the scavenger at the door would come in).
pub fn alone(app: &mut App) {
    let world = app.world_mut();
    let npcs: Vec<Entity> = world.query_filtered::<Entity, With<Npc>>().iter(world).collect();
    for npc in npcs {
        world.despawn(npc);
    }
}

/// Put the player on open ground 20 m east of the bunker, inside the
/// bunker's clear ground where nothing is scattered.
pub fn out_in_the_open(app: &mut App, player: Entity) {
    app.world_mut().get_mut::<Transform>(player).unwrap().translation.x += 20.0;
    app.update();
}

/// Take the player to the nearest site with buildings and step, so its
/// chunk is made and the site spawned with its boxes and people, as in
/// the game. Returns where the site stands.
pub fn at_a_site(app: &mut App, player: Entity) -> Vec2 {
    let world = app.world_mut();
    let site = world.resource_scope(|world, mut res: Mut<topside::terrain::WorldRes>| {
        let biomes = world.resource::<topside::content::BiomeRegistryRes>();
        let monuments = world.resource::<topside::content::MonumentRegistryRes>();
        let r = modforge::world2d::Registries {
            biomes: &biomes.0,
            monuments: &monuments.monuments,
            buildings: &monuments.buildings,
        };
        let far = Vec2::splat(2000.0);
        let mut sites = res.world.sites_in(r, -far, far);
        sites.retain(|s| !s.rolled.members.is_empty());
        sites.sort_by(|a, b| a.position.length().total_cmp(&b.position.length()));
        sites.first().expect("a site with buildings within 2000 m").position
    });
    let mut t = world.get_mut::<Transform>(player).unwrap();
    t.translation.x = site.x;
    t.translation.y = site.y;
    // Chunks and sites, then the physics world has their colliders.
    app.update();
    app.update();
    site
}

/// Push `action` for each of the next `steps` fixed ticks.
pub fn hold(app: &mut App, player: Entity, action: Action, steps: usize) {
    for _ in 0..steps {
        let next = app.world().resource::<SimTick>().0 + 1;
        app.world_mut().get_mut::<Actions>(player).unwrap().0.push(next, action);
        app.update();
    }
}

/// Push every action in `actions` for the next fixed tick, then step
/// once.
pub fn act(app: &mut App, player: Entity, actions: &[Action]) {
    let next = app.world().resource::<SimTick>().0 + 1;
    let mut queue = app.world_mut().get_mut::<Actions>(player).unwrap();
    for action in actions {
        queue.0.push(next, *action);
    }
    app.update();
}

pub fn position(app: &App, player: Entity) -> Vec2 {
    app.world().get::<Transform>(player).unwrap().translation.truncate()
}
