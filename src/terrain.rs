//! The 2D world, made and drawn chunk by chunk (design.md "2D world
//! generation", the Factorio way): modforge's `World2d` decides every
//! chunk from the seed; this makes the chunks the world needs now, around
//! the player, around every person, and at the bunkers, as Factorio makes
//! the chunks around its players. A chunk is two static bodies (water,
//! which stops a body; cliffs, trees, and rocks, which stop a body and
//! sight), its ground drawn per tile with a window, and the sites that
//! stand in it queued to be spawned. Nothing of a chunk is made twice.

use std::collections::HashSet;

use avian2d::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite_render::{TileData, TilemapChunk, TilemapChunkTileData};
use modforge::ground::{Ground, GroundChunk};
use modforge::places::Place;
use modforge::walk::{CHUNK, ChunkKey, Rect, TILE, cell_of};
use modforge::world2d::{Registries, World2d};

use crate::WorldSeed;
use crate::content::{BUNKER_AT, BiomeRegistryRes, MonumentRegistryRes, OTHER_BUNKERS, ground_color, roll_world2d};

/// The world: its ground, sites, and roads (modforge), the player's
/// bunker, the other bunkers, and the seed it was rolled from.
#[derive(Resource)]
pub struct WorldRes {
    pub world: World2d,
    pub seed: u64,
    pub bunker: Vec2,
    pub bunkers: Vec<Vec2>,
}

/// Anything that belongs to this reality and goes when the storm shifts
/// the world (design.md "The storm"): the ground, what stops a body, the
/// monuments. Never the bunkers, never a person.
#[derive(Component, Default)]
pub struct WorldPiece;

/// What stops sight as well as a body: cliffs, trees, rocks (design.md
/// "2D world generation": a cliff or a wood hides what is past it).
#[derive(Component, Default)]
pub struct BlocksSight;

/// The chunks made so far, and the sites spawned so far (by their tile).
#[derive(Resource, Default)]
pub struct Chunks {
    pub made: HashSet<ChunkKey>,
    pub sites: HashSet<(i32, i32)>,
}

/// Sites found in newly made chunks, waiting to be spawned.
#[derive(Resource, Default)]
pub struct NewSites(pub Vec<Place>);

/// The one white tile the ground is drawn with, tinted per tile.
#[derive(Resource)]
pub struct GroundTiles(Handle<Image>);

/// Chunks made round the player (drawn and solid), in chunks each way.
const AROUND_PLAYER: i32 = 3;
/// Chunks made round every person, in chunks each way.
const AROUND_PERSON: i32 = 1;

/// Roll the world from the seed: the ground, the site kinds, the bunkers'
/// clearings. Nothing is made until it is needed.
pub fn roll(
    mut commands: Commands,
    seed: Res<WorldSeed>,
    biomes: Res<BiomeRegistryRes>,
    monuments: Res<MonumentRegistryRes>,
) {
    // The seed in the log, so any world can be rolled again.
    info!("rolling the world from seed {}", seed.0);
    let world = roll_world2d(seed.0, &biomes.0, &monuments.monuments).expect("the ground def names registered biomes");
    commands.insert_resource(WorldRes {
        world,
        seed: seed.0,
        bunker: BUNKER_AT,
        bunkers: OTHER_BUNKERS.to_vec(),
    });
    commands.insert_resource(Chunks::default());
    commands.insert_resource(NewSites::default());
    commands.insert_resource(crate::structure::MonumentsRes::default());
}

/// The chunk a point is in.
pub fn chunk_of(p: Vec2) -> ChunkKey {
    let t = cell_of(p);
    (t.0.div_euclid(CHUNK), t.1.div_euclid(CHUNK))
}

/// The rectangles of one ground kind, as runs along each row.
fn runs_of(chunk: &GroundChunk, of: impl Fn(Ground) -> bool) -> Vec<Rect> {
    let f = chunk.first();
    let mut out = Vec::new();
    for y in 0..CHUNK {
        let mut x = 0;
        while x < CHUNK {
            if !of(chunk.ground[(y * CHUNK + x) as usize]) {
                x += 1;
                continue;
            }
            let start = x;
            while x < CHUNK && of(chunk.ground[(y * CHUNK + x) as usize]) {
                x += 1;
            }
            let min = Vec2::new((f.0 + start) as f32, (f.1 + y) as f32) * TILE;
            let max = Vec2::new((f.0 + x) as f32, (f.1 + y + 1) as f32) * TILE;
            out.push(Rect { min, max });
        }
    }
    out
}

