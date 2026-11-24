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
    /// What `make_chunks` already asked for (`Asked`).
    asked: Asked,
}

/// What `make_chunks` already asked for, so a step where no one has come
/// into another chunk asks for nothing: each person's chunk (the
/// player's too) when the chunks round them were last asked for, and
/// whether the bunkers' chunks and an edged world's whole range were.
/// Everything asked for is made the step it is asked, and the chunks made
/// only shrink by `unmake`, which forgets this; so the chunks made, and
/// their order, are the same as asking for everything every step. Worked
/// out from where people are, never saved: empty after a load
/// (`forget_asked`), so the first step after one asks for everything.
/// Prior art: Minecraft's server updates a player's loaded chunks only
/// when they cross into another chunk (ChunkMap.move).
#[derive(Default)]
struct Asked {
    round: std::collections::HashMap<Entity, ChunkKey>,
    fixed: bool,
}

/// The chunks asked for forgotten: a load (save.rs `loaded`) gives every
/// person a new entity and may take chunks back.
pub fn forget_asked(world: &mut World) {
    if let Some(mut chunks) = world.get_resource_mut::<Chunks>() {
        chunks.asked = Asked::default();
    }
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
    /// The town's water: its tower and every house's taps.
    pub water: bool,
    /// The town's power: its substation and every house's wiring.
    pub power: bool,
}

/// The chunks made when the game was saved (`Chunks::made` then). Saved;
/// a load into the same world takes back every chunk made since (`unmake`),
/// so the world is as it was, never with chunks the saved game did not
/// have yet.
#[derive(Resource, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct SavedChunks(pub std::collections::BTreeSet<ChunkKey>);

/// The chunks made now, for a save.
pub fn chunks_now(world: &World) -> SavedChunks {
    SavedChunks(world.get_resource::<Chunks>().map(|c| c.made.iter().copied().collect()).unwrap_or_default())
}

