//! Newcomers (todo 11i, operator 2026-10-02): word of a place reaches the
//! edge when someone who knows it tells people of another group, and a
//! group of 2 to 4 then rolls at the edge and walks to the place, at most
//! one group a game hour. modforge `edge` is the faucet (where, how many,
//! when); here the people are spawned through `spawn_actor` and sent there.

use bevy::prelude::*;

use crate::actor::{Asked, SpawnActor, spawn_actor};
use crate::content::ActorRegistryRes;

/// The edge's state (modforge `edge::Edge`): saved, so word on its way and
/// when the next group may come survive a load.
#[derive(Resource, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct EdgeRes(pub modforge::edge::Edge);

/// Word of a place told to someone of another group this tick (talk::talk):
/// the place's key, what it is, where it is, and where it was told. Taken
/// the same tick by `newcomers`.
#[derive(Resource, Default)]
pub struct WordToEdge(pub Vec<(u64, String, Vec3, Vec2)>);

/// Who comes in from the edge: someone of no group.
const NEWCOMER: &str = "wanderer";

/// Every tick: word told this tick sets out for the edge, reaching it as
/// long after as walking from where it was told to the edge takes (todo
/// 11i: Claude's reading, for the operator to change); a group due there
/// rolls, 2 to 4 people of no group, each asked to the place (the brain's
/// "do what was asked"), so they walk to it.
#[allow(clippy::too_many_arguments)]
pub fn newcomers(
    tick: Res<crate::SimTick>,
    seed: Res<crate::WorldSeed>,
    mut edge: ResMut<EdgeRes>,
    mut told: ResMut<WordToEdge>,
    monuments: Option<Res<crate::structure::MonumentsRes>>,
    registry: Res<ActorRegistryRes>,
    mut spawn: SpawnActor,
) {
    let places: Vec<(f32, f32)> = monuments.as_ref().map_or(Vec::new(), |m| m.0.iter().map(|(at, _)| (at.x, at.y)).collect());
    for (key, kind, place, from) in std::mem::take(&mut told.0) {
        let roll = seed.0 ^ tick.0.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ key;
        let Some(edge_at) = modforge::edge::edge_point((0.0, 0.0), &places, roll) else {
            continue;
        };
        let walk = from.distance(Vec2::new(edge_at.0, edge_at.1)) / crate::player::WALK_SPEED;
        let reaches_at = tick.0 + (walk * 60.0).ceil() as u64;
        edge.0.told(modforge::edge::Word { key, kind, place: (place.x, place.y, place.z), edge: edge_at, reaches_at });
    }
    let every = (modforge::edge::GROUP_EVERY_SECS * 60.0) as u64;
    let Some(word) = edge.0.due(tick.0, every) else {
        return;
    };
    let Some(def) = registry.0.def(NEWCOMER) else {
        return;
    };
    let count = modforge::edge::group_size(seed.0 ^ tick.0 ^ word.key);
    let place = Vec3::new(word.place.0, word.place.1, word.place.2);
    info!("newcomers: {count} at the edge {:?}, for the place {place}", word.edge);
    for i in 0..count {
        // Side by side, a step apart.
        let at = crate::on_ground(Vec2::new(word.edge.0, word.edge.1) + Vec2::new(i as f32 * 1.5, 0.0), 0.0);
        if let Some(person) = spawn_actor(&mut spawn, def, at, None) {
            // They know the place they heard of, and go to it.
            let (key, kind, now) = (word.key, word.kind.clone(), tick.0);
            spawn.commands.entity(person).insert(Asked(place)).queue(move |mut e: EntityWorldMut| {
                if let Some(mut memory) = e.get_mut::<crate::actor::Memory>() {
                    memory.0.see(key, &kind, place, now);
                }
            });
        }
    }
}
