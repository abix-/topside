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
use crate::interact::{Container, spawn_item};
use modforge::item::{Inventory, ItemRegistry, create};
use modforge::monument::validate_monument;
pub use modforge::structure::{
    Aabb, CONCRETE_FLOOR, CONCRETE_WALL, LightSpec, MonumentDef, NpcSpot, Opening, Piece,
    PieceKind, Rgb, RoomSpec, Side, SolidSpec, StairSpec, StructureDef, damage, piece_at,
    pieces_of, side_frame, validate,
};

fn cel_from_rgb(c: Rgb) -> CelMaterial {
    CelMaterial::from_srgb(c[0], c[1], c[2])
}

pub const DOOR_HEIGHT: f32 = 2.6;
const DOOR_THICKNESS: f32 = 0.1;
pub const DOOR_REACH: f32 = 2.5;
const DOOR_SWING_DEGREES: f32 = 100.0;

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

/// The structure's pieces (modforge data): the single source of
/// truth for its solid boxes. The collider and the meshes are baked
/// from this and rebaked when it changes.
#[derive(Component)]
pub struct Pieces(pub Vec<Piece>);

/// Marks a root whose pieces changed; `rebake_structures` rebuilds
/// its collider and meshes next fixed step.
#[derive(Component)]
pub struct Rebake;

/// One merged mesh per colour for a whole structure: every wall,
/// slab, step, and furniture piece of that colour drawn in one go.
/// The pieces themselves are not entities; their boxes live in the
/// root's compound collider.
#[derive(Component, Debug, PartialEq)]
pub struct StructureBatch;

/// Turn a root's pieces into its one compound collider and one
/// merged mesh per colour. Batched the way Rust (the game) runs its
/// buildings (Facepunch devblogs 79 and 82: server side collider
/// batching with a lookup back to the piece, mesh batching
/// refreshed when a piece changes). Measured 2026-08-20
/// (performance.md): one collider per piece costs 12 to 15 ms a
/// physics step in the city; the compound 1.6 ms.
fn bake(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<CelMaterial>,
    root: Entity,
    pieces: &[Piece],
) {
    let mut batches: Vec<(Rgb, Mesh)> = Vec::new();
    let mut shapes = Vec::with_capacity(pieces.len());
    for piece in pieces {
        let size = piece.size;
        shapes.push((
            piece.center,
            Quat::IDENTITY,
            Collider::cuboid(size.x, size.y, size.z),
        ));
        let mesh = Mesh::from(Cuboid::new(size.x, size.y, size.z)).translated_by(piece.center);
        match batches.iter_mut().find(|(c, _)| *c == piece.color) {
            Some((_, batch)) => batch
                .merge(&mesh)
                .expect("every piece is the same cuboid mesh layout"),
            None => batches.push((piece.color, mesh)),
        }
    }
    for (color, mesh) in batches {
        commands.spawn((
            StructureBatch,
            ChildOf(root),
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(cel_from_rgb(color))),
            Transform::IDENTITY,
        ));
    }
    if shapes.is_empty() {
        commands.entity(root).remove::<Collider>();
    } else {
        commands.entity(root).insert(Collider::compound(shapes));
    }
}

/// Rebuild every marked structure: drop its batches, bake again
/// from its pieces. Once per change, never per frame.
pub fn rebake_structures(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CelMaterial>>,
    roots: Query<(Entity, &Pieces, &Children), With<Rebake>>,
    batches: Query<(), With<StructureBatch>>,
) {
    for (root, pieces, children) in &roots {
        for child in children.iter() {
            if batches.get(child).is_ok() {
                commands.entity(child).despawn();
            }
        }
        bake(&mut commands, &mut meshes, &mut materials, root, &pieces.0);
        commands.entity(root).remove::<Rebake>();
    }
}

