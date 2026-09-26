//! The bunker's crafting bench (todo 10), healing from food and a
//! bandage (todo 10ca), and night tiring the awake (todo 11e4).

use crate::common;

use bevy::prelude::*;
use common::{act, alone, game};
use modforge::crafting::StationKind;
use modforge::hud::{Holder, SlotRef};
use modforge::item::create;
use topside::SimTick;
use topside::actor::{Inventory, Survival};
use topside::combat::Health;
use topside::content::{BENCH_AT, GAME_DAY_SECS, ItemRegistryRes, RecipeRegistryRes};
use topside::input::Action;
use topside::interact::OpenStation;
use topside::terrain::WorldRes;

fn give(app: &mut App, player: Entity, item: &str, count: u32) {
    let stack = {
        let items = app.world().resource::<ItemRegistryRes>();
        create(items.0.def(item).unwrap(), count, &[], 0.0, 0)
    };
    let _ = app.world_mut().get_mut::<Inventory>(player).unwrap().0.add(stack, 20);
}

fn slot_of(app: &App, player: Entity, item: &str) -> SlotRef {
    let index = app
        .world()
        .get::<Inventory>(player)
        .unwrap()
        .0
        .slots
        .iter()
        .position(|s| s.as_ref().is_some_and(|s| s.item == item))
        .unwrap();
    SlotRef {
        holder: Holder::Inventory,
        index,
    }
}

#[test]
fn e_at_the_bench_opens_it_and_cloth_becomes_a_bandage() {
    let (mut app, player) = game();
    alone(&mut app);
    let bunker = app.world().resource::<WorldRes>().bunker;
    let [x, z] = BENCH_AT;
    let mut t = app.world_mut().get_mut::<Transform>(player).unwrap();
    t.translation = (bunker + Vec2::new(x, z - 0.6)).extend(t.translation.z);
    act(&mut app, player, &[Action::Use]);
    assert_eq!(app.world().get::<OpenStation>(player).unwrap().0, Some(StationKind::CraftingBench));

    give(&mut app, player, "cloth", 2);
    // A click on the bandage row crafts it.
    let index = app
        .world()
        .resource::<RecipeRegistryRes>()
        .0
        .for_station(StationKind::CraftingBench)
        .iter()
        .position(|r| r.name == "bandage")
        .expect("the bench makes bandages");
    act(&mut app, player, &[Action::Craft { index }]);
    let inventory = &app.world().get::<Inventory>(player).unwrap().0;
    assert_eq!(inventory.count_of("bandage"), 1);
    assert_eq!(inventory.count_of("cloth"), 0, "the cloth went into it");
}

#[test]
fn a_bandage_heals_more_than_a_can() {
    let heal = |item: &str| {
        let (mut app, player) = game();
        alone(&mut app);
        app.world_mut().get_mut::<Health>(player).unwrap().0.current = 40.0;
        give(&mut app, player, item, 1);
        let slot = slot_of(&app, player, item);
        act(&mut app, player, &[Action::UseSlot { slot }]);
        app.world().get::<Health>(player).unwrap().0.current - 40.0
    };
    let can = heal("canned food");
    let bandage = heal("bandage");
    assert!(can > 0.0, "food heals a little");
    assert!(bandage > can, "a bandage heals more: {bandage} vs {can}");
}

#[test]
fn night_tires_the_awake_faster() {
    let rest_lost = |at_night: bool| {
        let (mut app, player) = game();
        alone(&mut app);
        if at_night {
            // Half a day on from morning is night.
            app.world_mut().resource_mut::<SimTick>().0 = (GAME_DAY_SECS / 2.0 * 60.0) as u64;
        }
        let before = app.world().get::<Survival>(player).unwrap().0.rest;
        for _ in 0..600 {
            app.update();
        }
        before - app.world().get::<Survival>(player).unwrap().0.rest
    };
    let day = rest_lost(false);
    let night = rest_lost(true);
    assert!(night > 3.0 * day, "night {night} vs day {day}");
}
