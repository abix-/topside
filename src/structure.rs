//! Buildings seen from above, one floor at a time (design.md "What 2D
//! changes"), on the tile grid (pathing.md "One grid"): modforge turns
//! a `StructureDef` into runs of tiles (`tile_plan`: floor, wall, door,
//! furniture); topside only draws the runs and gives the solid ones one
//! static collider per building (the 3D game's rule, one collider per
//! building). Structure-local x and z are the screen's x and y, the
//! same as the world's.

use avian2d::prelude::*;
use bevy::prelude::*;
use modforge::hud::InteractKind;
use modforge::structure::{MonumentDef, Rgb, StructureDef, TileKind, TileRun, tile_plan};

use modforge::crafting::StationKind;
use modforge::ground::tile_hash;
use modforge::item::{self, create, write};
use modforge::loot::roll_loot;
use modforge::monument::Roll;
use modforge::walk::cell_of;

use crate::content::{
    BENCH_AT, BUNKER_STORE, BUNKER_STORE_AT, ItemRegistryRes, STARTING_ITEMS,
    STARTING_NOTE_AT, loot_table,
    starting_bunker, starting_note,
};
use crate::interact::{Container, Door, Interactable, spawn_item, spawn_station};
use crate::terrain::{NewSites, WorldPiece, WorldRes};

/// Drawing order: floors under walls and furniture, doors on top.
const FLOOR_Z: f32 = 0.2;
const SOLID_Z: f32 = 0.6;
const DOOR_Z: f32 = 0.7;
const DOOR_COLOR: Rgb = [0.35, 0.25, 0.15];

/// A building's root: the body holding its one collider.
#[derive(Component)]
pub struct StructureRoot;

fn sprite(rgb: Rgb, size: Vec2) -> Sprite {
    Sprite {
        color: Color::srgb(rgb[0], rgb[1], rgb[2]),
        custom_size: Some(size),
        ..default()
    }
}

/// Where a building has a roof, in world metres: its floors. Anyone
/// standing inside one is sheltered from the storm.
#[derive(Component, Default)]
pub struct Shelter(pub Vec<(Vec2, Vec2)>);

impl Shelter {
    /// Whether `point` is under this building's roof.
    pub fn covers(&self, point: Vec2) -> bool {
        self.0
            .iter()
            .any(|(min, max)| point.x >= min.x && point.x <= max.x && point.y >= min.y && point.y <= max.y)
    }
}

/// Whether `point` is under any roof.
pub fn sheltered(shelters: &Query<&Shelter>, point: Vec2) -> bool {
    shelters.iter().any(|s| s.covers(point))
}

/// Spawn one building's ground floor at `at`, on the tile grid
/// (docs/pathing.md "One grid"): the building snaps to whole tiles and
/// comes from modforge's `tile_plan`, every run of tiles drawn, its wall
/// and furniture runs as one compound collider on a static root, its
/// floor as its `Shelter`, and a closed door on every door tile run. A
/// building of this reality (`of_world`) is a `WorldPiece`, gone when
/// the storm shifts the world; the bunker is not. Returns the root.
pub fn spawn_structure(commands: &mut Commands, def: &StructureDef, at: Vec2, of_world: bool) -> Entity {
    let at = at.round();
    let runs = tile_plan(def);
    let rect = |r: &TileRun| {
        (
            Vec2::new(r.from as f32, r.row as f32),
            Vec2::new(r.to as f32, r.row as f32 + 1.0),
        )
    };
    let shelter = Shelter(
        runs.iter()
            .filter(|r| matches!(r.kind, TileKind::Floor | TileKind::Furniture))
            .map(|r| {
                let (min, max) = rect(r);
                (at + min, at + max)
            })
            .collect(),
    );
    let shapes: Vec<(Vec2, Rotation, Collider)> = runs
        .iter()
        .filter(|r| r.kind.solid())
        .map(|r| {
            let (min, max) = rect(r);
            let size = max - min;
            ((min + max) / 2.0, Rotation::IDENTITY, Collider::rectangle(size.x, size.y))
        })
        .collect();
    let mut root = commands.spawn((
        StructureRoot,
        shelter,
        RigidBody::Static,
        Transform::from_translation(at.extend(0.0)),
    ));
    if !shapes.is_empty() {
        root.insert(Collider::compound(shapes));
    }
    if of_world {
        root.insert(WorldPiece);
    }
    let root = root.id();
    for r in runs.iter().filter(|r| r.kind != TileKind::Door) {
        let z = if r.kind == TileKind::Floor { FLOOR_Z } else { SOLID_Z };
        let (min, max) = rect(r);
        let mut piece = commands.spawn((
            sprite(r.color, max - min),
            Transform::from_translation((at + (min + max) / 2.0).extend(z)),
        ));
        if of_world {
            piece.insert(WorldPiece);
        }
    }
    for r in runs.iter().filter(|r| r.kind == TileKind::Door) {
        let (min, max) = rect(r);
        let size = max - min;
        let centre = at + (min + max) / 2.0;
        // The hinge is at one end of the door, along the run.
        let along = Vec2::X * size.x;
        let mut door = commands.spawn((
            Door {
                open: false,
                closed_center: centre,
                hinge: centre - along / 2.0,
            },
            Interactable {
                kind: InteractKind::Door { open: false },
            },
            RigidBody::Static,
            Collider::rectangle(size.x, size.y),
            sprite(DOOR_COLOR, size),
            Transform::from_translation(centre.extend(DOOR_Z)),
        ));
        if of_world {
            door.insert(WorldPiece);
        }
    }
    root
}