/// A hit on a structure at a world point: find the piece, take
/// `amount` off it, and mark the root for rebake if the piece died.
/// Returns the piece index hit, if any.
pub fn damage_at(
    commands: &mut Commands,
    root: Entity,
    transform: &Transform,
    pieces: &mut Pieces,
    world_point: Vec3,
    amount: f32,
) -> Option<usize> {
    let local = world_point - transform.translation;
    let index = piece_at(&pieces.0, local)?;
    if damage(&mut pieces.0, index, amount) {
        commands.entity(root).insert(Rebake);
    }
    Some(index)
}

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

    // The root owns every piece (despawn takes all) and is the
    // building's one static body.
    let root = commands
        .spawn((
            StructureRoot,
            RigidBody::Static,
            Transform::from_translation(origin),
            Visibility::default(),
        ))
        .id();

    // The solid boxes are modforge data; they bake into one compound
    // collider and one mesh per colour (see `bake`). Doors are the
    // moving parts, spawned from the openings.
    let pieces = pieces_of(def);
    bake(commands, meshes, materials, root, &pieces);
    commands.entity(root).insert(Pieces(pieces));

    let door_mat = materials.add(CelMaterial::from_srgb(0.25, 0.28, 0.30));
    for room in &def.rooms {
        let t = room.wall_thickness;
        let height = room.interior.y;
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

/// The root of one spawned monument; its member structures hang
/// off it, so despawning it takes the whole monument (the storm).
#[derive(Component)]
pub struct MonumentRoot {
    pub name: String,
    /// World position the monument was rolled at.
    pub origin: Vec3,
    /// Where this monument's NPCs spawn, world space, kept until
    /// the one NPC spawn path lands.
    pub npc_spots: Vec<NpcSpot>,
}

/// A gate waiting for its door: placed at the gate position when the
/// monument spawns; `lock_gates` finds the door that was spawned
/// there and locks it.
#[derive(Component)]
pub struct PendingGate {
    pub level: u32,
}

/// A door that stays shut until `level` of progression, which
/// nothing grants yet (Rust's card rooms).
#[derive(Component, Debug, PartialEq)]
pub struct Locked(pub u32);

/// The one path from a MonumentDef to the world: every member goes
/// through `spawn_structure` under one root, every loot spot becomes
/// a storage box through `spawn_item`, every gate marks its door.
/// Loot is a fixed list scaled by the spot's danger until loot
/// tables land.
pub fn spawn_monument(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<CelMaterial>,
    items: &ItemRegistry,
    def: &MonumentDef,
    origin: Vec3,
    now: f32,
) -> Result<Entity, String> {
    validate_monument(def)?;
    // The root owns the members (despawn takes all) but sits at the
    // world origin: every member, door, and box keeps world-space
    // positions, as the structure contract requires.
    let root = commands
        .spawn((
            MonumentRoot {
                name: def.name.clone(),
                origin,
                npc_spots: def
                    .npc_spots
                    .iter()
                    .map(|s| NpcSpot {
                        position: origin + s.position,
                        danger: s.danger,
                    })
                    .collect(),
            },
            Transform::default(),
            Visibility::default(),
        ))
        .id();
    for member in &def.members {
        let structure = spawn_structure(
            commands,
            meshes,
            materials,
            &member.structure,
            origin + member.offset,
        )?;
        commands.entity(structure).insert(ChildOf(root));
    }
    let box_def = items
        .def("storage box")
        .ok_or("the storage box item is not registered")?;
    for (i, spot) in def.loot_spots.iter().enumerate() {
        let mut loot = Inventory::new(box_def.storage.unwrap_or(0));
        let salt = i as u64 + 100;
        for (name, count) in [
            ("scrap", 4 + spot.danger * 2),
            ("cloth", 2 + spot.danger),
            ("canned food", 1 + spot.danger / 2),
        ] {
            let item = items.def(name).ok_or(format!("item '{name}' is not registered"))?;
            loot.add(create(item, count, &[], now, salt), item.max_stack);
        }
        if spot.danger >= 3 {
            let pipe = items.def("pipe").ok_or("item 'pipe' is not registered")?;
            loot.add(create(pipe, 1, &[], now, salt + 1), pipe.max_stack);
        }
        let entity = spawn_item(
            commands,
            meshes,
            materials,
            box_def,
            create(box_def, 1, &[], now, salt + 2),
            origin + spot.position,
            [0.55, 0.45, 0.20],
        );
        commands.entity(entity).insert(Container(loot));
    }
    for gate in &def.gates {
        commands.spawn((
            PendingGate { level: gate.level },
            Transform::from_translation(origin + gate.position),
        ));
    }
    Ok(root)
}

/// Lock the door nearest each pending gate. Doors are spawned by
/// commands, so this runs a tick after `spawn_monument`.
pub fn lock_gates(
    mut commands: Commands,
    gates: Query<(Entity, &PendingGate, &Transform)>,
    doors: Query<(Entity, &Door), Without<Locked>>,
) {
    const REACH: f32 = 1.5;
    for (gate, pending, transform) in &gates {
        let nearest = doors
            .iter()
            .map(|(e, d)| (e, d.closed_center.distance(transform.translation)))
            .filter(|(_, dist)| *dist < REACH)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((door, _)) = nearest {
            commands.entity(door).insert(Locked(pending.level));
            commands.entity(gate).despawn();
        }
    }
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

