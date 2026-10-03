//! Buildings on the tile grid (pathing.md "One grid"): modforge turns a
//! `StructureDef` into runs of tiles (`tile_plan`: floor, wall, door,
//! furniture); topside stands each run up to its height, draws it, and
//! gives the solid ones one static collider per building (the 3D game's
//! rule, one collider per building). Structure-local x and z are the
//! world's ground x and y. The ground floor only, as before.

use avian3d::prelude::*;
use bevy::prelude::*;
use modforge::hud::InteractKind;
use modforge::structure::{
    DOORWAY_HEIGHT, DoorBlock, MonumentDef, Part, PartKind, SLAB, StructureDef, TileKind, TileRun, door_blocks, levels,
    open_to_below, parts_of, tile_plan, tile_plan_at,
};

use modforge::crafting::StationKind;
use modforge::ground::tile_hash;
use modforge::item::{self, create, write};
use modforge::loot::roll_loot;
use modforge::monument::Roll;
use modforge::walk::cell_of;

use crate::content::{
    BENCH_AT, BUNKER_STORE, BUNKER_STORE_AT, ItemRegistryRes, PLAYER_ROOM_FLOOR, STARTING_ITEMS, SURFACE_ROOM_FLOOR,
    STARTING_NOTE_AT, loot_table,
    starting_bunker, starting_note,
};
use crate::interact::{Container, Door, Interactable, spawn_item, spawn_station};
use crate::model::{Model, Tint};
use crate::on_ground;
use crate::terrain::{NewSites, WorldPiece, WorldRes};

/// How tall a wall and a door stand, in metres (a room's height in
/// modforge's starting bunker).
pub const WALL_HEIGHT: f32 = 3.0;
/// How tall furniture stands, in metres: under the eyes, so it is seen
/// over.
pub const FURNITURE_HEIGHT: f32 = 0.9;
/// How thick the floor model is, about its place, in metres
/// (scripts/models.py `floor`).
pub const FLOOR_TILE: f32 = 0.02;

/// The 3D point on a building's floor over the ground point `g`, the floor
/// of the level at height `level` (0 the ground floor): on the top of the
/// level's slab, which lies on the level, so a ground floor stands above
/// the ground by the slab's thickness (operator, 2026-10-03: the floor sits
/// above the ground, never in it). Everything standing on a floor (people,
/// furniture, doors, items) stands here.
pub fn on_floor(g: Vec2, level: f32) -> Vec3 {
    on_ground(g, level + SLAB)
}

/// A building's root: the body holding its one collider.
#[derive(Component)]
pub struct StructureRoot;

/// Where a building has a roof, in world metres: its floors. Anyone
/// standing inside one is sheltered from the storm.
#[derive(Component, Default)]
pub struct Shelter(pub Vec<(Vec2, Vec2)>);

impl Shelter {
    /// Whether `point` is under this building's roof.
    pub fn covers(&self, point: Vec2) -> bool {
        inside(&self.0, point)
    }
}

/// Whether `point` is in any of `rects` (world metres, min and max).
fn inside(rects: &[(Vec2, Vec2)], point: Vec2) -> bool {
    rects.iter().any(|(min, max)| point.x >= min.x && point.x <= max.x && point.y >= min.y && point.y <= max.y)
}

/// Whether `point` is under any roof.
pub fn sheltered(shelters: &Query<&Shelter>, point: Vec2) -> bool {
    shelters.iter().any(|s| s.covers(point))
}

/// What a building has below the ground: where its levels below stand
/// (world metres, every tile of them), the height of its lowest floor, and
/// the ground tiles it leaves open over the way down (world tiles). A body
/// inside it goes as low as that floor (`player::movement`); no ground is
/// drawn over the open tiles (`terrain`).
#[derive(Component)]
pub struct Below {
    pub rects: Vec<(Vec2, Vec2)>,
    pub bottom: f32,
    pub open: Vec<(i32, i32)>,
}

