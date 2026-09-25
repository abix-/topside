//! The roll (combat.md "The roll"): about 0.7 s, safe from damage for
//! the first half, the direction locked once started.

mod common;

use bevy::prelude::*;
use common::{act, alone, game, out_in_the_open, position};
use topside::combat::{Bullet, Health};
use topside::input::Action;

const UP: Action = Action::Move { x: 0.0, y: 1.0 };

/// A bullet 1 m above the player flying straight down at them, from a
/// shooter who is not there.
fn bullet_at(app: &mut App, player: Entity) {
    let shooter = app.world_mut().spawn_empty().id();
    let at = position(app, player) + Vec2::new(0.0, 1.0);
    app.world_mut().spawn((
        Bullet {
            shooter,
            damage: "shotgun pellet".to_string(),
            direction: Vec2::NEG_Y,
            travelled: 0.0,
            range: 40.0,
        },
        Transform::from_translation(at.extend(1.5)),
    ));
}

fn health(app: &App, e: Entity) -> f32 {
    app.world().get::<Health>(e).unwrap().0.current
}

fn setup() -> (App, Entity) {
    let (mut app, player) = game();
    alone(&mut app);
    out_in_the_open(&mut app, player);
    (app, player)
}

#[test]
fn a_bullet_passes_through_the_first_half_of_a_roll() {
    let (mut app, player) = setup();
    act(&mut app, player, &[UP, Action::Roll]);
    bullet_at(&mut app, player);
    for _ in 0..5 {
        act(&mut app, player, &[UP]);
    }
    assert_eq!(health(&app, player), 100.0, "rolled through it untouched");
}

#[test]
fn a_bullet_lands_in_the_second_half_of_a_roll() {
    let (mut app, player) = setup();
    act(&mut app, player, &[UP, Action::Roll]);
    // Past the halfway mark (0.35 s is 21 ticks).
    for _ in 0..24 {
        act(&mut app, player, &[UP]);
    }
    bullet_at(&mut app, player);
    for _ in 0..3 {
        act(&mut app, player, &[UP]);
    }
    assert!(health(&app, player) < 100.0, "open in the second half");
}

#[test]
fn a_rolls_direction_holds_whatever_is_pressed() {
    let (mut app, player) = setup();
    let start = position(&app, player);
    act(&mut app, player, &[UP, Action::Roll]);
    for _ in 0..20 {
        act(&mut app, player, &[Action::Move { x: 1.0, y: 0.0 }]);
    }
    let end = position(&app, player);
    assert!(end.y > start.y + 1.0, "it rolled up");
    assert!((end.x - start.x).abs() < 1e-3, "the right key did not turn it");
}
