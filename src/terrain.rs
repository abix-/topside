//! The world, made and drawn chunk by chunk (design.md "2D world
//! generation", the Factorio way): modforge's `World2d` decides every
//! chunk's ground from the seed; this makes the chunks the world needs
//! now, around the player, around every person, and at the bunkers, as
//! Factorio makes the chunks around its players. A chunk is two static
//! bodies (water, which stops a body and is seen over; cliffs, trees, and
//! rocks, which stop a body and sight), its ground drawn as one mesh with
//! a window, and the sites that stand in it queued to be spawned. Nothing
//! of a chunk is made twice.

use std::collections::HashSet;

use avian3d::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use modforge::ground::{Ground, GroundChunk};
use modforge::places::Place;
use modforge::walk::{CHUNK, ChunkKey, Rect, TILE, cell_of};
use modforge::world2d::{Registries, World2d};

use crate::content::{BUNKER_AT, BiomeRegistryRes, MonumentRegistryRes, WorldKind, ground_color, roll_world2d};
use crate::model::Model;
use crate::{Ground as _, WorldSeed, on_ground};

/// How tall water stands as a body: it stops a person, and is seen over.
const WATER_HEIGHT: f32 = 1.0;
/// How tall cliffs, trees, and rocks stand as a body: over the eyes, so
/// they hide what is past them.
const SOLID_HEIGHT: f32 = 4.0;

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
#[derive(Component, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
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
    /// Per chunk made, its shore, from the made chunk: each land tile
    /// beside water with the water tile beside it, where a person stands
    /// to drink from a river and what they drink (design.md: rivers
    /// drinkable).
    pub shores: std::collections::HashMap<ChunkKey, Vec<Shore>>,
    /// Road tiles in the chunks made, for the `world` op.
    pub roads: usize,
    /// Per chunk made, the ground of each of its tiles (row by row), the
    /// one answer for what a tile is: water is seen over, and the map
    /// draws each kind its colour.
    pub ground: std::collections::HashMap<ChunkKey, Vec<Ground>>,
}

impl Chunks {
    /// The ground of tile `t`, if its chunk is made.
    pub fn ground_at(&self, t: (i32, i32)) -> Option<Ground> {
        let key = (t.0.div_euclid(CHUNK), t.1.div_euclid(CHUNK));
        let i = (t.1.rem_euclid(CHUNK) * CHUNK + t.0.rem_euclid(CHUNK)) as usize;
        self.ground.get(&key).and_then(|g| g.get(i).copied())
    }
}

/// A land tile beside water, and that water tile.
#[derive(Clone, Copy, Debug)]
pub struct Shore {
    pub land: (i32, i32),
    pub water: (i32, i32),
}

/// The land tiles of the made `chunk` with water beside them (in the
/// chunk), each with its water tile.
fn shore_of(chunk: &GroundChunk) -> Vec<Shore> {
    let f = chunk.first();
    let inside = |t: (i32, i32)| t.0 >= f.0 && t.0 < f.0 + CHUNK && t.1 >= f.1 && t.1 < f.1 + CHUNK;
    let mut out = Vec::new();
    for y in 0..CHUNK {
        for x in 0..CHUNK {
            let land = (f.0 + x, f.1 + y);
            if !chunk.ground_at(land).is_land() {
                continue;
            }
            let water = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .into_iter()
                .map(|(dx, dy)| (land.0 + dx, land.1 + dy))
                .find(|n| inside(*n) && chunk.ground_at(*n) == Ground::Water);
            if let Some(water) = water {
                out.push(Shore { land, water });
            }
        }
    }
    out
}

/// Sites found in newly made chunks, waiting to be spawned, each with
/// whether its things are to be made (false: they come from a save).
#[derive(Resource, Default)]
pub struct NewSites(pub Vec<(Place, bool)>);