/// The lowest a body at `point` goes: the top of the lowest floor of the
/// building it stands in that goes below the ground, else the ground.
pub fn lowest_at(below: &Query<&Below>, point: Vec2) -> f32 {
    below
        .iter()
        .filter(|b| inside(&b.rects, point))
        .map(|b| on_floor(point, b.bottom).y)
        .fold(0.0, f32::min)
}

/// Spawn one building at `at`, every level below the ground and above it,
/// on the tile grid (docs/pathing.md "One grid"): each level from
/// modforge's `tile_plan_at`, standing at its floor's height, its walls as
/// tall as the level, its furniture, a slab under every tile walked on
/// above the lowest floor, every door block (modforge `door_blocks`) its
/// frame and a closed door of the model `door_model` ("steel door" the
/// bunker's, "door" in the wild; None when its doors come from a save,
/// the building made again after a load); the stair tower's steps and landings
/// from `parts_of`. Everything solid is one
/// compound collider on a static root; the ground floor is its `Shelter`;
/// what it has below the ground is its `Below`. A building of this
/// reality (`of_world`) is a `WorldPiece`, gone when the storm shifts the
/// world; the bunker is not. Returns the root.
pub fn spawn_structure(commands: &mut Commands, ids: &mut crate::interact::ThingIssuer, def: &StructureDef, at: Vec2, of_world: bool, door_model: Option<&str>) -> Entity {
    let at = at.round();
    let rect = |r: &TileRun| {
        (
            Vec2::new(r.from as f32, r.row as f32),
            Vec2::new(r.to as f32, r.row as f32 + 1.0),
        )
    };
    let shelter = Shelter(
        tile_plan(def)
            .iter()
            .filter(|r| matches!(r.kind, TileKind::Floor | TileKind::Furniture))
            .map(|r| {
                let (min, max) = rect(r);
                (at + min, at + max)
            })
            .collect(),
    );
    let mut shapes: Vec<(Vec3, Rotation, Collider)> = Vec::new();
    let mut pieces: Vec<(Model, Tint, Transform)> = Vec::new();
    let levels = levels(def);
    // The lowest floor: the ground, or the deepest level below it.
    let bottom = levels.first().map_or(0.0, |(y, _)| y.min(0.0));
    let mut below: Vec<(Vec2, Vec2)> = Vec::new();
    for &(y, height) in &levels {
        let runs = tile_plan_at(def, y);
        if y < -0.5 {
            below.extend(runs.iter().map(|r| {
                let (min, max) = rect(r);
                (at + min, at + max)
            }));
        }
        for r in &runs {
            let (min, max) = rect(r);
            let size = max - min;
            let centre = (min + max) / 2.0;
            // Every level, the ground floor too, stands on its slab: the slab
            // lies on the level and what stands on the level stands on its
            // top (`on_floor`), so no floor lies in the ground (operator,
            // 2026-10-03), and the navigation mesh sees every floor. A wall
            // stands from the level up, the slab's thickness taller, so
            // nothing shows under it.
            // A hatch's hole has neither: its door lies over the way down.
            if r.kind == TileKind::Hatch {
                continue;
            }
            let floor_tint = Tint(Color::srgb(def.floor_color[0], def.floor_color[1], def.floor_color[2]));
            let tint = Tint(Color::srgb(r.color[0], r.color[1], r.color[2]));
            if r.kind == TileKind::Wall {
                let tall = height + SLAB;
                shapes.push((on_ground(centre, y + tall / 2.0), Rotation::IDENTITY, Collider::cuboid(size.x, tall, size.y)));
            } else {
                shapes.push((on_ground(centre, y + SLAB / 2.0), Rotation::IDENTITY, Collider::cuboid(size.x, SLAB, size.y)));
            }
            if r.kind == TileKind::Furniture {
                shapes.push((on_floor(centre, y) + Vec3::Y * (FURNITURE_HEIGHT / 2.0), Rotation::IDENTITY, Collider::cuboid(size.x, FURNITURE_HEIGHT, size.y)));
            }
            for x in r.from..r.to {
                let tile = at + Vec2::new(x as f32 + 0.5, r.row as f32 + 0.5);
                if r.kind == TileKind::Wall {
                    pieces.push((Model("wall".to_string()), tint, Transform::from_translation(on_ground(tile, y)).with_scale(Vec3::new(1.0, (height + SLAB) / WALL_HEIGHT, 1.0))));
                    continue;
                }
                // The slab, drawn as the floor tile stretched to its
                // thickness (the floor model is FLOOR_TILE thick about its
                // place); the doors themselves stand below, from the
                // doorways.
                let colour = if r.kind == TileKind::Floor { tint } else { floor_tint };
                pieces.push((Model("floor".to_string()), colour, Transform::from_translation(on_ground(tile, y + SLAB / 2.0)).with_scale(Vec3::new(1.0, SLAB / FLOOR_TILE, 1.0))));
                if r.kind == TileKind::Furniture {
                    pieces.push((Model("furniture".to_string()), tint, Transform::from_translation(on_floor(tile, y))));
                }
            }
        }
    }
    // The stairs' steps and landings, and the slabs a stair tower is walked
    // on at every level (modforge's parts), above the ground and below.
    let walked = |p: &&Part| {
        let thin_furniture = p.kind == PartKind::Furniture && p.size.y <= 2.0 * SLAB;
        matches!(p.kind, PartKind::Step | PartKind::Landing) || thin_furniture
    };
    // Each from the floor it rises from (`on_floor`): a flight climbs from
    // one level's slab top to the next's, its landing level with it.
    for part in parts_of(def).iter().filter(walked) {
        let local = Vec2::new(part.center.x, part.center.z);
        shapes.push((
            on_floor(local, part.center.y),
            Rotation::IDENTITY,
            Collider::cuboid(part.size.x, part.size.y, part.size.z),
        ));
        // Drawn as the furniture block, stretched to the part.
        let scale = Vec3::new(part.size.x / 0.9, part.size.y / FURNITURE_HEIGHT, part.size.z / 0.9);
        pieces.push((
            Model("furniture".to_string()),
            Tint(Color::srgb(part.color[0], part.color[1], part.color[2])),
            Transform::from_translation(on_floor(at + local, part.center.y - part.size.y / 2.0)).with_scale(scale),
        ));
    }
    // Every door block (modforge `door_blocks`): its frame, two posts either
    // side of the doorway and a head over it up to the level's top, solid
    // and drawn as the building's wall, filling the wall tile's depth.
    let blocks: Vec<(DoorBlock, f32, f32)> = door_blocks(def)
        .into_iter()
        .filter_map(|b| {
            let &(level, tall) = levels.iter().find(|(y, h)| b.floor > y - 0.5 && b.floor < y + h - 0.5)?;
            let floor = b.floor;
            Some((b, floor, level + tall - floor))
        })
        .collect();
    let wall_tint = Tint(Color::srgb(def.wall_color[0], def.wall_color[1], def.wall_color[2]));
    for (block, floor, height) in &blocks {
        // A piece of the frame: its middle on the ground, its width along the
        // wall, its bottom and how tall.
        let mut frame = |middle: Vec2, along: f32, bottom: f32, tall: f32| {
            if tall <= 0.0 || along <= 0.0 {
                return;
            }
            let size = block.along * along + Vec2::new(block.along.y, block.along.x);
            shapes.push((on_ground(middle, bottom + tall / 2.0), Rotation::IDENTITY, Collider::cuboid(size.x, tall, size.y)));
            pieces.push((
                Model("wall".to_string()),
                wall_tint,
                Transform::from_translation(on_ground(at + middle, bottom)).with_scale(Vec3::new(size.x, tall / WALL_HEIGHT, size.y)),
            ));
        };
        // From the floor's top (`on_floor`) to the level's top.
        let floor = on_floor(Vec2::ZERO, *floor).y;
        for (middle, along) in block.posts() {
            frame(middle, along, floor, *height);
        }
        frame(block.centre, block.doorway(), floor + DOORWAY_HEIGHT, height - DOORWAY_HEIGHT);
    }
    let mut root = commands.spawn((
        StructureRoot,
        shelter,
        RigidBody::Static,
        Transform::from_translation(on_ground(at, 0.0)),
    ));
    if !shapes.is_empty() {
        root.insert(Collider::compound(shapes));
    }
    if of_world {
        root.insert(WorldPiece);
    }
    if bottom < -0.5 {
        let open = open_to_below(def).into_iter().map(|(x, z)| (at.x as i32 + x, at.y as i32 + z)).collect();
        root.insert(Below { rects: below, bottom, open });
    }
    let root = root.id();
    for piece in pieces {
        let mut piece = commands.spawn(piece);
        if of_world {
            piece.insert(WorldPiece);
        }
    }
    // Every door in its doorway, hung on its block's hinge (modforge
    // `door_blocks`), the model of the building's doors. The door's own
    // place is the hinge; the panel and its collider reach from it across
    // the doorway, so opening is a quarter turn about the hinge. Made
    // again after a load, its doors come from the save.
    // Every hatch lying in its floor over its hole (modforge `Hatch`), the
    // same door.
    let Some(door_model) = door_model else {
        return root;
    };
    let standing = blocks.iter().map(|(block, floor, _)| {
        let (centre, hinge) = (at + block.centre, at + block.hinge());
        let door = Door {
            open: false,
            closed_center: centre,
            hinge,
            closed: Door::facing(hinge, centre),
            width: block.doorway(),
            height: DOORWAY_HEIGHT,
            lying: false,
            model: door_model.to_string(),
        };
        (door, *floor)
    });
    let lying = def.hatches.iter().map(|h| (Door::hatch(h, at, door_model), h.level));
    for (made, floor) in standing.chain(lying).collect::<Vec<_>>() {
        // On its floor at the hinge; its body and panel are made from it
        // (interact::door_body).
        let place = Transform::from_translation(on_floor(made.hinge, floor)).with_rotation(made.closed);
        let mut door = commands.spawn((
            made,
            Interactable {
                kind: InteractKind::Door { open: false },
            },
            ids.issue(),
            place,
        ));
        if of_world {
            door.insert(WorldPiece);
        }
    }
    root
}

