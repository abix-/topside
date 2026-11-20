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

/// More than this in one tick is not walking (a load, being put somewhere),
/// and counts for nothing (Claude's, for the operator to change).
const MOST_IN_A_TICK: f32 = 1.0;

/// People who cross the way the player walks: who, how many, where they
/// start (this far ahead of the player and this far to one side), how far
/// past the path on the other side they go, and how far apart they walk.
struct Crossing {
    encounter: &'static str,
    who: &'static str,
    count: usize,
    ahead: f32,
    side: f32,
    past: f32,
    apart: f32,
}

/// Every encounter that is people crossing (Claude's numbers, for the
/// operator to change): a stranger of no group, close; a band of raiders
/// far off, in the distance (todo 10h2).
const CROSSINGS: [Crossing; 2] = [
    Crossing {
        encounter: crate::content::STRANGER_CROSSES,
        who: "wanderer",
        count: 1,
        ahead: 25.0,
        side: 40.0,
        past: 80.0,
        apart: 0.0,
    },
    Crossing {
        encounter: crate::content::BAND_PASSES,
        who: "raider",
        count: 4,
        ahead: 70.0,
        side: 90.0,
        past: 180.0,
        apart: 3.0,
    },
];

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
    let Some(crossing) = CROSSINGS.iter().find(|c| c.encounter == hit.name) else {
        return;
    };
    // Across the way the player walks: from one side, ahead of them, to
    // well past the other side; which side from the same roll. A group
    // walks in a line along the player's way, `apart` between each.
    let ahead = step.normalize_or_zero();
    let side = ahead.perp() * if modforge::roll::salted_index(seed.0 ^ rolls, 0x5349_4445, 2) == 0 { 1.0 } else { -1.0 };
    let archipelago = archipelagos.get(nav.archipelago).ok();
    let stand = |at: Vec2| archipelago.and_then(|a| crate::navmesh::on_mesh(a, crate::on_ground(at, 0.0)));
    let Some(def) = registry.0.def(crossing.who) else {
        return;
    };
    for k in 0..crossing.count {
        let along = now + ahead * (crossing.ahead + k as f32 * crossing.apart);
        let (Some(from), Some(to)) = (stand(along + side * crossing.side), stand(along - side * crossing.past)) else {
            warn!("randy: nowhere to stand across the player's path at {now}; {} does not come", hit.name);
            continue;
        };
        if let Some(one) = spawn_actor(&mut spawn, def, from, None) {
            spawn.commands.entity(one).insert(Asked(to));
            info!("randy: {}: {} from {from} to {to}; the player at {now}", hit.name, crossing.who);
        }
    }
}
