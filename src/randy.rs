//! Randy Random's encounters (todo 10h1; design.md "The unknown" rule 1,
//! "The first hour" rule 4): while the player walks topside, modforge
//! `randy` rolls from content's table; what it hits happens here, its
//! people through `spawn_actor`.

use bevy::prelude::*;

use crate::actor::{Asked, SpawnActor, spawn_actor};
use crate::content::ActorRegistryRes;

/// How far the player has walked toward the next roll, and where they stood
/// last tick: saved, so a load rolls the same.
#[derive(Resource, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct RandyRes {
    pub walking: modforge::randy::Walking,
    pub last: Option<Vec2>,
}

/// Who crosses the player's path: someone of no group.
const STRANGER: &str = "wanderer";

/// More than this in one tick is not walking (a load, being put somewhere),
/// and counts for nothing (Claude's, for the operator to change).
const MOST_IN_A_TICK: f32 = 1.0;

/// Where the stranger starts: this far ahead of the player, and this far to
/// one side; and how far past the path, on the other side, they go
/// (Claude's numbers, for the operator to change).
const AHEAD: f32 = 25.0;
const SIDE: f32 = 40.0;
const PAST: f32 = 80.0;

/// Every tick after the player wakes: the ground walked out in the open
/// (under no roof) goes to Randy Random; an encounter it hits happens.
#[allow(clippy::too_many_arguments)]
pub fn encounters(
    seed: Res<crate::WorldSeed>,
    mut randy: ResMut<RandyRes>,
    player: Query<&Transform, With<crate::player::Player>>,
    shelters: Query<&crate::structure::Shelter>,
    registry: Res<ActorRegistryRes>,
    (nav, archipelagos): (Res<crate::navmesh::Nav>, Query<&bevy_landmass::prelude::Archipelago3d>),
    mut spawn: SpawnActor,
) {
    let Ok(player) = player.single() else {
        return;
    };
    let now = player.translation.xz();
    let Some(last) = randy.last.replace(now) else {
        return;
    };
    let step = now - last;
    if step.length() > MOST_IN_A_TICK || crate::structure::sheltered(&shelters, now) {
        return;
    }
    let table = crate::content::encounter_table();
    let RandyRes { walking, .. } = &mut *randy;
    let Some(hit) = table.walked(walking, step.length(), seed.0) else {
        return;
    };
    let rolls = walking.rolls;
    info!("randy: roll {rolls} hit {}", hit.name);
    if hit.name != crate::content::STRANGER_CROSSES {
        return;
    }
    // Across the way the player walks: from one side, ahead of them, to
    // well past the other side; which side from the same roll.
    let ahead = step.normalize_or_zero();
    let side = ahead.perp() * if modforge::roll::salted_index(seed.0 ^ rolls, 0x5349_4445, 2) == 0 { 1.0 } else { -1.0 };
    let archipelago = archipelagos.get(nav.archipelago).ok();
    let stand = |at: Vec2| archipelago.and_then(|a| crate::navmesh::on_mesh(a, crate::on_ground(at, 0.0)));
    let (Some(from), Some(to)) = (stand(now + ahead * AHEAD + side * SIDE), stand(now + ahead * AHEAD - side * PAST)) else {
        warn!("randy: nowhere to stand across the player's path at {now}; the stranger does not come");
        return;
    };
    let Some(def) = registry.0.def(STRANGER) else {
        return;
    };
    if let Some(stranger) = spawn_actor(&mut spawn, def, from, None) {
        spawn.commands.entity(stranger).insert(Asked(to));
        info!("randy: a stranger crosses the player's path, from {from} to {to}; the player at {now}");
    }
}