/// Where the player's bunker door stands on the ground (the starting
/// bunker's front door), and which way it faces out.
pub fn bunker_door(bunker: Vec2) -> (Vec2, Vec2) {
    // The front door: of the bunker's door blocks on the ground (modforge
    // `door_blocks`, what its doors are built from), the one with a side
    // outside every room on the ground, a step through its wall; a door
    // between two rooms has a room on both sides. It faces that way out.
    let plan = starting_bunker();
    let in_a_room = |p: Vec2| {
        plan.rooms.iter().any(|r| {
            r.origin.y <= 0.01 && r.origin.y + r.interior.y > 0.01 && (p.x - r.origin.x).abs() < r.interior.x / 2.0 && (p.y - r.origin.z).abs() < r.interior.z / 2.0
        })
    };
    let front = door_blocks(&plan).into_iter().filter(|b| b.floor.abs() < 0.01).find_map(|b| {
        let through = b.along.perp();
        [through, -through].into_iter().find(|out| !in_a_room(b.centre + *out)).map(|out| (b.centre, out))
    });
    let (door, out) = front.unwrap_or((Vec2::ZERO, Vec2::Y));
    (bunker + door, out)
}

/// The bunker's doors are steel; doors in the wild are wood for now
/// (design.md "Built in 1 m blocks, doors as blocks").
pub const BUNKER_DOOR: &str = "steel door";
pub const WILD_DOOR: &str = "door";

