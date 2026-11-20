//! The turret (todo 11k; authority.md "Actor": "a turret is an actor
//! with speed zero placed from the inventory"). Anyone holding a turret
//! strikes (Attack) to set it down a step ahead: it comes into the world
//! through `spawn_actor` with the turret's `ActorDef` (it guards, its
//! gun a pipe shotgun), takes the placer's faction, and is loaded with
//! every shell the placer carries. It never moves; it shoots whoever it
//! stands hostile to until its shells run out, is hit like anyone, and
//! dies through the one `die`, dropping its box. Everyone is equal: any
//! person can set one.

use bevy::prelude::*;
use modforge::actions::Action;

use crate::actor::{Dead, Hotbar, Inventory, Npc};
use crate::content::{ActorRegistryRes, PLAYER_FACTION, TURRET, TURRET_AMMO};
use crate::input::ThisTick;
use crate::{Ground, on_ground};

/// How far ahead a turret is set, metres (Claude's number).
const AHEAD: f32 = 1.5;

/// A turret: it never moves. `load` is the shells still to put in it.
#[derive(Component, Clone, Copy, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Turret {
    pub load: u32,
}

/// Every step: a turret struck down from the hand is set a step ahead.
#[allow(clippy::type_complexity)]
pub fn place(
    mut spawn: crate::actor::SpawnActor,
    registry: Res<ActorRegistryRes>,
    mut people: Query<(&ThisTick, &Transform, &crate::actor::Facing, &mut Inventory, &mut Hotbar, Option<&Npc>, Has<crate::player::Player>), Without<Dead>>,
) {
    for (this_tick, at, facing, mut inventory, mut hotbar, npc, is_player) in &mut people {
        if !this_tick.0.iter().any(|s| s.action == Action::Attack) || hotbar.0.held_stack().is_none_or(|s| s.item != TURRET) {
            continue;
        }
        let Some(def) = registry.0.def(TURRET) else {
            continue;
        };
        let Some(index) = hotbar.0.held else {
            continue;
        };
        let feet = crate::interact::feet_of(at);
        let spot = on_ground(at.translation.ground() + facing.direction() * AHEAD, feet.y);
        let faction = if is_player { Some(PLAYER_FACTION.to_string()) } else { npc.and_then(|n| n.faction.clone()) };
        let Some(turret) = crate::actor::spawn_actor(&mut spawn, def, spot, faction.as_deref()) else {
            continue;
        };
        hotbar.0.slots.remove(index, 1);
        // Every shell carried goes into it.
        let mut load = 0;
        while inventory.0.take_one(TURRET_AMMO) || hotbar.0.slots.take_one(TURRET_AMMO) {
            load += 1;
        }
        spawn.commands.entity(turret).insert(Turret { load });
        info!("a turret set at {spot:?}, {load} shells");
    }
}

/// A turret set this step is loaded with its shells.
pub fn load(items: Res<crate::content::ItemRegistryRes>, mut turrets: Query<(&mut Turret, &mut Inventory), Added<Turret>>) {
    let Some(def) = items.0.def(TURRET_AMMO) else {
        return;
    };
    for (mut turret, mut inventory) in &mut turrets {
        if turret.load > 0 {
            let stack = modforge::item::create(def, turret.load, &[], 0.0, 0);
            let _ = inventory.0.add(stack, def.max_stack);
            turret.load = 0;
        }
    }
}

/// Every step, before anyone moves: a turret's moves are dropped; it
/// turns and fires where it stands.
pub fn stay(mut turrets: Query<&mut ThisTick, With<Turret>>) {
    for mut this_tick in &mut turrets {
        this_tick.0.retain(|s| !matches!(s.action, Action::Move { .. } | Action::Go { .. } | Action::Roll));
    }
}
