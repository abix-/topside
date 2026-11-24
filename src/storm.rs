//! The storm (design.md "The storm"), on modforge's clock: the sky
//! warns, the storm hits and anyone caught in the open takes the
//! storm's damage through the one hit path, player and NPC alike; when
//! it passes, the world shifts to another reality. The ground, what
//! stands on it, and the things lying about are rolled anew from a new
//! seed through the same spawn paths. The bunkers and the bunker people
//! stay; the topside people die with their world and the next world's
//! are found at its monuments.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;
use modforge::combat::Hit;
use modforge::storm::StormPhase;

use crate::Ground;
use crate::actor::{Actor, ActorMapRes, Dead, Gone, Home, Npc, Persona};
use crate::combat::leave;
use crate::combat::{Equipment, Health, Protection, hit_actor};
use crate::content::{DamageRegistryRes, EpisodeRegistryRes, ItemRegistryRes, LayerRegistryRes, RealityRegistryRes};
use crate::survival::Day;
use crate::interact::WorldItem;
use crate::structure::{Shelter, sheltered};
use crate::terrain::WorldPiece;
use crate::{Episode, Reality, SimTick, WorldSeed};

/// How many times the world has shifted, and the second (since the world
/// began) the next storm hits: rolled from the running episode's
/// `storm_in` when it starts (design.md "The session rhythm"), none until
/// then.
#[derive(Resource, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct StormRes {
    pub shifts: u32,
    #[serde(default)]
    pub at: Option<f32>,
    /// The reality the `storm` op named for the next shift; none leaves
    /// the pick to the registry. Saved, so a loaded save shifts the same.
    #[serde(default)]
    pub reality: Option<String>,
}

/// With no episode running (none fits the reality), the storm comes a game
/// day after the last.
const NO_EPISODE_STORM_IN: (f32, f32) = (1.0, 1.0);

/// The game draws a window (the ground needs one to be drawn).
#[derive(Resource)]
pub struct Windowed;

/// Where the world is in the storm's coming now.
pub fn phase(tick: &SimTick, day: Day, storms: &StormRes) -> StormPhase {
    storms.at.map_or(StormPhase::Calm, |at| day.storm().phase(Day::secs(tick), at))
}

/// The `storm` op's request to bring the next storm now, taken by the
/// game on its next step.
pub static STORM_NOW: AtomicBool = AtomicBool::new(false);

/// The reality the `storm` op named, taken into `StormRes` on the game's
/// next step.
pub static NEXT_REALITY: Mutex<Option<String>> = Mutex::new(None);

/// Every fixed step: with no storm set, the running episode's is rolled
/// (`storm_in` game days after it started, from the seed and the storm's
/// number); the `storm` op's request brings it now, the sky warning from
/// this step; during the storm, once a second, everyone in the open takes
/// the storm's damage; when it has passed, the world shifts.
#[allow(clippy::too_many_arguments)]
pub fn storm(
    mut commands: Commands,
    tick: Res<SimTick>,
    day: Res<Day>,
    items: Res<ItemRegistryRes>,
    layers: Res<LayerRegistryRes>,
    damage: Res<DamageRegistryRes>,
    mut state: ResMut<StormRes>,
    (episode, episodes, seed): (Res<Episode>, Res<EpisodeRegistryRes>, Res<WorldSeed>),
    shelters: Query<&Shelter>,
    mut actors: Query<(&Transform, &Protection, &Equipment, &mut Health, Option<&mut Persona>), Without<Dead>>,
) {
    let secs = Day::secs(&tick);
    if state.at.is_none() {
        let days = episode.name.as_deref().and_then(|n| episodes.0.def(n)).map_or(NO_EPISODE_STORM_IN, |d| d.storm_in);
        let from = episode.started as f32 / 60.0;
        state.at = Some(modforge::storm::storm_at(from, days, day.0, seed.0 ^ u64::from(state.shifts)));
    }
    if let Some(named) = NEXT_REALITY.lock().ok().and_then(|mut n| n.take()) {
        state.reality = Some(named);
    }
    if STORM_NOW.swap(false, Ordering::Relaxed) && phase(&tick, *day, &state) == StormPhase::Calm {
        state.at = Some(day.storm().now(secs));
    }
    if phase(&tick, *day, &state) == StormPhase::Storm
        && tick.0 % 60 == 0
        && let Some(def) = damage.0.def("storm")
    {
        for (at, own, worn, mut health, mut memory) in &mut actors {
            if sheltered(&shelters, at.translation.ground()) {
                continue;
            }
            let hit = Hit {
                def,
                self_inflicted: false,
                distance: 0.0,
                location_scale: 1.0,
            };
            hit_actor(&hit, own, worn, &items.0, &layers.0, Default::default(), &mut health, memory.as_deref_mut(), None, tick.0);
        }
    }
    if state.at.is_some_and(|at| day.storm().passed(secs, at)) {
        state.shifts += 1;
        // The next episode's storm is rolled once it has started.
        state.at = None;
        commands.run_system_cached(shift_world);
    }
}