/// What this world has made of its own things so far (Minecraft's
/// populated chunks): the chunks whose wells and bushes were put out, the
/// sites (by their tile) whose doors, loot boxes and people were, and
/// whether the topside people on open land were. Saved; a load rolls the
/// world again from its seed and makes the ground, walls and props of
/// every chunk it needs, but the things of these only come from the save,
/// never a second time. A new world (`roll`) has made none.
#[derive(Resource, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct ThingsMade {
    pub chunks: HashSet<ChunkKey>,
    pub sites: HashSet<(i32, i32)>,
    pub people: bool,
}

/// With a window: the one material every chunk's ground mesh is drawn
/// with, its colour from each tile's corners (the ground kind's colour).
#[derive(Resource)]
pub struct GroundMaterial(Handle<StandardMaterial>);

/// With a window: the ground's material, made once.
pub fn setup_ground(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 1.0,
        ..default()
    });
    commands.insert_resource(GroundMaterial(material));
}

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
    });
    commands.insert_resource(Chunks::default());
    commands.insert_resource(NewSites::default());
    commands.insert_resource(ThingsMade::default());
    commands.insert_resource(crate::structure::MonumentsRes::default());
    // The mesh of the world before is forgotten with it.
    commands.run_system_cached(crate::navmesh::forget);
}

/// The chunk a point is in.
pub fn chunk_of(p: Vec2) -> ChunkKey {
    let t = cell_of(p);
    (t.0.div_euclid(CHUNK), t.1.div_euclid(CHUNK))
}

/// The rectangles of the tiles `of` picks (by its tile and ground), as
/// runs along each row.
fn runs_of(f: (i32, i32), ground: &[Ground], of: impl Fn((i32, i32), Ground) -> bool) -> Vec<Rect> {
    let picked = |x: i32, y: i32| of((f.0 + x, f.1 + y), ground[(y * CHUNK + x) as usize]);
    let mut out = Vec::new();
    for y in 0..CHUNK {
        let mut x = 0;
        while x < CHUNK {
            if !picked(x, y) {
                x += 1;
                continue;
            }
            let start = x;
            while x < CHUNK && picked(x, y) {
                x += 1;
            }
            let min = Vec2::new((f.0 + start) as f32, (f.1 + y) as f32) * TILE;
            let max = Vec2::new((f.0 + x) as f32, (f.1 + y + 1) as f32) * TILE;
            out.push(Rect { min, max });
        }
    }
    out
}

/// One static body of `rects`, each from `bottom` up `height` tall.
fn body(commands: &mut Commands, rects: &[Rect], bottom: f32, height: f32, sight: bool) -> Option<Entity> {
    if rects.is_empty() {
        return None;
    }
    let shapes: Vec<(Vec3, Rotation, Collider)> = rects
        .iter()
        .map(|r| {
            let size = r.max - r.min;
            (
                on_ground((r.min + r.max) / 2.0, bottom + height / 2.0),
                Rotation::IDENTITY,
                Collider::cuboid(size.x, height, size.y),
            )
        })
        .collect();
    let mut e = commands.spawn((WorldPiece, RigidBody::Static, Collider::compound(shapes), Transform::default()));
    if sight {
        e.insert(BlocksSight);
    }
    Some(e.id())
}

/// How thick the ground's slab is, its top the ground (y 0).
const GROUND_THICK: f32 = 0.1;

/// A chunk's ground as a solid slab (docs/pathing.md "A navigation mesh":
/// the navigation mesh is built from colliders, so the ground is one),
/// open over every way down (`structure::Below`).
#[derive(Component)]
pub struct GroundBody(pub ChunkKey);

/// The ground slab of one chunk, none on its open tiles.
fn ground_body(commands: &mut Commands, key: ChunkKey, ground: &[Ground], open: &HashSet<(i32, i32)>) {
    let first = (key.0 * CHUNK, key.1 * CHUNK);
    if let Some(e) = body(commands, &runs_of(first, ground, |t, _| !open.contains(&t)), -GROUND_THICK, GROUND_THICK, false) {
        commands.entity(e).insert(GroundBody(key));
    }
}

