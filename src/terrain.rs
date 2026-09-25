//! The rolled world drawn flat from above (docs/design.md "What 2D
//! changes"): every cell coloured by road, water, or its biome's
//! ground, one `TilemapChunk` per `CHUNK` by `CHUNK` cells. One world
//! unit is one metre, and modforge's (x, y) is the screen's (x, y).

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite_render::{TileData, TilemapChunk, TilemapChunkTileData};
use modforge::biome::BiomeRegistry;
use modforge::structure::Rgb;
use modforge::worldgen::{World, roll_world};

use crate::content::{BiomeRegistryRes, MonumentRegistryRes, world_def};
use crate::{WorldName, WorldSeed};

/// Cells per chunk side.
pub const CHUNK: usize = 32;
/// Cells on either side of a road's centre line that are road.
const ROAD_HALF_WIDTH: usize = 1;
const ROAD_COLOR: Rgb = [0.22, 0.21, 0.2];
const WATER_COLOR: Rgb = [0.2, 0.3, 0.45];

/// The rolled world, for anything that asks what is where.
#[derive(Resource)]
pub struct WorldRes(pub World);

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
    let mut road = vec![false; n * n];
    for r in &world.roads {
        for p in &r.points {
            let (c, row) = world.cell_of(*p);
            for dr in -(ROAD_HALF_WIDTH as isize)..=(ROAD_HALF_WIDTH as isize) {
                for dc in -(ROAD_HALF_WIDTH as isize)..=(ROAD_HALF_WIDTH as isize) {
                    let (cc, rr) = (c as isize + dc, row as isize + dr);
                    if cc >= 0 && rr >= 0 && (cc as usize) < n && (rr as usize) < n {
                        road[rr as usize * n + cc as usize] = true;
                    }
                }
            }
        }
    }
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
}
