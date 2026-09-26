//! Hunger and thirst (modforge survival): they fall at their rates for
//! everyone, and eating from the inventory raises them.

use crate::common;

use common::{act, alone, game};
use modforge::hud::{Holder, SlotRef};
use modforge::item::create;
use topside::actor::{Inventory, Survival};
use topside::combat::Health;
use topside::content::{GAME_DAY_SECS, ItemRegistryRes};
use topside::input::Action;

#[test]
fn an_empty_stomach_costs_health_every_second() {
    let (mut app, player) = game();
    alone(&mut app);
    app.world_mut().get_mut::<Survival>(player).unwrap().0.hunger = 0.0;
    let before = app.world().get::<Health>(player).unwrap().0.current;
    // Three seconds.
    for _ in 0..180 {
        app.update();
    }
    let after = app.world().get::<Health>(player).unwrap().0.current;
    assert!(before - after >= 4.0 && before - after <= 8.0, "lost {} in three seconds", before - after);
}

#[test]
fn a_game_hour_drains_hunger_and_thirst_and_a_can_brings_hunger_back() {
    let (mut app, player) = game();
    alone(&mut app);
    let before = app.world().get::<Survival>(player).unwrap().0;
    // One game hour: a day is GAME_DAY_SECS, at 60 ticks a second.
    let hour_ticks = (GAME_DAY_SECS / 24.0 * 60.0) as usize;
    for _ in 0..hour_ticks {
        app.update();
    }
    let after = app.world().get::<Survival>(player).unwrap().0;
    let hunger_lost = before.hunger - after.hunger;
    let thirst_lost = before.thirst - after.thirst;
    assert!(hunger_lost > 0.0, "hunger fell");
    // A canteen lasts half as long as a stomach: thirst falls twice as fast.
    assert!((thirst_lost - 2.0 * hunger_lost).abs() < 0.05 * thirst_lost, "{thirst_lost} vs {hunger_lost}");

    let can = {
        let items = app.world().resource::<ItemRegistryRes>();
        create(items.0.def("canned food").unwrap(), 1, &[], 0.0, 0)
    };
    let _ = app.world_mut().get_mut::<Inventory>(player).unwrap().0.add(can, 10);
    let slot = SlotRef {
        holder: Holder::Inventory,
        index: 0,
    };
    act(&mut app, player, &[Action::UseSlot { slot }]);
    let fed = app.world().get::<Survival>(player).unwrap().0;
    assert!(fed.hunger > after.hunger, "the can was eaten");
    assert_eq!(app.world().get::<Inventory>(player).unwrap().0.count_of("canned food"), 0);
}
