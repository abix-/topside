//! The player walks through the one input path: actions pushed onto
//! its queue, drained each fixed tick, moved by `movement`.

use std::time::Duration;

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use topside::input::{Action, Actions};
use topside::player::{PLAYER_SIZE, Player, WALK_SPEED};
use topside::{SimTick, build_app};

/// A headless game at 60 fixed ticks a second, one tick per update,
/// with the world rolled and the player at the bunker.
fn game() -> (App, Entity) {
    let mut app = build_app(true, 1);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 60.0)));
    // Startup rolls the world and spawns the player at the bunker.
    app.update();
    let player = app
        .world_mut()
        .query_filtered::<Entity, With<Player>>()
        .single(app.world())
        .expect("one player");
    (app, player)
}

/// Hold W for `steps` fixed ticks: one move pushed for each next tick.
fn hold_w(app: &mut App, player: Entity, steps: usize) {
    for _ in 0..steps {
        let next = app.world().resource::<SimTick>().0 + 1;
        app.world_mut()
            .get_mut::<Actions>(player)
            .unwrap()
            .0
            .push(next, Action::Move { x: 0.0, y: 1.0 });
        app.update();
    }
}

#[test]
fn a_wall_stops_the_player_at_its_edge() {
    let (mut app, player) = game();
    let start = app.world().get::<Transform>(player).unwrap().translation;
    // A wall 3 m north, one metre thick, across the way.
    let wall_bottom = start.y + 3.0;
    app.world_mut().spawn((
        RigidBody::Static,
        Collider::rectangle(10.0, 1.0),
        Transform::from_xyz(start.x, wall_bottom + 0.5, 0.0),
    ));
    app.update();
    hold_w(&mut app, player, 120);
    let end = app.world().get::<Transform>(player).unwrap().translation;
    let edge = wall_bottom - PLAYER_SIZE / 2.0;
    assert!(end.y <= edge + 0.01, "walked through the wall: {} past {}", end.y, edge);
    assert!(end.y > edge - 0.2, "stopped short of the wall: {} before {}", end.y, edge);
}

#[test]
fn pushed_move_actions_walk_the_player() {
    let (mut app, player) = game();
    let start = app.world().get::<Transform>(player).unwrap().translation;
    let ticks_before = app.world().resource::<SimTick>().0;
    hold_w(&mut app, player, 60);
    let end = app.world().get::<Transform>(player).unwrap().translation;
    let walked = end.y - start.y;
    let ticks = (app.world().resource::<SimTick>().0 - ticks_before) as f32;
    assert!(walked > 0.0, "W moves the player up the screen");
    assert_eq!(end.x, start.x, "W does not move sideways");
    assert!(
        (walked - WALK_SPEED * ticks / 60.0).abs() < 0.2,
        "walked {walked} m in {ticks} ticks"
    );
}
