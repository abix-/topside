//! A world's secrets (todo 11x1; design.md "The unknown" rule 7: "every
//! map should have the possibility to have 0-5 secrets"): modforge
//! `World2d::secrets` picks 0 to 5 places off every road and site, and a
//! secret site is placed at each: a loot box filled at a secret's danger,
//! nothing announcing it.

use bevy::prelude::*;
use modforge::world2d::Registries;

use crate::content::{BiomeRegistryRes, ItemRegistryRes, MonumentRegistryRes, SECRET_DANGER, loot_table};
use crate::terrain::{WorldPiece, WorldRes};

/// The world (its seed) whose secrets are placed: saved, so a load does
/// not place them twice; the boxes themselves are in the save.
#[derive(Resource, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct SecretsPlaced(pub Option<u64>);

/// How many chunks each way from the middle a world with no edge looks
/// for its secrets (Claude's, for the operator to change).
const NO_EDGE_CHUNKS: i32 = 8;

/// Once a world, after the player wakes: a secret site at each of the
/// world's secrets.
#[allow(clippy::too_many_arguments)]
pub fn place(
    mut commands: Commands,
    mut placed: ResMut<SecretsPlaced>,
    mut world: ResMut<WorldRes>,
    biomes: Res<BiomeRegistryRes>,
    monuments: Res<MonumentRegistryRes>,
    items: Res<ItemRegistryRes>,
    layers: Res<crate::content::LayerRegistryRes>,
    mut ids: ResMut<crate::interact::ThingIssuer>,
    (tick, day): (Res<crate::SimTick>, Res<crate::survival::Day>),
) {
    if placed.0 == Some(world.seed) {
        return;
    }
    placed.0 = Some(world.seed);
    let r = Registries {
        biomes: &biomes.0,
        monuments: &monuments.monuments,
        buildings: &monuments.buildings,
    };
    let range = world.world.ground.chunk_range().unwrap_or((-NO_EDGE_CHUNKS, NO_EDGE_CHUNKS));
    let found = world.world.secrets(r, range);
    info!("secrets: {} in this world", found.len());
    let Some(box_def) = items.0.def("storage box") else {
        return;
    };
    let table = loot_table();
    let today = crate::survival::today(&tick, *day);
    for (k, at) in found.into_iter().enumerate() {
        let seed = world.seed ^ 0x5EC2_E700 ^ k as u64;
        let contents = crate::structure::fill_box(&items, &layers, &table, box_def, SECRET_DANGER, seed, today);
        let stack = modforge::item::create(box_def, 1, &[], 0.0, seed);
        let (made, _) = crate::interact::spawn_item(&mut commands, &mut ids, box_def, stack, crate::on_ground(at, 0.0));
        commands.entity(made).insert((crate::interact::Container(contents), WorldPiece));
        info!("secrets: a secret site at {at}");
    }
}
