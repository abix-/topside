//! Topside's binder for the modforge structure vocabulary: the
//! defs and relation rules live in modforge::structure (buildings
//! as engine-agnostic data); this module owns making them real in
//! THIS world: spawn_structure (the one path to Bevy geometry,
//! colliders, and walkable surfaces), doors, and the walking
//! collision the player moves against.
//!
//! Bevy 0.19 and modforge share the same glam, so Vec3 here and
//! Vec3 in the defs are one type.

use avian3d::prelude::*;
use bevy::prelude::*;
use crate::cel::CelMaterial;
pub use modforge::structure::{
    Aabb, LightSpec, Opening, Rgb, RoomSpec, Side, SolidSpec, StairSpec, StructureDef, side_frame,
    validate,
};

fn cel_from_rgb(c: Rgb) -> CelMaterial {
    CelMaterial::from_srgb(c[0], c[1], c[2])
}

pub const DOOR_HEIGHT: f32 = 2.6;
const DOOR_THICKNESS: f32 = 0.1;
pub const DOOR_REACH: f32 = 2.5;
const DOOR_SWING_DEGREES: f32 = 100.0;
const SLAB: f32 = 0.1;
const STEP_RISE_MAX: f32 = 0.31;
const STEP_DEPTH: f32 = 0.3;

pub const BODY_RADIUS: f32 = 0.35;
pub const BODY_TOP: f32 = 1.8;
/// Height a walking body steps over / climbs per move.
pub const CLIMB: f32 = 0.35;

/// The walking body's collision shape: a capsule BODY_TOP tall,
/// BODY_RADIUS wide, centred at feet + BODY_TOP / 2.
pub fn body_shape() -> Collider {
    Collider::capsule(BODY_RADIUS, BODY_TOP - 2.0 * BODY_RADIUS)
}

/// The static body every piece of one spawned structure hangs
/// off. Despawning it takes the geometry, doors, and colliders
/// with it.
#[derive(Component, Debug, PartialEq)]
pub struct StructureRoot;

/// A wall, slab, step, or furniture piece: visible geometry and
/// its collider on one child of the structure root.
#[derive(Component, Debug, PartialEq)]
pub struct StructureSolid;

/// A hinged door. All positions are world space, fixed at spawn;
/// the collider applies only while closed. `origin` is the
/// structure root's position, so the door's local transform is
/// its world pose minus the origin.
#[derive(Component)]
pub struct Door {
    pub open: bool,
    pub origin: Vec3,
    pub closed_center: Vec3,
    pub hinge: Vec3,
    pub closed_rotation: Quat,
    pub collider: Aabb,
}

impl Door {
    /// Local transform relative to the structure root.
    pub fn transform(&self) -> Transform {
        if !self.open {
            return Transform::from_translation(self.closed_center - self.origin)
                .with_rotation(self.closed_rotation);
        }
        let swing = Quat::from_rotation_y(DOOR_SWING_DEGREES.to_radians());
        Transform::from_translation(
            self.hinge + swing * (self.closed_center - self.hinge) - self.origin,
        )
        .with_rotation(swing * self.closed_rotation)
    }
}

