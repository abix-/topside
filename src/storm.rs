//! The storm (design.md "The storm"), on modforge's clock: the sky
//! warns, the storm hits and anyone caught in the open takes the
//! storm's damage through the one hit path, player and NPC alike; when
//! it passes, the world shifts to another reality. The ground, what
//! stands on it, and the things lying about are rolled anew from a new
//! seed through the same spawn paths; the bunker and every person stay.

use bevy::prelude::*;
use modforge::combat::Hit;
use modforge::storm::StormPhase;

use crate::actor::Dead;
use crate::combat::{Equipment, Health, Protection, hit_actor};
use crate::content::{DamageRegistryRes, ItemRegistryRes, STORM};
use crate::interact::WorldItem;
use crate::structure::{Shelter, sheltered};
use crate::terrain::WorldPiece;
use crate::{SimTick, WorldSeed};

/// How many times the world has shifted.
#[derive(Resource, Default)]
pub struct StormRes {
    pub shifts: u32,
}

/// The game draws a window (the ground needs one to be drawn).
#[derive(Resource)]
pub struct Windowed;

/// Where the world is in the storm's cycle now.
pub fn phase(tick: &SimTick) -> StormPhase {
    STORM.phase(tick.0 as f32 / 60.0)
}

/// Every fixed step: during the storm, once a second, everyone in the
/// open takes the storm's damage; when a storm has passed, the world
/// shifts.
pub fn storm(
    mut commands: Commands,
    tick: Res<SimTick>,
    items: Res<ItemRegistryRes>,
    damage: Res<DamageRegistryRes>,
    mut state: ResMut<StormRes>,
    shelters: Query<&Shelter>,
    mut actors: Query<(&Transform, &Protection, &Equipment, &mut Health), Without<Dead>>,
) {
    if phase(&tick) == StormPhase::Storm
        && tick.0 % 60 == 0
        && let Some(def) = damage.0.def("storm")
    {
        for (at, own, worn, mut health) in &mut actors {
            if sheltered(&shelters, at.translation.truncate()) {
                continue;
            }
            let hit = Hit {
                def,
                self_inflicted: false,
                distance: 0.0,
                location_scale: 1.0,
            };
            hit_actor(&hit, own, worn, &items.0, &mut health);
        }
    }
    let passed = STORM.passed(tick.0 as f32 / 60.0);
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
    windowed: Option<Res<Windowed>>,
    pieces: Query<Entity, With<WorldPiece>>,
    lying: Query<(Entity, &Transform), With<WorldItem>>,
    bunker: Query<&Shelter, Without<WorldPiece>>,
) {
    for entity in &pieces {
        commands.entity(entity).despawn();
    }
    for (entity, at) in &lying {
        let point = at.translation.truncate();
        if !bunker.iter().any(|s| s.covers(point)) {
            commands.entity(entity).despawn();
        }
    }
    seed.0 = seed.0.rotate_left(17) ^ (u64::from(state.shifts)).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    commands.run_system_cached(crate::terrain::roll);
    commands.run_system_cached(crate::terrain::spawn_solids);
    commands.run_system_cached(crate::structure::spawn_monuments);
    if windowed.is_some() {
        commands.run_system_cached(crate::terrain::spawn_ground);
    }
}
