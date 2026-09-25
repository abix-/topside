//! The headless game every test drives: 60 fixed ticks a second, one
//! tick per update, the world rolled, the player in the bunker.

#![allow(dead_code)]

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
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

/// Put the player on open ground 20 m east of the bunker, inside the
/// bunker's clear ground where nothing is scattered.
pub fn out_in_the_open(app: &mut App, player: Entity) {
    app.world_mut().get_mut::<Transform>(player).unwrap().translation.x += 20.0;
    app.update();
}

/// Push `action` for each of the next `steps` fixed ticks.
pub fn hold(app: &mut App, player: Entity, action: Action, steps: usize) {
    for _ in 0..steps {
        let next = app.world().resource::<SimTick>().0 + 1;
        app.world_mut().get_mut::<Actions>(player).unwrap().0.push(next, action);
        app.update();
    }
}

pub fn position(app: &App, player: Entity) -> Vec2 {
    app.world().get::<Transform>(player).unwrap().translation.truncate()
}
