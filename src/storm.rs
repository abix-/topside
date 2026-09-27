//! The storm (design.md "The storm"), on modforge's clock: the sky
//! warns, the storm hits and anyone caught in the open takes the
//! storm's damage through the one hit path, player and NPC alike; when
//! it passes, the world shifts to another reality. The ground, what
//! stands on it, and the things lying about are rolled anew from a new
//! seed through the same spawn paths. The bunkers and the bunker people
//! stay; the topside people die with their world and the next world's
//! are found at its monuments.

use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;
use modforge::combat::Hit;
use modforge::storm::StormPhase;

use crate::Ground;
use crate::actor::{Actor, ActorMapRes, Dead, Home, Memory, Npc};
use crate::combat::leave;
use crate::combat::{Equipment, Health, Protection, hit_actor};
use crate::content::{DamageRegistryRes, EpisodeRegistryRes, ItemRegistryRes, LayerRegistryRes, RealityRegistryRes};
use crate::survival::Day;
use crate::interact::WorldItem;
use crate::structure::{Shelter, sheltered};
use crate::terrain::WorldPiece;
use crate::{Episode, Reality, SimTick, WorldSeed};

/// How many times the world has shifted, and how far the storm's clock
/// runs ahead of the world's (the `storm` op brings the next storm now by
/// running it ahead).
#[derive(Resource, Default)]
pub struct StormRes {
    pub shifts: u32,
    pub ahead: f32,
}

impl StormRes {
    /// Seconds on the storm's clock at `tick`.
    pub fn secs(&self, tick: &SimTick) -> f32 {
        Day::secs(tick) + self.ahead
    }
}

/// The game draws a window (the ground needs one to be drawn).
#[derive(Resource)]
pub struct Windowed;

/// Where the world is in the storm's cycle now, on the storm's clock.
pub fn phase(tick: &SimTick, day: Day, storms: &StormRes) -> StormPhase {
    day.storm().phase(storms.secs(tick))
}

/// The `storm` op's request to bring the next storm now, taken by the
/// game on its next step.
pub static STORM_NOW: AtomicBool = AtomicBool::new(false);

/// Every fixed step: the `storm` op's request runs the storm's clock
/// ahead to the next warning; during the storm, once a second, everyone
/// in the open takes the storm's damage; when a storm has passed, the
/// world shifts.
pub fn storm(
    mut commands: Commands,
    tick: Res<SimTick>,
    day: Res<Day>,
    items: Res<ItemRegistryRes>,
    layers: Res<LayerRegistryRes>,
    damage: Res<DamageRegistryRes>,
    mut state: ResMut<StormRes>,
    shelters: Query<&Shelter>,
    mut actors: Query<(&Transform, &Protection, &Equipment, &mut Health, Option<&mut Memory>), Without<Dead>>,
) {
    if STORM_NOW.swap(false, Ordering::Relaxed) {
        state.ahead += day.storm().until_next_warning(state.secs(&tick));
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
            hit_actor(&hit, own, worn, &items.0, &layers.0, &mut health, memory.as_deref_mut(), None, tick.0);
        }
    }
    let passed = day.storm().passed(state.secs(&tick));
    if passed > state.shifts {
        state.shifts = passed;
        commands.run_system_cached(shift_world);
    }
}

/// The world shifts to another reality: everything of this one goes
/// (the ground, what stands on it, things lying outside the bunker),
/// then the next rolls from a new seed through the same paths that
/// built the first.
pub fn shift_world(
    mut commands: Commands,
    state: Res<StormRes>,
    mut seed: ResMut<WorldSeed>,
    pieces: Query<Entity, With<WorldPiece>>,
    lying: Query<(Entity, &Transform), With<WorldItem>>,
    bunker: Query<&Shelter, Without<WorldPiece>>,
    mut topside_people: Query<(Entity, Option<&Actor>, Option<&mut Memory>), (With<Npc>, Without<Home>, Without<Dead>)>,
    mut map: ResMut<ActorMapRes>,
    tick: Res<SimTick>,
    realities: Res<RealityRegistryRes>,
    mut reality: ResMut<Reality>,
    episodes: Res<EpisodeRegistryRes>,
    mut episode: ResMut<Episode>,
) {
    // The storm hands over another reality (realities.md), picked from
    // the seed and the storm's number, never the one being left.
    if let Some(next) = realities.0.pick(seed.0, state.shifts, &reality.0.name) {
        info!("the storm hands over {} after storm {}", next.name, state.shifts);
        reality.0 = next.clone();
    }
    // Then the storyteller starts an episode that fits it (episodes.md).
    episode.0 = episodes.0.pick(&reality.0.name, seed.0, state.shifts).map(|e| e.name.clone());
    info!("episode: {:?}", episode.0);
    for entity in &pieces {
        commands.entity(entity).despawn();
    }
    // The topside people are part of this world and die with it
    // (design.md "Bunker people and topside people"), the storm's death in
    // their memory like any other; they stay, dead, to query.
    for (entity, actor, memory) in &mut topside_people {
        if let Some(mut memory) = memory {
            memory.0.did(modforge::memory::Did::Died(None, "storm".to_string()), tick.0);
            memory.0.learned.died(tick.0);
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