/// The starting bunker at the world's bunker point (design.md "The bunker
/// you start in"): the starting items and the note on the floor of the
/// player's room, the bench in the surface room.
pub fn spawn_bunker(
    mut commands: Commands,
    world: Res<WorldRes>,
    items: Res<ItemRegistryRes>,
    layers: Res<crate::content::LayerRegistryRes>,
    mut ids: ResMut<crate::interact::ThingIssuer>,
) {
    let at = world.bunker;
    spawn_structure(&mut commands, &mut ids, &starting_bunker(), at, false, Some(BUNKER_DOOR));
    let def = |name: &str| {
        items
            .0
            .def(name)
            .unwrap_or_else(|| panic!("item '{name}' is not registered"))
    };
    for (name, count, salt, [x, z], _) in STARTING_ITEMS {
        let on: Vec<String> = crate::content::starting_layers(name).iter().map(|l| l.to_string()).collect();
        let stack = item::create_layered(def(name), &on, &layers.0, count, &[], 0.0, salt)
            .unwrap_or_else(|e| panic!("starting item {name}: {e}"));
        spawn_item(&mut commands, &mut ids, def(name), stack, on_floor(at + Vec2::new(x, z), PLAYER_ROOM_FLOOR));
    }
    // Its store, as every bunker's (Roxanne's, and anyone who joins).
    spawn_store(&mut commands, &mut ids, &items, at, PLAYER_STORE_SALT);
    // The crafting bench, where E opens its recipes.
    let [x, z] = BENCH_AT;
    spawn_station(&mut commands, &mut ids, StationKind::CraftingBench, on_floor(at + Vec2::new(x, z), SURFACE_ROOM_FLOOR));
    // The note and the world both come from the world's seed.
    let [x, z] = STARTING_NOTE_AT;
    spawn_item(
        &mut commands,
        &mut ids,
        def("note"),
        write(def("note"), starting_note(world.seed), 0.0, 13).expect("the note def is a note"),
        on_floor(at + Vec2::new(x, z), PLAYER_ROOM_FLOOR),
    );
}

