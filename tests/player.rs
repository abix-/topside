//! The player walks through the one input path: actions pushed onto
//! its queue, drained each fixed tick, moved by `movement`, stopped by
//! Avian colliders.

use crate::common;

use avian2d::prelude::*;
use bevy::prelude::*;
use common::{alone, game, hold, out_in_the_open, position};
use topside::SimTick;
use topside::actor::Facing;
use topside::input::Action;
use topside::player::{PLAYER_SIZE, WALK_SPEED};

const W: Action = Action::Move { x: 0.0, y: 1.0 };

#[test]
fn a_wall_stops_the_player_at_its_edge() {
    let (mut app, player) = game();
    alone(&mut app);
    out_in_the_open(&mut app, player);
    let start = position(&app, player);
    // A wall 3 m north, one metre thick, across the way.
    let wall_bottom = start.y + 3.0;
    app.world_mut().spawn((
        RigidBody::Static,
        Collider::rectangle(10.0, 1.0),
        Transform::from_xyz(start.x, wall_bottom + 0.5, 0.0),
    ));
    app.update();
    hold(&mut app, player, W, 120);
    let end = position(&app, player);
    let edge = wall_bottom - PLAYER_SIZE / 2.0;
    assert!(end.y <= edge + 0.01, "walked through the wall: {} past {}", end.y, edge);
    assert!(end.y > edge - 0.2, "stopped short of the wall: {} before {}", end.y, edge);
}

#[test]
fn the_player_faces_the_point_it_aims_at() {
    let (mut app, player) = game();
    alone(&mut app);
    let at = position(&app, player);
    let target = at + Vec2::new(-3.0, 4.0);
    hold(&mut app, player, Action::Aim { x: target.x, y: target.y }, 1);
    let facing = app.world().get::<Facing>(player).unwrap().direction();
    let want = (target - at).normalize();
    assert!(facing.distance(want) < 1e-4, "faces {facing}, wants {want}");
}

#[test]
fn pushed_move_actions_walk_the_player() {
    let (mut app, player) = game();
    alone(&mut app);
    out_in_the_open(&mut app, player);
    let start = position(&app, player);
    let ticks_before = app.world().resource::<SimTick>().0;
    hold(&mut app, player, W, 60);
    let end = position(&app, player);
    let walked = end.y - start.y;
    let ticks = (app.world().resource::<SimTick>().0 - ticks_before) as f32;
    assert!(walked > 0.0, "W moves the player up the screen");
    assert_eq!(end.x, start.x, "W does not move sideways");
    assert!(
        (walked - WALK_SPEED * ticks / 60.0).abs() < 0.2,
        "walked {walked} m in {ticks} ticks"
    );
}
