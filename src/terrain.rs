//! The rolled world drawn flat from above (docs/design.md "What 2D
//! changes"): every cell coloured by road, water, or its biome's
//! ground, one `TilemapChunk` per `CHUNK` by `CHUNK` cells. One world
//! unit is one metre, and modforge's (x, y) is the screen's (x, y).
//! What stops a body (water, scattered nature, the Labyrinth's walls)
//! is one static Avian body per chunk with one compound collider, the
//! 3D game's rule (performance.md: one collider per chunk, never a
//! body per piece).

use avian2d::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite_render::{TileData, TilemapChunk, TilemapChunkTileData};
use modforge::biome::BiomeRegistry;
use modforge::structure::Rgb;
use modforge::worldgen::{Scattered, World, roll_world};

use crate::content::{BiomeRegistryRes, MonumentRegistryRes, world_def};
use crate::{WorldName, WorldSeed};

/// Cells per chunk side.
pub const CHUNK: usize = 32;
const ROAD_COLOR: Rgb = [0.22, 0.21, 0.2];
const WATER_COLOR: Rgb = [0.2, 0.3, 0.45];

/// The rolled world, for anything that asks what is where.
#[derive(Resource)]
pub struct WorldRes(pub World);

/// Anything that belongs to this reality and goes when the storm
/// shifts the world (design.md "The storm"): the ground, what stops a
/// body, the monuments. Never the bunker, never a person.
#[derive(Component, Default)]
pub struct WorldPiece;

/// One chunk of ground: the cell at its bottom left and the colour of
/// every cell in it, row by row from the bottom. `None` is past the
/// world's edge.
pub struct Chunk {
    pub first: UVec2,
    pub tiles: Vec<Option<Rgb>>,
}

/// Roll the world from the seed and the reality's def.
pub fn roll(
    mut commands: Commands,
    seed: Res<WorldSeed>,
    name: Res<WorldName>,
    biomes: Res<BiomeRegistryRes>,
    monuments: Res<MonumentRegistryRes>,
) {
    let def = world_def(&name.0).expect("the world name is one of content::WORLDS");
    let world = roll_world(&def, seed.0, &biomes.0, &monuments.monuments)
        .expect("the world def rolls with the registered content");
    commands.insert_resource(WorldRes(world));
}

/// The ground as chunks: each cell is road, water, or its biome's
/// ground colour.
pub fn chunks(world: &World, biomes: &BiomeRegistry) -> Vec<Chunk> {
    let n = world.cells;
    let road = world.road_cells();
    let count = n.div_ceil(CHUNK);
    let mut out = Vec::with_capacity(count * count);
    for cr in 0..count {
        for cc in 0..count {
            let (c0, r0) = (cc * CHUNK, cr * CHUNK);
            let mut tiles = Vec::with_capacity(CHUNK * CHUNK);
            for r in r0..r0 + CHUNK {
                for c in c0..c0 + CHUNK {
                    tiles.push((c < n && r < n).then(|| {
                        if road[r * n + c] {
                            ROAD_COLOR
                        } else if world.is_water(c, r) {
                            WATER_COLOR
                        } else {
                            biomes.def(world.biome_at_cell(c, r)).map_or([0.3, 0.3, 0.3], |b| b.ground)
                        }
                    }));
                }
            }
            out.push(Chunk {
                first: UVec2::new(c0 as u32, r0 as u32),
                tiles,
            });
        }
    }
    out
}

/// One solid rectangle on the ground: centre and size in metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Solid {
    pub centre: Vec2,
    pub size: Vec2,
}

/// The chunk (as an index into `chunks`) a point falls in.
fn chunk_of(world: &World, p: Vec2) -> usize {
    let (c, r) = world.cell_of(p);
    let count = world.cells.div_ceil(CHUNK);
    (r / CHUNK) * count + c / CHUNK
}

/// What stops a body, per chunk in the order of `chunks`: every run of
/// water cells along a row, every piece of scatter by its footprint
/// (size x across, z deep), and every wall of the generator by its
/// footprint, each in the chunk its centre falls in.
pub fn solids(world: &World, scatter: &[Scattered]) -> Vec<Vec<Solid>> {
    let n = world.cells;
    let count = n.div_ceil(CHUNK);
    let cell = world.def.cell;
    let mut out = vec![Vec::new(); count * count];
    for r in 0..n {
        let mut c = 0;
        while c < n {
            if !world.is_water(c, r) {
                c += 1;
                continue;
            }
            // A run stays inside one chunk so each chunk owns its own.
            let start = c;
            let end = ((start / CHUNK + 1) * CHUNK).min(n);
            while c < end && world.is_water(c, r) {
                c += 1;
            }
            let from = world.cell_center(start, r);
            let to = world.cell_center(c - 1, r);
            out[(r / CHUNK) * count + start / CHUNK].push(Solid {
                centre: (from + to) / 2.0,
                size: Vec2::new((c - start) as f32 * cell, cell),
            });
        }
    }
    for s in scatter {
        out[chunk_of(world, s.position)].push(Solid {
            centre: s.position,
            size: Vec2::new(s.size.x, s.size.z),
        });
    }
    for w in &world.walls {
        let (min, max) = w.footprint();
        let centre = (min + max) / 2.0;
        out[chunk_of(world, centre)].push(Solid {
            centre,
            size: max - min,
        });
    }
    out
}

