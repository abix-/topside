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

use crate::actor::{Dead, Persona};
use crate::combat::{Equipment, Health, Protection, hit_actor};
use crate::content::{DamageRegistryRes, ItemRegistryRes};
use crate::structure::{Roofs, sheltered};
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
    shelters: Res<Roofs>,
    world: (Option<Res<crate::terrain::WorldRes>>, Res<crate::content::BiomeRegistryRes>, Res<crate::event::EventsRes>),
    mut people: Query<(Entity, &Transform, Option<&mut Exposure>, &Protection, &Equipment, &mut Health, Option<&mut Persona>, Option<&crate::actor::Inventory>), (With<crate::actor::Actor>, Without<Dead>, Without<crate::vehicle::Vehicle>)>,
    heaters: Query<(&crate::interact::WorldItem, &Transform, &crate::power::Powered), Without<crate::actor::Actor>>,
    mut biome_of: Local<(u64, std::collections::HashMap<modforge::path::Cell, u16>)>,
) {
    // The heat sources running now, each where, its heat and its reach
    // (topside-town "Everything works"; a fire while it burns, a furnace
    // or an air conditioner while it has power and its thermostat is on).
    let heat: Vec<(Vec3, f32, f32)> = heaters
        .iter()
        .filter(|(.., on)| on.0)
        .filter_map(|(item, at, _)| crate::content::HEAT.iter().find(|(name, ..)| *name == item.stack.item).map(|(_, h, r)| (at.translation, *h, *r)))
        .collect();
    let (items, layers, damage) = harm;
    let (world, biomes, events) = world;
    let night = crate::survival::night(&tick, *day, &reality);
    let storm = matches!(crate::storm::phase(&tick, *day, &storms), modforge::storm::StormPhase::Storm);
    let second = tick.0 % 60 == 0;
    // The weather is the same for everyone in one biome this step: worked
    // out once a biome (by its place in the world's biome list), not once
    // a person.
    let mut weathers: Vec<(u16, modforge::weather::Weather)> = Vec::new();
    let mut near: Vec<(f32, f32, f32)> = Vec::with_capacity(heat.len());
    // The heat by where it stands, and the farthest any reaches: each
    // person looks only at the heat whose cells their reach covers.
    let heat_at = crate::ai::Near::new(heat.iter().map(|h| h.0));
    let farthest = heat.iter().map(|h| h.2).fold(0.0, f32::max);
    let mut heat_near: Vec<usize> = Vec::new();
    // A tile's biome is the world's noise, the same for the whole world:
    // kept by tile until the world is rolled again (bounded by the tiles
    // people stand on).
    if let Some(w) = world.as_deref()
        && biome_of.0 != w.seed
    {
        *biome_of = (w.seed, Default::default());
    }
    for (entity, at, exposure, own, worn, mut health, mut memory, carried) in &mut people {
        let Some(mut exposure) = exposure else {
            commands.entity(entity).insert(Exposure::default());
            continue;
        };
        let inside = sheltered(&shelters, at.translation.ground());
        // The weather where they stand, in the open (todo 21).
        let weather = world.as_deref().filter(|_| !inside).map(|w| {
            let cell = modforge::walk::cell_of(at.translation.ground());
            let biome = *biome_of.1.entry(cell).or_insert_with(|| w.world.ground.biome_at(cell));
            match weathers.iter().find(|(b, _)| *b == biome) {
                Some((_, weather)) => *weather,
                None => {
                    let weather = crate::weather::at(w, &biomes, &events, tick.0, *day, at.translation.ground());
                    weathers.push((biome, weather));
                    weather
                }
            }
        });
        let base_air = if inside { SHELTER_AIR } else if night { NIGHT_AIR } else { DAY_AIR };
        // Near a fire, a furnace or an AC, and wrapped in a blanket: only
        // the ones in reach warm (modforge `warmth_near`).
        near.clear();
        heat_at.within(at.translation, farthest, &mut heat_near);
        near.extend(heat_near.iter().map(|&i| &heat[i]).map(|(p, h, r)| (p.distance(at.translation), *h, *r)).filter(|(far, _, reach)| far < reach));
        let blanket = if carried.is_some_and(|i| i.0.count_of(crate::content::BLANKET) > 0) { crate::content::BLANKET_WARMTH } else { 0.0 };
        let air = base_air + weather.map_or(0.0, |w| w.air()) + modforge::exposure::warmth_near(&near) + blanket;
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
