//! Buildings seen from above, one floor at a time (design.md "What 2D
//! changes"). modforge turns a `StructureDef` into parts
//! (`parts_of`), the parts into the plan of one floor (`floor_plan`),
//! and places its doors (`doors_of`); topside only draws the plan and
//! gives its solid parts one static collider per building (the 3D
//! game's rule, one collider per building). Structure-local x and z
//! are the screen's x and y, the same as the world's.

use avian2d::prelude::*;
use bevy::prelude::*;
use modforge::hud::InteractKind;
use modforge::structure::{
    MonumentDef, PartKind, Rgb, StructureDef, doors_of, floor_plan, parts_of,
};

use modforge::item::{create, write};

use crate::content::{
    ItemRegistryRes, MonumentRegistryRes, STARTING_ITEMS, STARTING_NOTE_AT, starting_bunker, starting_note,
};
use crate::interact::{Door, Interactable, spawn_item};
use crate::terrain::WorldRes;

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

/// Spawn one building's ground floor at `at`: every part of its plan
/// drawn, its solid parts as one compound collider on a static root,
/// and a closed door in every doorway. Returns the root.
pub fn spawn_structure(commands: &mut Commands, def: &StructureDef, at: Vec2) -> Entity {
    let plan = floor_plan(&parts_of(def), 0.0);
    let shapes: Vec<(Vec2, Rotation, Collider)> = plan
        .iter()
        .filter(|p| p.solid)
        .map(|p| {
            let size = p.max - p.min;
            ((p.min + p.max) / 2.0, Rotation::IDENTITY, Collider::rectangle(size.x, size.y))
        })
        .collect();
    let mut root = commands.spawn((StructureRoot, RigidBody::Static, Transform::from_translation(at.extend(0.0))));
    if !shapes.is_empty() {
        root.insert(Collider::compound(shapes));
    }
    let root = root.id();
    for p in &plan {
        let z = if p.kind == PartKind::Floor { FLOOR_Z } else { SOLID_Z };
        commands.spawn((
            sprite(p.color, p.max - p.min),
            Transform::from_translation((at + (p.min + p.max) / 2.0).extend(z)),
        ));
    }
    for door in doors_of(def) {
        let size = Vec2::new(door.size.x, door.size.z);
        let centre = at + Vec2::new(door.center.x, door.center.z);
        // The hinge is at one end of the door, along the wall.
        let along = if size.x >= size.y { Vec2::X * size.x } else { Vec2::Y * size.y };
        commands.spawn((
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
    }
    root
}

/// The starting bunker at the world's bunker point, with its starting
/// items and the note on the floor.
pub fn spawn_bunker(mut commands: Commands, world: Res<WorldRes>, items: Res<ItemRegistryRes>) {
    let at = world.0.def.bunker;
    spawn_structure(&mut commands, &starting_bunker(), at);
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
    // The note and the world both come from the world's seed.
    let [x, z] = STARTING_NOTE_AT;
    spawn_item(
        &mut commands,
        def("note"),
        write(def("note"), starting_note(world.0.seed), 0.0, 13).expect("the note def is a note"),
        at + Vec2::new(x, z),
        NOTE_COLOR,
    );
}

const NOTE_COLOR: Rgb = [0.9, 0.88, 0.8];

/// Every site's monument as modforge rolled it, and where it stands:
/// rolled once, read by whatever needs its NPC spots or loot spots.
#[derive(Resource, Default)]
pub struct MonumentsRes(pub Vec<(Vec2, MonumentDef)>);

/// Roll the monument at every site (from the world's `site_seed`) and
/// spawn it: each member building's ground floor through
/// `spawn_structure`, and its props as one static collider.
pub fn spawn_monuments(
    mut commands: Commands,
    world: Res<WorldRes>,
    registries: Res<MonumentRegistryRes>,
) {
    let world = &world.0;
    let mut rolled = Vec::with_capacity(world.sites.len());
    for (i, site) in world.sites.iter().enumerate() {
        let monument = registries
            .monuments
            .roll(&site.monument, &registries.buildings, world.site_seed(i))
            .expect("every site's monument type is registered");
        for member in &monument.members {
            let at = site.position + Vec2::new(member.offset.x, member.offset.z);
            spawn_structure(&mut commands, &member.structure, at);
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
                RigidBody::Static,
                Collider::compound(shapes),
                Transform::from_translation(site.position.extend(0.0)),
            ));
            for p in &monument.props {
                let centre = site.position + Vec2::new(p.position.x, p.position.z);
                commands.spawn((
                    sprite(p.color, Vec2::new(p.size.x, p.size.z)),
                    Transform::from_translation(centre.extend(SOLID_Z)),
                ));
            }
        }
        rolled.push((site.position, monument));
    }
    commands.insert_resource(MonumentsRes(rolled));
}