/// Make every chunk the world needs now and has not made: round the
/// player, round every person, and at the bunkers.
pub fn make_chunks(
    mut commands: Commands,
    mut world: ResMut<WorldRes>,
    mut chunks: ResMut<Chunks>,
    mut new_sites: ResMut<NewSites>,
    mut things_made: ResMut<ThingsMade>,
    biomes: Res<BiomeRegistryRes>,
    monuments: Res<MonumentRegistryRes>,
    items: Res<crate::content::ItemRegistryRes>,
    ground: Option<Res<GroundMaterial>>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    players: Query<&Transform, With<crate::player::Player>>,
    people: Query<&Transform, (With<crate::actor::Npc>, Without<crate::player::Player>)>,
    below: Query<&crate::structure::Below>,
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
        around(t.translation.ground(), AROUND_PLAYER);
    }
    for t in &people {
        around(t.translation.ground(), AROUND_PERSON);
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
    let open = open_tiles(&below);
    for key in needed {
        if !chunks.made.insert(key) {
            continue;
        }
        let chunk = world.world.chunk(r, key);
        chunks.roads += chunk.ground.iter().filter(|g| **g == Ground::Road).count();
        chunks.ground.insert(key, chunk.ground.to_vec());
        let first = chunk.first();
        body(&mut commands, &runs_of(first, &chunk.ground, |_, g| g == Ground::Water), 0.0, WATER_HEIGHT, false);
        let mut solid = runs_of(first, &chunk.ground, |_, g| matches!(g, Ground::Cliff | Ground::OutOfMap));
        solid.extend(chunk.things.iter().filter(|t| t.source.is_none()).map(|t| t.rect()));
        body(&mut commands, &solid, 0.0, SOLID_HEIGHT, true);
        ground_body(&mut commands, key, &chunk.ground, &open);
        let shore = shore_of(&chunk);
        if !shore.is_empty() {
            chunks.shores.insert(key, shore);
        }
        // The sources (wells, berry bushes) are items standing in the world,
        // through the one `spawn_item`; they stop nobody. Once a world:
        // a chunk made again after a load has its sources in the save.
        let sources = things_made.chunks.insert(key);
        for (k, thing) in chunk.things.iter().enumerate().filter(|_| sources) {
            if let Some(def) = thing.source.as_ref().and_then(|name| items.0.def(name)) {
                let stack = modforge::item::create(def, 1, &[], 0.0, k as u64);
                let at = modforge::walk::centre(thing.tile);
                let (found, _) = crate::interact::spawn_item(&mut commands, def, stack, on_ground(at, 0.0));
                commands.entity(found).insert(WorldPiece);
            }
        }
        if let (Some(ground), Some(meshes)) = (ground.as_ref(), meshes.as_mut()) {
            draw(&mut commands, &chunk, &open, meshes, &ground.0);
        }
        // The sites standing in this chunk, once each.
        let lo = Vec2::new((key.0 * CHUNK) as f32, (key.1 * CHUNK) as f32) * TILE;
        let hi = lo + Vec2::splat(CHUNK as f32 * TILE);
        for site in world.world.sites_in(r, lo, hi) {
            let cell = cell_of(site.position);
            if chunks.sites.insert(cell) {
                let things = things_made.sites.insert(cell);
                new_sites.0.push((site, things));
            }
        }
    }
}

/// A chunk's ground mesh, by the chunk it draws.
#[derive(Component)]
pub struct GroundMesh(pub ChunkKey);

/// Every ground tile left open over a way down (`structure::Below`).
fn open_tiles(below: &Query<&crate::structure::Below>) -> HashSet<(i32, i32)> {
    below.iter().flat_map(|b| b.open.iter().copied()).collect()
}

