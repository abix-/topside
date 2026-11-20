//! Biomes (todo 32): each region's own look (its ground colour and
//! scatter, terrain.rs), weather (its weather list, weather.rs),
//! monuments (its monument pool, modforge worldgen), people and
//! wildlife, and resources, all read from its `BiomeDef` (content
//! `register_biomes`), the one owner. Who stands topside is the biome's:
//! the world's people table says how many and where, and the biome where
//! each stands says who, a person from its people, an animal from its
//! wildlife, each only if registered. A site's boxes hold the biome's
//! resources on top of their loot.

use bevy::prelude::*;
use modforge::biome::BiomeDef;
use modforge::weather::Weather;

use crate::content::BiomeRegistryRes;

/// The biome at a point on the ground.
pub fn at<'a>(world: &crate::terrain::WorldRes, biomes: &'a BiomeRegistryRes, point: Vec2) -> Option<&'a BiomeDef> {
    let ground = &world.world.ground;
    let name = ground.biomes.get(ground.biome_at(modforge::walk::cell_of(point)) as usize)?;
    biomes.0.def(name)
}

/// A biome's weather table: each weather it names, weighted by how often.
pub fn weathers(biome: Option<&BiomeDef>) -> Vec<(Weather, u32)> {
    let mut table: Vec<(Weather, u32)> = Vec::new();
    for weather in biome.into_iter().flat_map(|b| &b.weather).filter_map(|name| Weather::from_name(name)) {
        match table.iter_mut().find(|(w, _)| *w == weather) {
            Some((_, n)) => *n += 1,
            None => table.push((weather, 1)),
        }
    }
    table
}

/// Who stands at a topside spot: the biome's own, of the same sort (an
/// animal for an animal), picked by the spot's seed; else the world's.
pub fn who<'a>(biome: Option<&'a BiomeDef>, name: &'a str, registry: &crate::content::ActorRegistryRes, seed: u64) -> &'a str {
    let Some(biome) = biome else {
        return name;
    };
    let pool = if crate::content::ANIMALS.contains(&name) { &biome.wildlife } else { &biome.npcs };
    let known: Vec<&String> = pool.iter().filter(|n| registry.0.def(n.as_str()).is_some()).collect();
    if known.is_empty() {
        return name;
    }
    known[(seed % known.len() as u64) as usize]
}

/// One of the biome's resources added to a box, 1 to 3 of it, by seed.
pub fn harvest_into(contents: &mut modforge::item::Inventory, biome: Option<&BiomeDef>, items: &crate::content::ItemRegistryRes, seed: u64) {
    let Some(biome) = biome.filter(|b| !b.harvest.is_empty()) else {
        return;
    };
    let name = &biome.harvest[(seed % biome.harvest.len() as u64) as usize];
    if let Some(def) = items.0.def(name) {
        let stack = modforge::item::create(def, 1 + ((seed >> 16) % 3) as u32, &[], 0.0, seed);
        let _ = contents.add(stack, def.max_stack);
    }
}
