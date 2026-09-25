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
use modforge::structure::{PartKind, Rgb, StructureDef, doors_of, floor_plan, parts_of};

use crate::content::starting_bunker;
use crate::interact::{Door, Interactable};
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

/// The starting bunker at the world's bunker point.
pub fn spawn_bunker(mut commands: Commands, world: Res<WorldRes>) {
    spawn_structure(&mut commands, &starting_bunker(), world.0.def.bunker);
}
