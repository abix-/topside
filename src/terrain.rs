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
use bevy::image::{ImageArrayLayout, ImageLoaderSettings};
use bevy::prelude::*;
use bevy::sprite_render::{TileData, TilemapChunk, TilemapChunkTileData};
use modforge::ground::{Ground, GroundChunk};
use modforge::places::Place;
use modforge::walk::{CHUNK, ChunkKey, Rect, TILE, cell_of};
use modforge::world2d::{Registries, World2d};

use crate::WorldSeed;
use crate::content::{BUNKER_AT, BiomeRegistryRes, MonumentRegistryRes, WorldKind, ground_color, roll_world2d};

/// The world: its ground, sites, and roads (modforge), the player's
/// bunker, the other bunkers, and the seed it was rolled from.
#[derive(Resource)]
pub struct WorldRes {
    pub world: World2d,
    pub seed: u64,
    pub bunker: Vec2,
    pub bunkers: Vec<Vec2>,
    /// Whether this world's topside people have been put in it.
    pub peopled: bool,
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
    /// Per chunk made, its shore: the land tiles beside water, where a
    /// person stands to drink from a river (design.md: rivers drinkable).
    pub shores: std::collections::HashMap<ChunkKey, Vec<(i32, i32)>>,
}

/// The land tiles of `chunk` with water beside them (in the chunk).
fn shore_of(chunk: &GroundChunk) -> Vec<(i32, i32)> {
    let f = chunk.first();
    let inside = |t: (i32, i32)| t.0 >= f.0 && t.0 < f.0 + CHUNK && t.1 >= f.1 && t.1 < f.1 + CHUNK;
    let mut out = Vec::new();
    for y in 0..CHUNK {
        for x in 0..CHUNK {
            let t = (f.0 + x, f.1 + y);
            if !chunk.ground_at(t).is_land() {
                continue;
            }
            let wet = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .into_iter()
                .map(|(dx, dy)| (t.0 + dx, t.1 + dy))
                .any(|n| inside(n) && chunk.ground_at(n) == Ground::Water);
            if wet {
                out.push(t);
            }
        }
    }
    out
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
    kind: Res<WorldKind>,
    biomes: Res<BiomeRegistryRes>,
    monuments: Res<MonumentRegistryRes>,
) {
    // The seed in the log, so any world can be rolled again.
    info!("rolling the {:?} world from seed {}", *kind, seed.0);
    let world = roll_world2d(*kind, seed.0, &biomes.0, &monuments.monuments).expect("the ground def names registered biomes");
    commands.insert_resource(WorldRes {
        world,
        seed: seed.0,
        bunker: BUNKER_AT,
        bunkers: kind.other_bunkers(),
        peopled: false,
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
    // A world with an edge is made whole, and the ring of chunks past the
    // edge with it, so the edge stops a body wherever anyone walks.
    if let Some((lo, hi)) = world.world.ground.chunk_range() {
        for y in lo - 1..=hi {
            for x in lo - 1..=hi {
                needed.push((x, y));
            }
        }
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
        let mut solid = runs_of(&chunk, |g| matches!(g, Ground::Cliff | Ground::OutOfMap));
        solid.extend(chunk.things.iter().filter(|t| t.source.is_none()).map(|t| t.rect()));
        body(&mut commands, &solid, true);
        let shore = shore_of(&chunk);
        if !shore.is_empty() {
            chunks.shores.insert(key, shore);
        }
        // The sources (wells, berry bushes) stop nobody: each is a spot
        // to use, drawn with the chunk's things.
        for thing in &chunk.things {
            if let Some(name) = &thing.source {
                crate::interact::spawn_source(&mut commands, name, modforge::walk::centre(thing.tile));
            }
        }
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

/// Draw one chunk: its ground as one tilemap, a tile each, and every
/// thing on it as its picture.
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
                .enumerate()
                .map(|(i, g)| {
                    let rgb = ground_color(*g);
                    let tile = (first.0 + i as i32 % CHUNK, first.1 + i as i32 / CHUNK);
                    Some(TileData {
                        tileset_index: ground_layer(*g, tile),
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
        let centre = (r.min + r.max) / 2.0;
        // Lower on the screen draws in front, so a forest's crowns overlap
        // the right way (still under people: a kilometre of y moves this
        // a thousandth).
        let at = Transform::from_translation(centre.extend(THING_Z - centre.y * 1e-6));
        // Its picture; never a box (design.md: no coloured boxes).
        match &thing.picture {
            Some(name) => {
                commands.spawn((WorldPiece, crate::picture::Picture(name.clone()), at));
            }
            None => warn!("a thing at {:?} has no picture", thing.tile),
        }
    }
}

/// The kinds of ground in the order scripts/models.py stacks their tiles
/// into assets/ground.png (its GROUND_KINDS), each with
/// GROUND_VARIANTS tiles; change both together.
const GROUND_KINDS: [Ground; 9] = [
    Ground::Grass,
    Ground::Dirt,
    Ground::Sand,
    Ground::ForestFloor,
    Ground::Rubble,
    Ground::Road,
    Ground::Water,
    Ground::Cliff,
    Ground::OutOfMap,
];
const GROUND_VARIANTS: u16 = 4;

/// Which ground tile picture a tile shows: its kind's, in a variant picked
/// by where it is (as Factorio picks a tile's variation), so the ground
/// does not repeat and a tile always looks the same.
fn ground_layer(ground: Ground, tile: (i32, i32)) -> u16 {
    let kind = GROUND_KINDS.iter().position(|g| *g == ground).unwrap_or(0) as u16;
    let hash = (tile.0 as u32).wrapping_mul(73_856_093) ^ (tile.1 as u32).wrapping_mul(19_349_663);
    kind * GROUND_VARIANTS + (hash.wrapping_mul(2_654_435_761) >> 30) as u16 % GROUND_VARIANTS
}

/// With a window: the ground tiles (scripts/models.py renders them), one
/// picture a tile tall each, stacked into one strip read as a tileset.
pub fn setup_ground_tiles(mut commands: Commands, assets: Res<AssetServer>) {
    let tiles = assets
        .load_builder()
        .with_settings(|s: &mut ImageLoaderSettings| {
            s.array_layout = Some(ImageArrayLayout::RowHeight {
                pixels: crate::picture::PIXELS_PER_METRE as u32,
            });
        })
        // Outside assets/pictures, so loading that folder never loads the
        // strip as a plain picture.
        .load("ground.png");
    commands.insert_resource(GroundTiles(tiles));
}
