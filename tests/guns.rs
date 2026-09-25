//! Guns (combat.md "Guns"): every shot is a visible bullet that flies
//! over several ticks, falls off with distance, stops at its range, and
//! uses a round.

mod common;

use avian2d::prelude::*;
use bevy::prelude::*;
use common::{act, alone, game, out_in_the_open, position};
use modforge::item::create;
use topside::actor::{Hotbar, Inventory};
use topside::combat::{Bullet, Equipment, Firing, Health, Protection};
use topside::content::ItemRegistryRes;
use topside::input::Action;

/// The player in the open with the pipe shotgun held and 8 shells.
fn gunner() -> (App, Entity, Vec2) {
    let (mut app, player) = game();
    alone(&mut app);
    out_in_the_open(&mut app, player);
    let (gun, shells) = {
        let items = app.world().resource::<ItemRegistryRes>();
        (
            create(items.0.def("pipe shotgun").unwrap(), 1, &[], 0.0, 0),
            create(items.0.def("shotgun shell").unwrap(), 8, &[], 0.0, 1),
        )
    };
    {
        let mut hotbar = app.world_mut().get_mut::<Hotbar>(player).unwrap();
        hotbar.0.slots.slots[0] = Some(gun);
        hotbar.0.held = Some(0);
    }
    let _ = app.world_mut().get_mut::<Inventory>(player).unwrap().0.add(shells, 20);
    let at = position(&app, player);
    (app, player, at)
}

/// Someone to shoot: a body, health, armor, no brain.
fn target(app: &mut App, at: Vec2) -> Entity {
    app.world_mut()
        .spawn((
            RigidBody::Static,
            Collider::circle(0.4),
            Transform::from_translation(at.extend(0.9)),
            Firing::default(),
            Protection::default(),
            Equipment::default(),
            Health::default(),
        ))
        .id()
}

fn health(app: &App, e: Entity) -> f32 {
    app.world().get::<Health>(e).unwrap().0.current
}

/// One shot east, then `ticks` more steps.
fn shoot_east(app: &mut App, player: Entity, at: Vec2, ticks: usize) {
    act(app, player, &[Action::Aim { x: at.x + 100.0, y: at.y }, Action::Attack]);
    for _ in 0..ticks {
        act(app, player, &[Action::Aim { x: at.x + 100.0, y: at.y }]);
    }
}

#[test]
fn a_shot_flies_over_several_ticks_hits_and_uses_a_shell() {
    let (mut app, player, at) = gunner();
    let t = target(&mut app, at + Vec2::new(10.0, 0.0));
    app.update();
    shoot_east(&mut app, player, at, 2);
    assert!(app.world_mut().query::<&Bullet>().iter(app.world()).count() > 0, "bullets in the air");
    assert_eq!(health(&app, t), 100.0, "not there yet after two ticks");
    for _ in 0..20 {
        app.update();
    }
    assert!(health(&app, t) < 100.0, "the pellets landed");
    assert_eq!(app.world().get::<Inventory>(player).unwrap().0.count_of("shotgun shell"), 7);
}

#[test]
fn damage_falls_off_with_distance() {
    let dealt = |distance: f32| {
        let (mut app, player, at) = gunner();
        let t = target(&mut app, at + Vec2::new(distance, 0.0));
        app.update();
        shoot_east(&mut app, player, at, 40);
        100.0 - health(&app, t)
    };
    let near = dealt(3.0);
    let far = dealt(25.0);
    assert!(near > 0.0);
    assert!(far < near, "far {far} is less than near {near}");
}

#[test]
fn nothing_lands_past_the_range() {
    let (mut app, player, at) = gunner();
    // The pipe shotgun reaches 40 m.
    let t = target(&mut app, at + Vec2::new(45.0, 0.0));
    app.update();
    shoot_east(&mut app, player, at, 90);
    assert_eq!(health(&app, t), 100.0);
    assert_eq!(app.world_mut().query::<&Bullet>().iter(app.world()).count(), 0, "every bullet ended");
}