/// Draw one chunk: its ground, and every thing on it as its model.
fn draw(commands: &mut Commands, chunk: &GroundChunk, open: &HashSet<(i32, i32)>, meshes: &mut Assets<Mesh>, material: &Handle<StandardMaterial>) {
    let key = (chunk.first().0.div_euclid(CHUNK), chunk.first().1.div_euclid(CHUNK));
    draw_ground(commands, key, &chunk.ground, open, meshes, material);
    for thing in &chunk.things {
        let r = thing.rect();
        let at = Transform::from_translation(on_ground((r.min + r.max) / 2.0, 0.0));
        // Its model; never a box (design.md: no coloured boxes).
        match &thing.picture {
            Some(name) => {
                commands.spawn((WorldPiece, Model(name.clone()), at));
            }
            None => warn!("a thing at {:?} has no model", thing.tile),
        }
    }
}

/// A building with a way down stood after the ground under it was made:
/// that ground's slab made and drawn again, open over the way down.
pub fn open_the_ground(
    mut commands: Commands,
    chunks: Option<Res<Chunks>>,
    added: Query<&crate::structure::Below, Added<crate::structure::Below>>,
    below: Query<&crate::structure::Below>,
    drawn: Query<(Entity, &GroundMesh)>,
    slabs: Query<(Entity, &GroundBody)>,
    ground: Option<Res<GroundMaterial>>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
) {
    let Some(chunks) = chunks else {
        return;
    };
    let keys: HashSet<ChunkKey> = added.iter().flat_map(|b| b.open.iter().map(|t| (t.0.div_euclid(CHUNK), t.1.div_euclid(CHUNK)))).collect();
    if keys.is_empty() {
        return;
    }
    let open = open_tiles(&below);
    for key in keys {
        let Some(tiles) = chunks.ground.get(&key) else {
            continue;
        };
        // The slab, open over the way down.
        for (entity, of) in &slabs {
            if of.0 == key {
                commands.entity(entity).despawn();
            }
        }
        ground_body(&mut commands, key, tiles, &open);
        // And, with a window, the ground drawn.
        if let (Some(ground), Some(meshes)) = (ground.as_ref(), meshes.as_mut()) {
            for (entity, of) in &drawn {
                if of.0 == key {
                    commands.entity(entity).despawn();
                }
            }
            draw_ground(&mut commands, key, tiles, &open, meshes, &ground.0);
        }
    }
}

/// One chunk's ground as one flat mesh, a square a tile in its kind's
/// colour, none on the tiles open over a way down.
fn draw_ground(
    commands: &mut Commands,
    key: ChunkKey,
    ground: &[Ground],
    open: &HashSet<(i32, i32)>,
    meshes: &mut Assets<Mesh>,
    material: &Handle<StandardMaterial>,
) {
    let first = (key.0 * CHUNK, key.1 * CHUNK);
    let tiles = (CHUNK * CHUNK) as usize;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(tiles * 4);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(tiles * 4);
    let mut indices: Vec<u32> = Vec::with_capacity(tiles * 6);
    for (i, g) in ground.iter().enumerate() {
        let tile = (first.0 + i as i32 % CHUNK, first.1 + i as i32 / CHUNK);
        if open.contains(&tile) {
            continue;
        }
        let x = tile.0 as f32 * TILE;
        let y = tile.1 as f32 * TILE;
        let [r, gr, b] = ground_color(*g);
        let colour = Color::srgb(r, gr, b).to_linear().to_f32_array();
        let base = positions.len() as u32;
        // The corners a, b, c, d; each triangle wound so its face points up.
        positions.extend([[x, 0.0, y], [x + TILE, 0.0, y], [x + TILE, 0.0, y + TILE], [x, 0.0, y + TILE]]);
        colors.extend([colour; 4]);
        indices.extend([base, base + 3, base + 1, base + 1, base + 3, base + 2]);
    }
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(indices));
    commands.spawn((
        WorldPiece,
        GroundMesh(key),
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(material.clone()),
        Transform::default(),
    ));
}
