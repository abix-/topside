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
    MonumentDef, Part, PartKind, SLAB, StructureDef, TileKind, TileRun, levels, open_to_below, parts_of, tile_plan,
    tile_plan_at,
};

use modforge::crafting::StationKind;
use modforge::ground::tile_hash;
use modforge::item::{self, create, write};
use modforge::loot::roll_loot;
use modforge::monument::Roll;
use modforge::walk::cell_of;

use crate::content::{
    BENCH_AT, BUNKER_STORE, BUNKER_STORE_AT, COMMON_ROOM_FLOOR, ItemRegistryRes, PLAYER_ROOM_FLOOR, STARTING_ITEMS,
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

/// The lowest a body at `point` goes: the floor of the building it stands
/// in that goes below the ground, else the ground.
pub fn lowest_at(below: &Query<&Below>, point: Vec2) -> f32 {
    below
        .iter()
        .filter(|b| inside(&b.rects, point))
        .map(|b| b.bottom)
        .fold(0.0, f32::min)
}

/// Spawn one building at `at`, every level below the ground and above it,
/// on the tile grid (docs/pathing.md "One grid"): each level from
/// modforge's `tile_plan_at`, standing at its floor's height, its walls as
/// tall as the level, its furniture, a slab under every tile walked on
/// above the lowest floor, a closed door on every door tile run; the stair
/// tower's steps and landings from `parts_of`. Everything solid is one
/// compound collider on a static root; the ground floor is its `Shelter`;
/// what it has below the ground is its `Below`. A building of this
/// reality (`of_world`) is a `WorldPiece`, gone when the storm shifts the
/// world; the bunker is not. Returns the root.
pub fn spawn_structure(commands: &mut Commands, def: &StructureDef, at: Vec2, of_world: bool) -> Entity {
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
    let mut doors: Vec<(Vec2, Vec2, f32, f32)> = Vec::new();
    let levels = levels(def);
    // The lowest floor: the ground, or the deepest level below it.
    let bottom = levels.first().map_or(0.0, |(y, _)| y.min(0.0));
    let mut below: Vec<(Vec2, Vec2)> = Vec::new();
    for (y, height) in levels {
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
            let tall = match r.kind {
                TileKind::Wall => Some(height),
                TileKind::Furniture => Some(FURNITURE_HEIGHT),
                _ => None,
            };
            if let Some(tall) = tall {
                shapes.push((on_ground(centre, y + tall / 2.0), Rotation::IDENTITY, Collider::cuboid(size.x, tall, size.y)));
            }
            // Above the lowest floor, what is walked on stands on a slab.
            if y > bottom + 0.5 && r.kind != TileKind::Wall {
                shapes.push((on_ground(centre, y - SLAB / 2.0), Rotation::IDENTITY, Collider::cuboid(size.x, SLAB, size.y)));
            }
            match r.kind {
                TileKind::Door => doors.push((at + centre, Vec2::X * size.x, y, height)),
                kind => {
                    let (model, scale) = match kind {
                        TileKind::Floor => ("floor", Vec3::ONE),
                        TileKind::Furniture => ("furniture", Vec3::ONE),
                        _ => ("wall", Vec3::new(1.0, height / WALL_HEIGHT, 1.0)),
                    };
                    let tint = Tint(Color::srgb(r.color[0], r.color[1], r.color[2]));
                    for x in r.from..r.to {
                        let tile = at + Vec2::new(x as f32 + 0.5, r.row as f32 + 0.5);
                        pieces.push((Model(model.to_string()), tint, Transform::from_translation(on_ground(tile, y)).with_scale(scale)));
                    }
                }
            }
        }
    }
    // The stair tower's steps and landings, and the slabs it is walked on
    // at every level (modforge's parts), above the ground and below.
    let walked = |p: &&Part| {
        let thin_furniture = p.kind == PartKind::Furniture && p.size.y <= 2.0 * SLAB;
        matches!(p.kind, PartKind::Step | PartKind::Landing) || thin_furniture
    };
    for part in parts_of(def).iter().filter(walked) {
        let local = Vec2::new(part.center.x, part.center.z);
        shapes.push((
            on_ground(local, part.center.y),
            Rotation::IDENTITY,
            Collider::cuboid(part.size.x, part.size.y, part.size.z),
        ));
        // Drawn as the furniture block, stretched to the part.
        let scale = Vec3::new(part.size.x / 0.9, part.size.y / FURNITURE_HEIGHT, part.size.z / 0.9);
        pieces.push((
            Model("furniture".to_string()),
            Tint(Color::srgb(part.color[0], part.color[1], part.color[2])),
            Transform::from_translation(on_ground(at + local, part.center.y - part.size.y / 2.0)).with_scale(scale),
        ));
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
    for (centre, along, y, height) in doors {
        let mut door = commands.spawn((
            Door {
                open: false,
                closed_center: centre,
                // The hinge is at one end of the door, along the run.
                hinge: centre - along / 2.0,
            },
            Interactable {
                kind: InteractKind::Door { open: false },
            },
            RigidBody::Static,
            // Standing on its floor: the box raised by half its height.
            Collider::compound(vec![(
                Vec3::Y * height / 2.0,
                Rotation::IDENTITY,
                Collider::cuboid(along.x.max(1.0), height, 1.0),
            )]),
            Model("door".to_string()),
            // Not scaled: Avian would scale its collider too.
            Transform::from_translation(on_ground(centre, y)),
        ));
        if of_world {
            door.insert(WorldPiece);
        }
    }
    root
}

/// The starting bunker at the world's bunker point (design.md "The bunker
/// you start in"): the starting items and the note on the floor of the
/// player's room, the bench in the common room.
pub fn spawn_bunker(
    mut commands: Commands,
    world: Res<WorldRes>,
    items: Res<ItemRegistryRes>,
    layers: Res<crate::content::LayerRegistryRes>,
) {
    let at = world.bunker;
    spawn_structure(&mut commands, &starting_bunker(), at, false);
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
        spawn_item(&mut commands, def(name), stack, on_ground(at + Vec2::new(x, z), PLAYER_ROOM_FLOOR));
    }
    // The crafting bench, where E opens its recipes.
    let [x, z] = BENCH_AT;
    spawn_station(&mut commands, StationKind::CraftingBench, on_ground(at + Vec2::new(x, z), COMMON_ROOM_FLOOR));
    // The note and the world both come from the world's seed.
    let [x, z] = STARTING_NOTE_AT;
    spawn_item(
        &mut commands,
        def("note"),
        write(def("note"), starting_note(world.seed), 0.0, 13).expect("the note def is a note"),
        on_ground(at + Vec2::new(x, z), PLAYER_ROOM_FLOOR),
    );
}

