//! Melee (combat.md "Melee: the four moves"): the attack plus the move
//! held, read relative to facing, picks the swing; the swing's shape
//! decides who is hit; a block stops it; the beat chains; a swing
//! commits.

mod common;

use bevy::prelude::*;
use common::{act, game, out_in_the_open, position};
use modforge::combat::Swing;
use modforge::item::{EquipSlot, create};
use topside::combat::{Equipment, Firing, Health, Protection};
use topside::content::ItemRegistryRes;
use topside::input::Action;

/// A person to hit: health, armor, and a fire timer, no brain.
fn dummy(app: &mut App, at: Vec2) -> Entity {
    app.world_mut()
        .spawn((
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

/// The player in the open, the pipe in its hands.
fn armed() -> (App, Entity, Vec2) {
    let (mut app, player) = game();
    out_in_the_open(&mut app, player);
    let pipe = {
        let items = app.world().resource::<ItemRegistryRes>();
        create(items.0.def("pipe").unwrap(), 1, &[], 0.0, 0)
    };
    app.world_mut()
        .get_mut::<Equipment>(player)
        .unwrap()
        .0
        .set(EquipSlot::Weapon, Some(pipe));
    let at = position(&app, player);
    (app, player, at)
}

/// Aim along +x (east) from `at`.
fn east(at: Vec2) -> Action {
    Action::Aim { x: at.x + 10.0, y: at.y }
}

#[test]
fn the_overhead_hits_the_strip_ahead_and_misses_one_beside() {
    let (mut app, player, at) = armed();
    let ahead = dummy(&mut app, at + Vec2::new(1.2, 0.0));
    let beside = dummy(&mut app, at + Vec2::new(0.6, 1.1));
    act(&mut app, player, &[east(at), Action::Attack]);
    assert!(health(&app, ahead) < 100.0, "the one ahead is hit");
    assert_eq!(health(&app, beside), 100.0, "the one beside is missed");
    // The overhead stepped in.
    assert!(position(&app, player).x > at.x + 0.3, "stepped in");
}

#[test]
fn a_side_sweep_hits_the_one_beside() {
    let (mut app, player, at) = armed();
    let beside = dummy(&mut app, at + Vec2::new(0.6, 1.1));
    // Facing east, W (up the screen) is to the left: a left sweep.
    act(&mut app, player, &[east(at), Action::Move { x: 0.0, y: 1.0 }, Action::Attack]);
    assert_eq!(app.world().get::<Firing>(player).unwrap().0.swing, Swing::Left);
    assert!(health(&app, beside) < 100.0);
}

#[test]
fn a_block_stops_a_swing() {
    let (mut app, player, at) = armed();
    let guard = dummy(&mut app, at + Vec2::new(1.2, 0.0));
    app.world_mut()
        .get_mut::<Firing>(guard)
        .unwrap()
        .0
        .pull_melee(0.6, Swing::Block, true);
    act(&mut app, player, &[east(at), Action::Attack]);
    assert_eq!(health(&app, guard), 100.0, "the block held");
}

#[test]
fn a_press_on_the_beat_chains_without_a_pause() {
    let (mut app, player, at) = armed();
    let target = dummy(&mut app, at + Vec2::new(1.5, 0.0));
    act(&mut app, player, &[east(at), Action::Attack]);
    let one = health(&app, target);
    assert!(one < 100.0);
    // Let go, then press again in the last third of the pipe's 0.6 s:
    // 28 ticks in, under 0.2 s is left.
    for _ in 0..27 {
        act(&mut app, player, &[east(at)]);
    }
    act(&mut app, player, &[east(at), Action::Attack]);
    assert!(health(&app, target) < one, "the chained swing landed at once");
}

#[test]
fn a_swing_commits_the_person_until_it_ends() {
    let (mut app, player, at) = armed();
    // A block, so nothing steps in, then walk while it holds.
    act(&mut app, player, &[east(at), Action::Move { x: -1.0, y: 0.0 }, Action::Attack]);
    assert!(app.world().get::<Firing>(player).unwrap().0.committed());
    let held = position(&app, player);
    for _ in 0..20 {
        act(&mut app, player, &[Action::Move { x: 0.0, y: 1.0 }]);
    }
    assert_eq!(position(&app, player), held, "no walking mid block");
}