/// The world shifts to another reality: everything of this one goes
/// (the ground, what stands on it, things lying outside the bunker),
/// then the next rolls from a new seed through the same paths that
/// built the first.
pub fn shift_world(
    mut commands: Commands,
    mut state: ResMut<StormRes>,
    mut seed: ResMut<WorldSeed>,
    pieces: Query<Entity, With<WorldPiece>>,
    lying: Query<(Entity, &Transform), With<WorldItem>>,
    bunker: Query<&Shelter, Without<WorldPiece>>,
    mut topside_people: Query<(Entity, Option<&Actor>, Option<&mut Persona>), (With<Npc>, Without<Home>, Without<Gone>)>,
    mut map: ResMut<ActorMapRes>,
    tick: Res<SimTick>,
    realities: Res<RealityRegistryRes>,
    mut reality: ResMut<Reality>,
    episodes: Res<EpisodeRegistryRes>,
    mut episode: ResMut<Episode>,
) {
    // The storm hands over another reality (realities.md), picked from
    // the seed and the storm's number, never the one being left.
    // The reality the `storm` op named, if any; else the registry's pick.
    let named = state.reality.take();
    let next = match named.as_deref().and_then(|name| realities.0.def(name)) {
        Some(def) => Some(def),
        None => realities.0.pick(seed.0, state.shifts, &reality.0.name),
    };
    if let Some(next) = next {
        info!("the storm hands over {} after storm {}", next.name, state.shifts);
        reality.0 = next.clone();
    }
    // Then the storyteller starts an episode that fits it (episodes.md).
    episode.start(episodes.0.pick(&reality.0.name, seed.0, state.shifts).map(|e| e.name.clone()), tick.0);
    info!("episode: {:?}", episode.name);
    for entity in &pieces {
        commands.entity(entity).despawn();
    }
    // Each episode everything topside dies, and only bunker people come
    // back (operator, 2026-09-27): the topside people die with this world
    // (design.md "Bunker people and topside people"), those waiting to
    // wake in Loop included, the storm's death in their memory like any
    // other; they stay, gone for good, to query.
    for (entity, actor, memory) in &mut topside_people {
        if let Some(mut memory) = memory {
            memory.0.memory.did(modforge::memory::Did::Died(None, "storm".to_string()), tick.0);
            memory.0.memory.learned.died(tick.0);
        }
        leave(&mut commands, &mut map, entity, actor);
    }
    for (entity, at) in &lying {
        let point = at.translation.ground();
        if !bunker.iter().any(|s| s.covers(point)) {
            commands.entity(entity).despawn();
        }
    }
    seed.0 = seed.0.rotate_left(17) ^ (u64::from(state.shifts)).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    // The new world is made chunk by chunk from the next step on, as the
    // first was.
    commands.run_system_cached(crate::terrain::roll);
}
