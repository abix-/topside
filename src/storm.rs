//! The storm (design.md "The storm"), on modforge's clock: the sky
//! warns, the storm hits and anyone caught in the open takes the
//! storm's damage through the one hit path, player and NPC alike; when
//! it passes, the world shifts to another reality. The ground, what
//! stands on it, and the things lying about are rolled anew from a new
//! seed through the same spawn paths. The bunkers and the bunker people
//! stay; the topside people die with their world and the next world's
//! are found at its monuments.

use bevy::prelude::*;
use modforge::combat::Hit;
use modforge::storm::StormPhase;

use crate::actor::{Actor, ActorMapRes, Dead, Home, Npc};
use crate::ai::LifeStats;
use crate::combat::leave;
use crate::combat::{Equipment, Health, Protection, hit_actor};
use crate::content::{DamageRegistryRes, ItemRegistryRes};
use crate::survival::Day;
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

/// Where the world is in the storm's cycle now, on the day's clock.
pub fn phase(tick: &SimTick, day: Day) -> StormPhase {
    day.storm().phase(Day::secs(tick))
}

/// Every fixed step: during the storm, once a second, everyone in the
/// open takes the storm's damage; when a storm has passed, the world
/// shifts.
pub fn storm(
    mut commands: Commands,
    tick: Res<SimTick>,
    day: Res<Day>,
    items: Res<ItemRegistryRes>,
    damage: Res<DamageRegistryRes>,
    mut state: ResMut<StormRes>,
    shelters: Query<&Shelter>,
    mut actors: Query<(&Transform, &Protection, &Equipment, &mut Health), Without<Dead>>,
) {
    if phase(&tick, *day) == StormPhase::Storm
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
    let passed = day.storm().passed(Day::secs(&tick));
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
    topside_people: Query<(Entity, Option<&Actor>), (With<Npc>, Without<Home>)>,
    mut map: ResMut<ActorMapRes>,
    mut life: ResMut<LifeStats>,
) {
    for entity in &pieces {
        commands.entity(entity).despawn();
    }
    // The topside people are part of this world and die with it
    // (design.md "Bunker people and topside people").
    for (entity, actor) in &topside_people {
        life.died += 1;
        leave(&mut commands, &mut map, entity, actor);
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
    commands.run_system_cached(crate::actor::populate_monuments);
    if windowed.is_some() {
        commands.run_system_cached(crate::terrain::spawn_ground);
    }
}