/// The chunks `keys` taken back, as if never made: everything they made
/// of the world despawned (`OfChunk`), they and their sites out of
/// `Chunks` and the monuments, so they are made again when needed, their
/// own things with them (`ThingsMade` does not have them). Their meshes go
/// with them (`navmesh::wait_for_chunks`, a chunk no longer made).
pub fn unmake(world: &mut World, keys: &std::collections::BTreeSet<ChunkKey>) {
    if keys.is_empty() {
        return;
    }
    let pieces: Vec<Entity> = world.query::<(Entity, &OfChunk)>().iter(world).filter(|(_, c)| keys.contains(&c.0)).map(|(e, _)| e).collect();
    for piece in pieces {
        if let Ok(piece) = world.get_entity_mut(piece) {
            piece.despawn();
        }
    }
    let of = |cell: (i32, i32)| keys.contains(&(cell.0.div_euclid(CHUNK), cell.1.div_euclid(CHUNK)));
    if let Some(mut chunks) = world.get_resource_mut::<Chunks>() {
        // Chunks no longer made: what was asked for is asked again.
        chunks.asked = Asked::default();
        for key in keys {
            chunks.made.remove(key);
            chunks.ground.remove(key);
            chunks.shores.remove(key);
        }
        chunks.sites.retain(|cell| !of(*cell));
        let roads = chunks.ground.values().flatten().filter(|g| **g == Ground::Road).count();
        chunks.roads = roads;
    }
    if let Some(mut monuments) = world.get_resource_mut::<crate::structure::MonumentsRes>() {
        let due: Vec<Vec2> = monuments.1.iter().filter_map(|i| monuments.0.get(*i).map(|(at, _)| *at)).collect();
        monuments.0.retain(|(at, _)| !of(cell_of(*at)));
        let kept = monuments.0.iter().enumerate().filter(|(_, (at, _))| due.contains(at)).map(|(i, _)| i).collect();
        monuments.1 = kept;
    }
    info!("took back {} chunks made after the save", keys.len());
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
    (run, count): (Res<crate::RunSeed>, Res<crate::BunkerCount>),
) {
    // The seed in the log, so any world can be rolled again.
    info!("rolling the {:?} world from seed {}, the town from {} with {} bunkers", *kind, seed.0, run.0, count.0);
    let world = roll_world2d(*kind, seed.0, (run.0, count.0), &biomes.0, &monuments.monuments)
        .expect("the ground def names registered biomes");
    // The world's bunkers, the player's first.
    let bunkers = world.bunkers.iter().skip(1).map(|c| c.at).collect();
    commands.insert_resource(WorldRes {
        world,
        seed: seed.0,
        bunker: BUNKER_AT,
        bunkers,
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

/// The chunks a person at `p` needs made round them: `AROUND_PLAYER` each
/// way for the player, `AROUND_PERSON` for anyone else. The one rule,
/// read by `make_chunks` and by the world op's check (`not_made_round`).
pub fn round(p: Vec2, is_player: bool) -> impl Iterator<Item = ChunkKey> {
    let c = chunk_of(p);
    let n = if is_player { AROUND_PLAYER } else { AROUND_PERSON };
    (-n..=n).flat_map(move |dy| (-n..=n).map(move |dx| (c.0 + dx, c.1 + dy)))
}

/// How many chunks round the people at `people` ((ground point, is the
/// player)) are not made: none, once each step's chunks are made.
pub fn not_made_round(chunks: &Chunks, people: impl Iterator<Item = (Vec2, bool)>) -> usize {
    let mut not_made: HashSet<ChunkKey> = HashSet::new();
    for (p, is_player) in people {
        not_made.extend(round(p, is_player).filter(|key| !chunks.made.contains(key)));
    }
    not_made.len()
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

/// What one chunk made of the world (its ground, what stands on it, its
/// sites' buildings and props), by the chunk, so a load can take back the
/// chunks made after its save (`unmake`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct OfChunk(pub ChunkKey);

/// One static body of `rects`, each from `bottom` up `height` tall, made
/// by chunk `key`.
fn body(commands: &mut Commands, key: ChunkKey, rects: &[Rect], bottom: f32, height: f32, sight: bool) -> Option<Entity> {
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
    let mut e = commands.spawn((WorldPiece, OfChunk(key), RigidBody::Static, Collider::compound(shapes), Transform::default()));
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
    if let Some(e) = body(commands, key, &runs_of(first, ground, |t, _| !open.contains(&t)), -GROUND_THICK, GROUND_THICK, false) {
        commands.entity(e).insert(GroundBody(key));
    }
}

/// Rivers carved by descent (todo 33, modforge descent): each starts at
/// the highest of a few tiles tried and runs downhill to water or the
/// edge, a ford left every few tiles. Worked out from the world, once a
/// world, never saved. How many, how long and the fords are Claude's.
#[derive(Resource, Default)]
pub struct Rivers {
    pub seed: Option<u64>,
    pub tiles: HashSet<(i32, i32)>,
}

const RIVERS: u64 = 6;
const SOURCE_TRIES: u64 = 12;
const RIVER_MOST: usize = 4000;
const FORD_EVERY: usize = 12;

/// The rivers of a world with an edge (none in a world without one).
fn carve(world: &WorldRes) -> HashSet<(i32, i32)> {
    let ground = &world.world.ground;
    let mut tiles = HashSet::new();
    let Some((lo, hi)) = ground.chunk_range() else {
        return tiles;
    };
    let (lo, span) = (lo * CHUNK, ((hi - lo) * CHUNK).max(1) as u64);
    let end = |t: (i32, i32)| matches!(ground.ground_at(t), Ground::Water | Ground::OutOfMap);
    for k in 0..RIVERS {
        let source = (0..SOURCE_TRIES)
            .map(|a| {
                let h = modforge::ground::tile_hash(world.seed, (k as i32, 0), 0x5249_5645 + a);
                (lo + (h % span) as i32, lo + ((h >> 32) % span) as i32)
            })
            .max_by(|a, b| ground.elevation(*a).total_cmp(&ground.elevation(*b)));
        let Some(source) = source else {
            continue;
        };
        let path = modforge::descent::river(source, |t| ground.elevation(t), end, RIVER_MOST);
        tiles.extend(path.into_iter().enumerate().filter(|(i, _)| !modforge::descent::ford(*i, FORD_EVERY)).map(|(_, t)| t));
    }
    tiles
}

/// Make every chunk the world needs now and has not made: round the
/// player, round every person, and at the bunkers.
pub fn make_chunks(
    mut commands: Commands,
    mut world: ResMut<WorldRes>,
    mut chunks: ResMut<Chunks>,
    mut new_sites: ResMut<NewSites>,
    mut things_made: ResMut<ThingsMade>,
    mut ids: ResMut<crate::interact::ThingIssuer>,
    biomes: Res<BiomeRegistryRes>,
    monuments: Res<MonumentRegistryRes>,
    items: Res<crate::content::ItemRegistryRes>,
    ground: Option<Res<GroundMaterial>>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    players: Query<(Entity, &Transform), With<crate::player::Player>>,
    people: Query<(Entity, &Transform), (With<crate::actor::Npc>, Without<crate::player::Player>)>,
    below: Query<&crate::structure::Below>,
    mut rivers: ResMut<Rivers>,
) {
    if rivers.seed != Some(world.seed) {
        rivers.tiles = carve(&world);
        rivers.seed = Some(world.seed);
    }
    // In key order, whoever is near (authority.md rule 5): the order chunks
    // are made in is the order their things are issued ids, the same after
    // a load.
    let mut needed: std::collections::BTreeSet<ChunkKey> = std::collections::BTreeSet::new();
    // Round each person only when they are new or in another chunk than
    // when last asked (`Asked`): the chunks round where they were are made.
    let last = std::mem::take(&mut chunks.asked.round);
    for (who, t, is_player) in players.iter().map(|(e, t)| (e, t, true)).chain(people.iter().map(|(e, t)| (e, t, false))) {
        let p = t.translation.ground();
        let c = chunk_of(p);
        if last.get(&who) != Some(&c) {
            needed.extend(round(p, is_player));
        }
        chunks.asked.round.insert(who, c);
    }
    if !chunks.asked.fixed {
        chunks.asked.fixed = true;
        for b in std::iter::once(world.bunker).chain(world.bunkers.iter().copied()) {
            let c = chunk_of(b);
            needed.extend((-2..=2).flat_map(|dy| (-2..=2).map(move |dx| (c.0 + dx, c.1 + dy))));
        }
        // A world with an edge is made whole, and the ring of chunks past
        // the edge with it, so the edge stops a body wherever anyone walks.
        if let Some((lo, hi)) = world.world.ground.chunk_range() {
            for y in lo - 1..=hi {
                for x in lo - 1..=hi {
                    needed.insert((x, y));
                }
            }
        }
    }
    needed.retain(|key| !chunks.made.contains(key));
    if needed.is_empty() {
        return;
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
        let mut chunk = world.world.chunk(r, key);
        // The rivers carved through it, roads left as their bridges; what
        // stood there is gone (todo 33).
        let first = chunk.first();
        for y in 0..CHUNK {
            for x in 0..CHUNK {
                let t = (first.0 + x, first.1 + y);
                let i = (y * CHUNK + x) as usize;
                if rivers.tiles.contains(&t) && chunk.ground.get(i).is_some_and(|g| g.is_land() && *g != Ground::Road) {
                    chunk.ground[i] = Ground::Water;
                    chunk.things.retain(|thing| thing.tile != t);
                }
            }
        }
        chunks.roads += chunk.ground.iter().filter(|g| **g == Ground::Road).count();
        chunks.ground.insert(key, chunk.ground.to_vec());
        let first = chunk.first();
        body(&mut commands, key, &runs_of(first, &chunk.ground, |_, g| g == Ground::Water), 0.0, WATER_HEIGHT, false);
        let mut solid = runs_of(first, &chunk.ground, |_, g| matches!(g, Ground::Cliff | Ground::OutOfMap));
        solid.extend(chunk.things.iter().filter(|t| t.source.is_none()).map(|t| t.rect()));
        body(&mut commands, key, &solid, 0.0, SOLID_HEIGHT, true);
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
                let (found, _) = crate::interact::spawn_item(&mut commands, &mut ids, def, stack, on_ground(at, 0.0));
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
pub(crate) fn open_tiles(below: &Query<&crate::structure::Below>) -> HashSet<(i32, i32)> {
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
                commands.spawn((WorldPiece, OfChunk(key), Model(name.clone()), at));
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

/// A world with an edge's bounds on the ground, its lowest and highest
/// corner: its chunks run from lo up to hi, hi the first past it
/// (`make_chunks` makes lo - 1 to hi, the world and the ring past its
/// edge). None for a world with no edge.
pub fn world_bounds(world: &WorldRes) -> Option<((f32, f32), (f32, f32))> {
    let size = CHUNK as f32 * TILE;
    world.world.ground.chunk_range().map(|(lo, hi)| ((lo as f32 * size, lo as f32 * size), (hi as f32 * size, hi as f32 * size)))
}

/// The height the ground is drawn at: the ground, y 0.
pub const GROUND_DRAWN_AT: f32 = 0.0;

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
        let h = GROUND_DRAWN_AT;
        positions.extend([[x, h, y], [x + TILE, h, y], [x + TILE, h, y + TILE], [x, h, y + TILE]]);
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
        OfChunk(key),
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(material.clone()),
        Transform::default(),
    ));
}