/// The starting bunker at the world's bunker point, with its starting
/// items and the note on the floor.
pub fn spawn_bunker(mut commands: Commands, world: Res<WorldRes>, items: Res<ItemRegistryRes>) {
    let at = world.bunker;
    spawn_structure(&mut commands, &starting_bunker(), at, false);
    let def = |name: &str| {
        items
            .0
            .def(name)
            .unwrap_or_else(|| panic!("item '{name}' is not registered"))
    };
    for (name, count, salt, [x, z], color) in STARTING_ITEMS {
        spawn_item(
            &mut commands,
            def(name),
            create(def(name), count, &[], 0.0, salt),
            at + Vec2::new(x, z),
            color,
        );
    }
    // The crafting bench, where E opens its recipes.
    let [x, z] = BENCH_AT;
    spawn_station(&mut commands, StationKind::CraftingBench, at + Vec2::new(x, z));
    // The note and the world both come from the world's seed.
    let [x, z] = STARTING_NOTE_AT;
    spawn_item(
        &mut commands,
        def("note"),
        write(def("note"), starting_note(world.seed), 0.0, 13).expect("the note def is a note"),
        at + Vec2::new(x, z),
        NOTE_COLOR,
    );
}

const NOTE_COLOR: Rgb = [0.9, 0.88, 0.8];

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
            at + Vec2::new(x, z),
            LOOT_BOX_COLOR,
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
) {
    let table = loot_table();
    for site in new_sites.0.drain(..) {
        let monument = site.rolled.clone();
        for member in &monument.members {
            let at = site.position + Vec2::new(member.offset.x, member.offset.z);
            spawn_structure(&mut commands, &member.structure, at, true);
        }
        if !monument.props.is_empty() {
            let shapes: Vec<(Vec2, Rotation, Collider)> = monument
                .props
                .iter()
                .map(|p| {
                    (
                        Vec2::new(p.position.x, p.position.z),
                        Rotation::IDENTITY,
                        Collider::rectangle(p.size.x, p.size.z),
                    )
                })
                .collect();
            commands.spawn((
                WorldPiece,
                RigidBody::Static,
                Collider::compound(shapes),
                Transform::from_translation(site.position.extend(0.0)),
            ));
            for p in &monument.props {
                let centre = site.position + Vec2::new(p.position.x, p.position.z);
                let at = Transform::from_translation(centre.extend(SOLID_Z));
                // Its picture, or a box in its colour until it has one.
                match &p.picture {
                    Some(name) => commands.spawn((WorldPiece, crate::picture::Picture(name.clone()), at)),
                    None => commands.spawn((WorldPiece, sprite(p.color, Vec2::new(p.size.x, p.size.z)), at)),
                };
            }
        }
        // A box at every loot spot, filled by the spot's danger from the
        // one loot table (modforge's roll, seeded by the site).
        if let Some(box_def) = items.0.def("storage box") {
            for (k, spot) in monument.loot_spots.iter().enumerate() {
                let mut roll = Roll::new(tile_hash(world.seed, cell_of(site.position), k as u64));
                let mut contents = item::Inventory::new(box_def.storage.unwrap_or(12));
                for (name, count) in roll_loot(&table, spot.danger, &mut roll) {
                    if let Some(def) = items.0.def(&name) {
                        let _ = contents.add(item::create(def, count, &[], 0.0, k as u64), def.max_stack);
                    }
                }
                let at = site.position + Vec2::new(spot.position.x, spot.position.z);
                let found = spawn_item(&mut commands, box_def, item::create(box_def, 1, &[], 0.0, k as u64), at, LOOT_BOX_COLOR);
                commands.entity(found).insert((Container(contents), WorldPiece));
            }
        }
        monuments.0.push((site.position, monument));
    }
}

const LOOT_BOX_COLOR: Rgb = [0.5, 0.4, 0.22];
