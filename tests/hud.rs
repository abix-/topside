//! The hotbar and inventory through modforge's hud: T takes a stack out
//! of an open box, a drag moves a stack into the hotbar, a number key
//! holds it, and melee swings what is held.

mod common;

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use common::{act, game};
use modforge::hud::{Holder, SlotRef};
use topside::actor::{Hotbar, Inventory, Npc};
use topside::combat::{Equipment, Firing, Health, Protection};
use topside::hud::{HoveredSlot, drag_drop, drag_start};
use topside::input::Action;
use topside::interact::{Container, OpenContainer};
use topside::terrain::WorldRes;

fn put(app: &mut App, who: Entity, at: Vec2) {
    let mut t = app.world_mut().get_mut::<Transform>(who).unwrap();
    t.translation = at.extend(t.translation.z);
}

#[test]
fn t_takes_a_dead_npcs_weapon_out_of_its_box() {
    let (mut app, player) = game();
    let world = app.world_mut();
    let npc = world.query_filtered::<Entity, With<Npc>>().iter(world).next().unwrap();
    let fell_at = app.world().get::<Transform>(npc).unwrap().translation.truncate();
    app.world_mut().get_mut::<Health>(npc).unwrap().0.current = 0.0;
    app.update();
    put(&mut app, player, fell_at + Vec2::new(0.1, 0.0));
    act(&mut app, player, &[Action::Use]);
    let open = app.world().resource::<OpenContainer>().0.expect("the box is open");
    let (slot, weapon) = app
        .world()
        .get::<Container>(open)
        .unwrap()
        .0
        .slots
        .iter()
        .enumerate()
        .find_map(|(i, s)| s.as_ref().map(|s| (i, s.item.clone())))
        .expect("the box holds something");
    app.world_mut().resource_mut::<HoveredSlot>().0 = Some(SlotRef {
        holder: Holder::Container,
        index: slot,
    });
    act(&mut app, player, &[Action::Transfer { half: false }]);
    assert_eq!(app.world().get::<Inventory>(player).unwrap().0.count_of(&weapon), 1, "{weapon} taken");
}

#[test]
fn the_pipe_dragged_to_the_hotbar_and_held_with_1_swings() {
    let (mut app, player) = game();
    let bunker = app.world().resource::<WorldRes>().0.def.bunker;
    // Pick the pipe up off the bunker floor.
    put(&mut app, player, bunker + Vec2::new(1.0, 3.2));
    act(&mut app, player, &[Action::Use]);
    let index = app
        .world()
        .get::<Inventory>(player)
        .unwrap()
        .0
        .slots
        .iter()
        .position(|s| s.as_ref().is_some_and(|s| s.item == "pipe"))
        .expect("the pipe is in the inventory");
    // Drag it onto the first hotbar slot, then press 1.
    let from = SlotRef {
        holder: Holder::Inventory,
        index,
    };
    let to = SlotRef {
        holder: Holder::Hotbar,
        index: 0,
    };
    app.world_mut().run_system_once_with(drag_start, from).unwrap();
    app.world_mut().run_system_once_with(drag_drop, to).unwrap();
    act(&mut app, player, &[Action::Hotbar { index: 0 }]);
    let held = app.world().get::<Hotbar>(player).unwrap().0.held_stack().map(|s| s.item.clone());
    assert_eq!(held.as_deref(), Some("pipe"), "the pipe is in the hands");

    // Out in the open, swing it at someone.
    let at = bunker + Vec2::new(20.0, 0.0);
    put(&mut app, player, at);
    let target = app
        .world_mut()
        .spawn((
            Transform::from_translation((at + Vec2::new(1.2, 0.0)).extend(0.9)),
            Firing::default(),
            Protection::default(),
            Equipment::default(),
            Health::default(),
        ))
        .id();
    act(&mut app, player, &[Action::Aim { x: at.x + 10.0, y: at.y }, Action::Attack]);
    assert!(app.world().get::<Health>(target).unwrap().0.current < 100.0, "the held pipe hit");
}
