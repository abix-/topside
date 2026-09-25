//! Every hit on an actor goes through the one damage path: worn armor
//! (one total) takes its share, health takes the rest, and at zero the
//! actor dies.

mod common;

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use common::game;
use modforge::combat::Hit;
use modforge::item::{EquipSlot, create};
use topside::combat::{Equipment, Health, Protection, hit_actor};
use topside::content::{DamageRegistryRes, ItemRegistryRes};

/// One pipe swing on `target`; returns the damage it dealt.
fn swing(app: &mut App, target: Entity) -> f32 {
    app.world_mut()
        .run_system_once(
            move |damage: Res<DamageRegistryRes>,
                  items: Res<ItemRegistryRes>,
                  mut actors: Query<(&Protection, &Equipment, &mut Health)>| {
                let hit = Hit {
                    def: damage.0.def("pipe swing").expect("the pipe swing is registered"),
                    self_inflicted: false,
                    distance: 1.0,
                    location_scale: 1.0,
                };
                let (own, worn, mut health) = actors.get_mut(target).unwrap();
                hit_actor(&hit, own, worn, &items.0, &mut health).damage_dealt
            },
        )
        .unwrap()
}

#[test]
fn armor_takes_its_share_and_zero_health_kills() {
    let (mut app, _) = game();
    // Someone who is not the player (the player wakes in the bunker).
    let player = app
        .world_mut()
        .spawn((
            Transform::default(),
            Protection::default(),
            Equipment::default(),
            Health::default(),
        ))
        .id();
    let bare = swing(&mut app, player);
    assert!(bare > 0.0);
    let after_one = app.world().get::<Health>(player).unwrap().0.current;
    assert_eq!(after_one, 100.0 - bare);

    // Wear the vest: the same swing deals less.
    let vest = {
        let items = app.world().resource::<ItemRegistryRes>();
        let def = items.0.def("road sign vest").unwrap();
        create(def, 1, &[], 0.0, 0)
    };
    app.world_mut()
        .get_mut::<Equipment>(player)
        .unwrap()
        .0
        .set(EquipSlot::Chest, Some(vest));
    let guarded = swing(&mut app, player);
    assert!(guarded < bare, "the vest takes a share: {guarded} vs {bare}");

    // Swing until dead; the next step takes the body away.
    while app.world().get::<Health>(player).unwrap().0.current > 0.0 {
        swing(&mut app, player);
    }
    app.update();
    assert!(app.world().get_entity(player).is_err(), "the dead are gone");
}