fn body(commands: &mut Commands, rects: &[Rect], sight: bool) {
    if rects.is_empty() {
        return;
    }
    let shapes: Vec<(Vec2, Rotation, Collider)> = rects
        .iter()
        .map(|r| {
            let size = r.max - r.min;
            ((r.min + r.max) / 2.0, Rotation::IDENTITY, Collider::rectangle(size.x, size.y))
        })
        .collect();
    let mut e = commands.spawn((WorldPiece, RigidBody::Static, Collider::compound(shapes), Transform::default()));
    if sight {
        e.insert(BlocksSight);
    }
}

/// Above the ground, under people.
const THING_Z: f32 = 0.5;

/// Make every chunk the world needs now and has not made: round the
/// player, round every person, and at the bunkers.
pub fn make_chunks(
    mut commands: Commands,
    mut world: ResMut<WorldRes>,
    mut chunks: ResMut<Chunks>,
    mut new_sites: ResMut<NewSites>,
    biomes: Res<BiomeRegistryRes>,
    monuments: Res<MonumentRegistryRes>,
    tiles: Option<Res<GroundTiles>>,
    players: Query<&Transform, With<crate::player::Player>>,
    people: Query<&Transform, (With<crate::actor::Npc>, Without<crate::player::Player>)>,
) {
    let mut needed: Vec<ChunkKey> = Vec::new();
    let mut around = |p: Vec2, n: i32| {
        let c = chunk_of(p);
        for dy in -n..=n {
            for dx in -n..=n {
                needed.push((c.0 + dx, c.1 + dy));
            }
        }
    };
    for t in &players {
        around(t.translation.truncate(), AROUND_PLAYER);
    }
    for t in &people {
        around(t.translation.truncate(), AROUND_PERSON);
    }
    around(world.bunker, 2);
    for b in world.bunkers.clone() {
        around(b, 2);
    }
    let r = Registries {
        biomes: &biomes.0,
        monuments: &monuments.monuments,
        buildings: &monuments.buildings,
    };
    for key in needed {
        if !chunks.made.insert(key) {
            continue;
        }
        let chunk = world.world.chunk(r, key);
        body(&mut commands, &runs_of(&chunk, |g| g == Ground::Water), false);
        let mut solid = runs_of(&chunk, |g| g == Ground::Cliff);
        solid.extend(chunk.things.iter().map(|t| t.rect()));
        body(&mut commands, &solid, true);
        if let Some(tiles) = tiles.as_ref() {
            draw(&mut commands, &chunk, &tiles.0);
        }
        // The sites standing in this chunk, once each.
        let lo = Vec2::new((key.0 * CHUNK) as f32, (key.1 * CHUNK) as f32) * TILE;
        let hi = lo + Vec2::splat(CHUNK as f32 * TILE);
        for site in world.world.sites_in(r, lo, hi) {
            if chunks.sites.insert(cell_of(site.position)) {
                new_sites.0.push(site);
            }
        }
    }
}

/// Draw one chunk: its ground as one tilemap, a tile each, and every tree
/// and rock as a square.
fn draw(commands: &mut Commands, chunk: &GroundChunk, tileset: &Handle<Image>) {
    let first = chunk.first();
    let centre = Vec2::new(first.0 as f32, first.1 as f32) * TILE + Vec2::splat(CHUNK as f32 * TILE / 2.0);
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
                .ground
                .iter()
                .map(|g| {
                    let rgb = ground_color(*g);
                    Some(TileData {
                        color: Color::srgb(rgb[0], rgb[1], rgb[2]),
                        ..default()
                    })
                })
                .collect(),
        ),
        Transform::from_translation(centre.extend(0.0)).with_scale(Vec3::new(TILE, TILE, 1.0)),
    ));
    for thing in &chunk.things {
        let r = thing.rect();
        commands.spawn((
            WorldPiece,
            Sprite {
                color: Color::srgb(thing.color[0], thing.color[1], thing.color[2]),
                custom_size: Some(r.max - r.min),
                ..default()
            },
            Transform::from_translation(((r.min + r.max) / 2.0).extend(THING_Z)),
        ));
    }
}

/// With a window: the white tile the ground is drawn with.
pub fn setup_ground_tiles(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
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
    commands.insert_resource(GroundTiles(images.add(white)));
}
