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

/// A secret's box, for a rumor to tell of (todo Events 6, src/show.rs;
/// saved).
#[derive(Component, Clone, Copy, Default, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Secret;

/// How many chunks each way from the middle a world with no edge looks
/// for its secrets (Claude's, for the operator to change).
const NO_EDGE_CHUNKS: i32 = 8;

/// A ledge's height: past a step (`player::CLIMB`), within a jump and a
/// step; its sides; a pen's walls, its side and the walls' thickness;
/// how far a secret looks for water to sink its box in (all Claude's).
const LEDGE: f32 = 0.8;
const LEDGE_SIDE: f32 = 2.0;
const PEN_WALL: f32 = 1.6;
const PEN_SIDE: f32 = 3.0;
const PEN_THICK: f32 = 0.3;
const SWIM_SEARCH: i32 = 40;

/// A solid block standing on the ground at `at`, `size` big: drawn as a
/// stone, stopping a body.
fn solid(commands: &mut Commands, at: Vec2, size: Vec3) {
    commands.spawn((
        WorldPiece,
        avian3d::prelude::RigidBody::Static,
        avian3d::prelude::Collider::cuboid(size.x, size.y, size.z),
        crate::model::Model("stone".to_string()),
        Transform::from_translation(crate::on_ground(at, size.y / 2.0)).with_scale(size),
    ));
}

/// A walled pen with no door: a ledge outside to jump onto, the wall's
/// top to jump to, and the fall in; a ledge inside to jump back out.
fn pen(commands: &mut Commands, at: Vec2) {
    let half = PEN_SIDE / 2.0;
    solid(commands, at + Vec2::new(0.0, half), Vec3::new(PEN_SIDE, PEN_WALL, PEN_THICK));
    solid(commands, at - Vec2::new(0.0, half), Vec3::new(PEN_SIDE, PEN_WALL, PEN_THICK));
    solid(commands, at + Vec2::new(half, 0.0), Vec3::new(PEN_THICK, PEN_WALL, PEN_SIDE));
    solid(commands, at - Vec2::new(half, 0.0), Vec3::new(PEN_THICK, PEN_WALL, PEN_SIDE));
    solid(commands, at - Vec2::new(0.0, half + 0.6), Vec3::new(1.0, LEDGE, 0.9));
    solid(commands, at + Vec2::new(0.0, half - 0.6), Vec3::new(1.0, LEDGE, 0.9));
}

/// The water tile nearest `at` within `SWIM_SEARCH` tiles, its middle.
fn nearest_water(world: &WorldRes, at: Vec2) -> Option<Vec2> {
    let c = modforge::walk::cell_of(at);
    (-SWIM_SEARCH..=SWIM_SEARCH)
        .flat_map(|y| (-SWIM_SEARCH..=SWIM_SEARCH).map(move |x| (c.0 + x, c.1 + y)))
        .filter(|t| world.world.ground.ground_at(*t) == modforge::ground::Ground::Water)
        .map(modforge::walk::centre)
        .min_by(|a, b| a.distance(at).total_cmp(&b.distance(at)))
}

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
        // One tier above the loot of the place nearest it (todo Secrets 3,
        // operator, 2026-11-24: "1 tier higher than normal loot"): a spot's
        // loot tier is its danger and one (`fill_box`), so the nearest
        // lot's tier as the danger is one above its own; with no town, the
        // secrets' own danger.
        let danger = world
            .world
            .town
            .as_ref()
            .and_then(|town| (0..town.lots.len()).min_by(|&a, &b| town.lots[a].at.distance(at).total_cmp(&town.lots[b].at.distance(at))).map(|i| town.tier_of(i)))
            .unwrap_or(SECRET_DANGER);
        let contents = crate::structure::fill_box(&items, &layers, &table, box_def, danger, seed, today);
        let stack = modforge::item::create(box_def, 1, &[], 0.0, seed);
        // Reached only by a jump, by a jump and a fall, or by a swim, in
        // turn (todo Secrets 2, topside-town "Lots of secrets").
        let water = (k % 3 == 2).then(|| nearest_water(&world, at)).flatten();
        let box_at = match (k % 3, water) {
            (2, Some(wet)) => crate::on_ground(wet, 0.0),
            (1, _) => {
                pen(&mut commands, at);
                crate::on_ground(at, 0.0)
            }
            _ => {
                solid(&mut commands, at, Vec3::new(LEDGE_SIDE, LEDGE, LEDGE_SIDE));
                crate::on_ground(at, LEDGE)
            }
        };
        let (made, _) = crate::interact::spawn_item(&mut commands, &mut ids, box_def, stack, box_at);
        commands.entity(made).insert((crate::interact::Container(contents), WorldPiece, Secret));
        info!("secrets: a secret site at {at}");
    }
}