/// The other bunkers (design.md: every bunker is its own faction), each
/// the same building as the player's through `spawn_structure`, not of
/// any one world, with its store: a box of food and water.
pub fn spawn_other_bunkers(mut commands: Commands, world: Res<WorldRes>, items: Res<ItemRegistryRes>, mut ids: ResMut<crate::interact::ThingIssuer>) {
    for (i, &at) in world.bunkers.iter().enumerate() {
        spawn_structure(&mut commands, &mut ids, &starting_bunker(), at, false, Some(BUNKER_DOOR));
        spawn_store(&mut commands, &mut ids, &items, at, i as u64);
    }
}

/// The salt the player's bunker's store is made with (the other bunkers'
/// are their places in the world's list).
const PLAYER_STORE_SALT: u64 = 1000;

/// A bunker's store (design.md "The bunker you start in": the water store):
/// a box of food and water, every bunker's the same way, the player's too.
fn spawn_store(commands: &mut Commands, ids: &mut crate::interact::ThingIssuer, items: &ItemRegistryRes, bunker: Vec2, salt: u64) {
    let box_def = items.0.def("storage box").expect("the storage box is registered");
    let mut contents = item::Inventory::new(box_def.storage.unwrap_or(12));
    for (name, count) in BUNKER_STORE {
        if let Some(def) = items.0.def(name) {
            let _ = contents.add(create(def, count, &[], 0.0, salt), def.max_stack);
        }
    }
    let [x, z] = BUNKER_STORE_AT;
    let (store, _) = spawn_item(
        commands,
        ids,
        box_def,
        create(box_def, 1, &[], 0.0, salt),
        // In the surface room, where its people live (design.md "The
        // bunker you start in").
        on_floor(bunker + Vec2::new(x, z), SURFACE_ROOM_FLOOR),
    );
    commands.entity(store).insert(Container(contents));
}

/// Every spawned site's monument as modforge rolled it, and where it
/// stands, read by whatever needs its NPC spots or loot spots; and those
/// (by their place in the list) whose people are still to be put in
/// (none for a site made again after a load: its people are in the save).
#[derive(Resource, Default)]
pub struct MonumentsRes(pub Vec<(Vec2, MonumentDef)>, pub Vec<usize>);