/// The one path from a StructureDef to world geometry and colliders
/// at a world origin. Returns the structure root.
pub fn spawn_structure(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<CelMaterial>,
    def: &StructureDef,
    origin: Vec3,
) -> Result<Entity, String> {
    validate(def)?;

    let root = commands
        .spawn((
            StructureRoot,
            RigidBody::Static,
            Transform::from_translation(origin),
            Visibility::default(),
        ))
        .id();

    let concrete = materials.add(CelMaterial::from_srgb(0.45, 0.45, 0.47));
    let floor_mat = materials.add(CelMaterial::from_srgb(0.35, 0.35, 0.36));
    let door_mat = materials.add(CelMaterial::from_srgb(0.25, 0.28, 0.30));
    let stair_mat = materials.add(CelMaterial::from_srgb(0.40, 0.38, 0.36));

    // One child of the root per solid piece: mesh and collider
    // together, positioned relative to the root. `center` is
    // structure-local.
    let solid = |commands: &mut Commands,
                 meshes: &mut Assets<Mesh>,
                 center: Vec3,
                 size: Vec3,
                 material: Handle<CelMaterial>| {
        commands.spawn((
            StructureSolid,
            ChildOf(root),
            Collider::cuboid(size.x, size.y, size.z),
            Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
            MeshMaterial3d(material),
            Transform::from_translation(center),
        ));
    };

    for room in &def.rooms {
        let t = room.wall_thickness;
        let height = room.interior.y;

        for side in [Side::North, Side::South, Side::East, Side::West] {
            let (axis, frame, length) = side_frame(side, room.interior, t);
            let wall_center = room.origin + frame;
            let mut cuts: Vec<(f32, f32)> = room
                .openings
                .iter()
                .filter(|o| o.side == side)
                .map(|o| (o.offset - o.width / 2.0, o.offset + o.width / 2.0))
                .collect();
            cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut cursor = -length / 2.0;
            let mut segments = Vec::new();
            for (start, end) in cuts {
                if start > cursor {
                    segments.push((cursor, start));
                }
                cursor = cursor.max(end);
            }
            if cursor < length / 2.0 {
                segments.push((cursor, length / 2.0));
            }
            for (start, end) in segments {
                let mid = (start + end) / 2.0;
                let run = end - start;
                let center = wall_center + axis * mid + Vec3::Y * (height / 2.0);
                let size = axis * run + Vec3::Y * height + (Vec3::ONE - axis - Vec3::Y) * t;
                solid(commands, meshes, center, size, concrete.clone());
            }
            // Panels below raised sills.
            for opening in room
                .openings
                .iter()
                .filter(|o| o.side == side && o.sill > 0.0)
            {
                let center =
                    wall_center + axis * opening.offset + Vec3::Y * (opening.sill / 2.0);
                let size = axis * opening.width
                    + Vec3::Y * opening.sill
                    + (Vec3::ONE - axis - Vec3::Y) * t;
                solid(commands, meshes, center, size, concrete.clone());
            }
        }

        let slab_xz = Vec3::new(room.interior.x + 2.0 * t, 0.0, room.interior.z + 2.0 * t);
        if room.floor {
            let center = room.origin + Vec3::Y * (SLAB / 2.0);
            let size = slab_xz + Vec3::Y * SLAB;
            solid(commands, meshes, center, size, floor_mat.clone());
        }
        if room.ceiling {
            let center = room.origin + Vec3::Y * (height + SLAB);
            let size = slab_xz + Vec3::Y * 0.2;
            solid(commands, meshes, center, size, concrete.clone());
        }

        for opening in room.openings.iter().filter(|o| o.door) {
            let (axis, frame, _) = side_frame(opening.side, room.interior, t);
            let door_h = DOOR_HEIGHT.min(height - opening.sill);
            let closed_center = origin
                + room.origin
                + frame
                + axis * opening.offset
                + Vec3::Y * (opening.sill + door_h / 2.0);
            let hinge = closed_center - axis * (opening.width / 2.0);
            let closed_rotation = if axis == Vec3::X {
                Quat::IDENTITY
            } else {
                Quat::from_rotation_y(90f32.to_radians())
            };
            let collider_size = axis * opening.width
                + Vec3::Y * door_h
                + (Vec3::ONE - axis - Vec3::Y) * DOOR_THICKNESS;
            let door = Door {
                open: false,
                origin,
                closed_center,
                hinge,
                closed_rotation,
                collider: Aabb::from_center_size(closed_center, collider_size),
            };
            let transform = door.transform();
            commands.spawn((
                door,
                ChildOf(root),
                RigidBody::Kinematic,
                Collider::cuboid(opening.width, door_h, DOOR_THICKNESS),
                Mesh3d(meshes.add(Cuboid::new(opening.width, door_h, DOOR_THICKNESS))),
                MeshMaterial3d(door_mat.clone()),
                transform,
            ));
        }
    }

    for stair in &def.stairs {
        let dir = stair.side.outward();
        let across = Vec3::ONE - dir.abs() - Vec3::Y;
        let steps = (stair.rise / STEP_RISE_MAX).ceil().max(1.0) as usize;
        let step_rise = stair.rise / steps as f32;
        for i in 0..steps {
            let height = step_rise * (i + 1) as f32;
            let center = stair.base
                + dir * (STEP_DEPTH * (i as f32 + 0.5))
                + Vec3::Y * (height / 2.0);
            let size = dir.abs() * STEP_DEPTH + across * stair.width + Vec3::Y * height;
            solid(commands, meshes, center, size, stair_mat.clone());
        }
        if stair.landing > 0.0 {
            let center = stair.base
                + dir * (STEP_DEPTH * steps as f32 + stair.landing / 2.0)
                + Vec3::Y * (stair.rise - SLAB / 2.0);
            let size = dir.abs() * stair.landing + across * stair.width + Vec3::Y * SLAB;
            solid(commands, meshes, center, size, stair_mat.clone());
        }
    }

    for piece in &def.furniture {
        let material = materials.add(cel_from_rgb(piece.color));
        solid(commands, meshes, piece.center, piece.size, material);
    }

    for light in &def.lights {
        commands.spawn((
            ChildOf(root),
            PointLight {
                intensity: light.intensity,
                color: Color::srgb(light.color[0], light.color[1], light.color[2]),
                ..default()
            },
            Transform::from_translation(light.position),
        ));
    }

    Ok(root)
}

pub fn door_interact(
    keys: Res<ButtonInput<KeyCode>>,
    mut doors: Query<(&mut Door, &mut Transform)>,
    players: Query<&Transform, (With<crate::player::Player>, Without<Door>)>,
) {
    if !keys.just_pressed(KeyCode::KeyE) {
        return;
    }
    let Ok(player) = players.single() else {
        return;
    };
    for (mut door, mut transform) in &mut doors {
        if player.translation.distance(door.closed_center) < DOOR_REACH {
            door.open = !door.open;
            *transform = door.transform();
            return;
        }
    }
}

