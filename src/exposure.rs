//! Temperature and radiation on everyone (todo 20), on modforge's rules
//! (modforge exposure.rs). The air is mild by day and cold at night, and
//! mild under shelter; the storm is radiation in the open, and the
//! weather (weather.rs) adds its own to both. Out of the
//! safe band, or over the dose line, a person is hurt once a game second
//! through the one hit path, as hunger hurts. Everyone is equal. The
//! airs and the storm's radiation are Claude's, for the operator to
//! change.

use bevy::prelude::*;
use modforge::combat::Hit;
use modforge::exposure::Harm;

use crate::actor::{Dead, Memory};
use crate::combat::{Equipment, Health, Protection, hit_actor};
use crate::content::{DamageRegistryRes, ItemRegistryRes};
use crate::structure::{Shelter, sheltered};
use crate::Ground;

/// The air by day, at night, and under shelter, degrees C.
const DAY_AIR: f32 = 18.0;
const NIGHT_AIR: f32 = 2.0;
const SHELTER_AIR: f32 = 18.0;
/// The storm's radiation in the open, a game second.
const STORM_RADIATION: f32 = 2.0;

/// One person's exposure (saved).
#[derive(Component, Default, Clone, Copy, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Exposure(pub modforge::exposure::Exposure);

/// Every step: each person's warmth and dose move with the air and the
/// storm; once a game second whatever is out of bounds hurts.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn expose(
    mut commands: Commands,
    time: Res<Time<Fixed>>,
    tick: Res<crate::SimTick>,
    day: Res<crate::survival::Day>,
    reality: Res<crate::Reality>,
    storms: Res<crate::storm::StormRes>,
    harm: (Res<ItemRegistryRes>, Res<crate::content::LayerRegistryRes>, Res<DamageRegistryRes>),
    shelters: Query<&Shelter>,
    world: (Option<Res<crate::terrain::WorldRes>>, Res<crate::content::BiomeRegistryRes>),
    mut people: Query<(Entity, &Transform, Option<&mut Exposure>, &Protection, &Equipment, &mut Health, Option<&mut Memory>), (With<crate::actor::Actor>, Without<Dead>)>,
) {
    let (items, layers, damage) = harm;
    let (world, biomes) = world;
    let night = crate::survival::night(&tick, *day, &reality);
    let storm = matches!(crate::storm::phase(&tick, *day, &storms), modforge::storm::StormPhase::Storm);
    let second = tick.0 % 60 == 0;
    for (entity, at, exposure, own, worn, mut health, mut memory) in &mut people {
        let Some(mut exposure) = exposure else {
            commands.entity(entity).insert(Exposure::default());
            continue;
        };
        let inside = sheltered(&shelters, at.translation.ground());
        // The weather where they stand, in the open (todo 21).
        let weather = world.as_deref().filter(|_| !inside).map(|w| crate::weather::at(w, &biomes, tick.0, *day, at.translation.ground()));
        let base_air = if inside { SHELTER_AIR } else if night { NIGHT_AIR } else { DAY_AIR };
        let air = base_air + weather.map_or(0.0, |w| w.air());
        let base_radiation = if storm && !inside { STORM_RADIATION } else { 0.0 };
        let radiation = base_radiation + weather.map_or(0.0, |w| w.radiation());
        exposure.0.tick(air, radiation, time.delta_secs());
        if !second {
            continue;
        }
        for harm in exposure.0.harms() {
            let name = match harm {
                Harm::Cold => "cold",
                Harm::Heat => "heat",
                Harm::Radiation => "radiation",
            };
            let Some(def) = damage.0.def(name) else {
                continue;
            };
            let hit = Hit { def, self_inflicted: false, distance: 0.0, location_scale: 1.0 };
            hit_actor(&hit, own, worn, &items.0, &layers.0, Default::default(), &mut health, memory.as_deref_mut(), None, tick.0);
        }
    }
}