/// Spawn the sites of the chunks just made: each member building's ground
/// floor through `spawn_structure`, and its props as one static collider.
/// The monument is the one world generation rolled with its site
/// (design.md "A valid world"), never rolled again here.
pub fn spawn_sites(
    mut commands: Commands,
    world: Res<WorldRes>,
    mut new_sites: ResMut<NewSites>,
    mut monuments: ResMut<MonumentsRes>,
    items: Res<ItemRegistryRes>,
    layers: Res<crate::content::LayerRegistryRes>,
    mut ids: ResMut<crate::interact::ThingIssuer>,
) {
    // Built only when a site is new, not every step.
    if new_sites.0.is_empty() {
        return;
    }
    let table = loot_table();
    for (site, things) in new_sites.0.drain(..) {
        let monument = site.rolled.clone();
        // Its doors, loot boxes and people made once a world; made again
        // after a load, they come from the save.
        // Its buildings and props are its chunk's, taken back with it.
        let of_chunk = crate::terrain::OfChunk(crate::terrain::chunk_of(site.position));
        for member in &monument.members {
            let at = site.position + Vec2::new(member.offset.x, member.offset.z);
            let root = spawn_structure(&mut commands, &mut ids, &member.structure, at, true, things.then_some(WILD_DOOR));
            commands.entity(root).insert(of_chunk);
        }
        if !monument.props.is_empty() {
            let shapes: Vec<(Vec3, Rotation, Collider)> = monument
                .props
                .iter()
                .map(|p| {
                    (
                        on_ground(Vec2::new(p.position.x, p.position.z), p.size.y / 2.0),
                        Rotation::IDENTITY,
                        Collider::cuboid(p.size.x, p.size.y, p.size.z),
                    )
                })
                .collect();
            commands.spawn((
                WorldPiece,
                of_chunk,
                RigidBody::Static,
                Collider::compound(shapes),
                Transform::from_translation(on_ground(site.position, 0.0)),
            ));
            for p in &monument.props {
                let centre = site.position + Vec2::new(p.position.x, p.position.z);
                let at = Transform::from_translation(on_ground(centre, 0.0));
                // Its model; never a box (design.md: no coloured boxes).
                match &p.picture {
                    Some(name) => {
                        commands.spawn((WorldPiece, of_chunk, Model(name.clone()), at));
                    }
                    None => warn!("a prop of {} has no model", monument.name),
                }
            }
        }
        // A box at every loot spot, filled by the spot's danger from the
        // one loot table (modforge's roll, seeded by the site).
        if let Some(box_def) = items.0.def("storage box").filter(|_| things) {
            for (k, spot) in monument.loot_spots.iter().enumerate() {
                let mut roll = Roll::new(tile_hash(world.seed, cell_of(site.position), k as u64));
                let mut contents = item::Inventory::new(box_def.storage.unwrap_or(12));
                for (name, on, count) in roll_loot(&table, spot.danger, &mut roll) {
                    let Some(def) = items.0.def(&name) else {
                        continue;
                    };
                    match item::create_layered(def, &on, &layers.0, count, &[], 0.0, k as u64) {
                        Ok(stack) => {
                            let _ = contents.add(stack, def.max_stack);
                        }
                        Err(e) => warn!("loot: {e}"),
                    }
                }
                // On the floor its spot is on.
                let at = on_floor(site.position + Vec2::new(spot.position.x, spot.position.z), spot.position.y);
                let (found, _) = spawn_item(&mut commands, &mut ids, box_def, item::create(box_def, 1, &[], 0.0, k as u64), at);
                commands.entity(found).insert((Container(contents), WorldPiece));
            }
        }
        if things {
            let next = monuments.0.len();
            monuments.1.push(next);
        }
        monuments.0.push((site.position, monument));
    }
}