/// Above the ground, under people.
const SCATTER_Z: f32 = 0.5;
const WALL_COLOR: Rgb = [0.38, 0.36, 0.33];

/// Spawn what stops a body: one static body per chunk with one
/// compound collider, and every piece of scatter and every wall drawn
/// as its footprint in its colour.
pub fn spawn_solids(mut commands: Commands, world: Res<WorldRes>, biomes: Res<BiomeRegistryRes>) {
    let world = &world.0;
    let scatter = world.scatter(&biomes.0);
    for chunk in solids(world, &scatter) {
        if chunk.is_empty() {
            continue;
        }
        let shapes: Vec<(Vec2, Rotation, Collider)> = chunk
            .iter()
            .map(|s| (s.centre, Rotation::IDENTITY, Collider::rectangle(s.size.x, s.size.y)))
            .collect();
        commands.spawn((WorldPiece, RigidBody::Static, Collider::compound(shapes), Transform::default()));
    }
    let sprite = |rgb: Rgb, size: Vec2| Sprite {
        color: Color::srgb(rgb[0], rgb[1], rgb[2]),
        custom_size: Some(size),
        ..default()
    };
    for s in &scatter {
        commands.spawn((
            WorldPiece,
            sprite(s.color, Vec2::new(s.size.x, s.size.z)),
            Transform::from_translation(s.position.extend(SCATTER_Z)),
        ));
    }
    for w in &world.walls {
        let (min, max) = w.footprint();
        commands.spawn((
            WorldPiece,
            sprite(WALL_COLOR, max - min),
            Transform::from_translation(((min + max) / 2.0).extend(SCATTER_Z)),
        ));
    }
}

/// Spawn the ground: one tilemap chunk per chunk, each tile one white
/// pixel tinted the cell's colour, scaled to the cell size.
pub fn spawn_ground(
    mut commands: Commands,
    world: Res<WorldRes>,
    biomes: Res<BiomeRegistryRes>,
    mut images: ResMut<Assets<Image>>,
) {
    let world = &world.0;
    // TilemapChunk samples an array texture of at least two layers.
    let mut white = Image::new_fill(
        Extent3d {
            width: 1,
            height: 2,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[255, 255, 255, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    white
        .reinterpret_stacked_2d_as_array(2)
        .expect("two stacked pixels are two layers");
    let tileset = images.add(white);
    let cell = world.def.cell;
    let half = world.def.size / 2.0;
    let side = CHUNK as f32 * cell;
    for chunk in chunks(world, &biomes.0) {
        let centre = chunk.first.as_vec2() * cell - Vec2::splat(half) + Vec2::splat(side / 2.0);
        commands.spawn((
            WorldPiece,
            TilemapChunk {
                chunk_size: UVec2::splat(CHUNK as u32),
                tile_display_size: UVec2::ONE,
                tileset: tileset.clone(),
                ..default()
            },
            TilemapChunkTileData(
                chunk
                    .tiles
                    .iter()
                    .map(|t| {
                        t.map(|rgb| TileData {
                            color: Color::srgb(rgb[0], rgb[1], rgb[2]),
                            ..default()
                        })
                    })
                    .collect(),
            ),
            Transform::from_translation(centre.extend(0.0)).with_scale(Vec3::new(cell, cell, 1.0)),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rolled_world_is_one_chunk_per_32_by_32_cells() {
        let biomes = BiomeRegistryRes::default();
        let monuments = MonumentRegistryRes::default();
        let def = world_def("topside").unwrap();
        let world = roll_world(&def, 1, &biomes.0, &monuments.monuments).unwrap();
        let chunks = chunks(&world, &biomes.0);
        let side = world.cells.div_ceil(CHUNK);
        assert_eq!(chunks.len(), side * side);
        assert!(chunks.iter().all(|c| c.tiles.len() == CHUNK * CHUNK));
        // Every cell of the world is drawn exactly once.
        let drawn: usize = chunks.iter().map(|c| c.tiles.iter().flatten().count()).sum();
        assert_eq!(drawn, world.cells * world.cells);
    }

    #[test]
    fn every_water_cell_is_solid_once() {
        let biomes = BiomeRegistryRes::default();
        let monuments = MonumentRegistryRes::default();
        let def = world_def("topside").unwrap();
        let world = roll_world(&def, 1, &biomes.0, &monuments.monuments).unwrap();
        let solids = solids(&world, &[]);
        let water = (0..world.cells)
            .flat_map(|r| (0..world.cells).map(move |c| (c, r)))
            .filter(|&(c, r)| world.is_water(c, r))
            .count();
        let cell_area = world.def.cell * world.def.cell;
        let solid_area: f32 = solids.iter().flatten().map(|s| s.size.x * s.size.y).sum();
        assert!(water > 0, "the test world has water");
        assert!(
            (solid_area / cell_area - water as f32).abs() < 0.5,
            "{} cells of water, {} cells of solid",
            water,
            solid_area / cell_area
        );
    }
}