/// The other bunkers (design.md: every bunker is its own faction), each
/// the same building as the player's through `spawn_structure`, not of
/// any one world, with its store: a box of food and water.
pub fn spawn_other_bunkers(mut commands: Commands, world: Res<WorldRes>, items: Res<ItemRegistryRes>) {
    let box_def = items.0.def("storage box").expect("the storage box is registered");
    for (i, &at) in world.bunkers.iter().enumerate() {
        spawn_structure(&mut commands, &starting_bunker(), at, false);
        let mut contents = item::Inventory::new(box_def.storage.unwrap_or(12));
        for (name, count) in BUNKER_STORE {
            if let Some(def) = items.0.def(name) {
                let _ = contents.add(create(def, count, &[], 0.0, i as u64), def.max_stack);
            }
        }
        let [x, z] = BUNKER_STORE_AT;
        let store = spawn_item(
            &mut commands,
            box_def,
            create(box_def, 1, &[], 0.0, i as u64),
            // In the entrance, where its people can reach it until they can
            // go down the stairs (todo 13a2).
            on_ground(at + Vec2::new(x, z), 0.0),
        );
        commands.entity(store).insert(Container(contents));
    }
}

/// Every spawned site's monument as modforge rolled it, and where it
/// stands, read by whatever needs its NPC spots or loot spots; and how
/// many of them have had their people put in.
#[derive(Resource, Default)]
pub struct MonumentsRes(pub Vec<(Vec2, MonumentDef)>, pub usize);

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
) {
    let table = loot_table();
    for site in new_sites.0.drain(..) {
        let monument = site.rolled.clone();
        for member in &monument.members {
            let at = site.position + Vec2::new(member.offset.x, member.offset.z);
            spawn_structure(&mut commands, &member.structure, at, true);
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
                        commands.spawn((WorldPiece, Model(name.clone()), at));
                    }
                    None => warn!("a prop of {} has no model", monument.name),
                }
            }
        }
        // A box at every loot spot, filled by the spot's danger from the
        // one loot table (modforge's roll, seeded by the site).
        if let Some(box_def) = items.0.def("storage box") {
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
                let at = on_ground(site.position + Vec2::new(spot.position.x, spot.position.z), spot.position.y);
                let found = spawn_item(&mut commands, box_def, item::create(box_def, 1, &[], 0.0, k as u64), at);
                commands.entity(found).insert((Container(contents), WorldPiece));
            }
        }
        monuments.0.push((site.position, monument));
    }
}
