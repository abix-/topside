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

/// Walls and furniture the player cannot walk through.
#[derive(Resource, Default)]
pub struct StaticColliders(pub Vec<Aabb>);

/// Surfaces feet can stand on (floor slabs, steps, landings). The
/// bare ground is implicit at y 0.
#[derive(Resource, Default)]
pub struct Walkables(pub Vec<Aabb>);

impl Walkables {
    /// Height of the highest surface under the body at (x, z) whose
    /// top is reachable (at or below `max_feet`). 0 = the ground.
    /// Surfaces count within most of the body radius, matching the
    /// collision footprint: without this, a step flush against a
    /// wall can never be stood on, because the wall's body radius
    /// keeps the center off it.
    pub fn support(&self, x: f32, z: f32, max_feet: f32) -> f32 {
        const R: f32 = 0.3;
        self.0
            .iter()
            .filter(|a| {
                x > a.min.x - R
                    && x < a.max.x + R
                    && z > a.min.z - R
                    && z < a.max.z + R
                    && a.max.y <= max_feet
            })
            .map(|a| a.max.y)
            .fold(0.0, f32::max)
    }
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

/// The one path from a StructureDef to world geometry, colliders,
/// and walkables, at a world origin.
pub fn spawn_structure(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<CelMaterial>,
    colliders: &mut StaticColliders,
    walkables: &mut Walkables,
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
    // Movement compatibility output, kept until the player moves
    // onto the Avian world (todo priority 2).
    let blocking = |colliders: &mut StaticColliders, center: Vec3, size: Vec3| {
        colliders
            .0
            .push(Aabb::from_center_size(origin + center, size));
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
                blocking(colliders, center, size);
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
                blocking(colliders, center, size);
            }
        }

        let slab_xz = Vec3::new(room.interior.x + 2.0 * t, 0.0, room.interior.z + 2.0 * t);
        if room.floor {
            let center = room.origin + Vec3::Y * (SLAB / 2.0);
            let size = slab_xz + Vec3::Y * SLAB;
            solid(commands, meshes, center, size, floor_mat.clone());
            walkables
                .0
                .push(Aabb::from_center_size(origin + center, size));
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
            walkables
                .0
                .push(Aabb::from_center_size(origin + center, size));
        }
        if stair.landing > 0.0 {
            let center = stair.base
                + dir * (STEP_DEPTH * steps as f32 + stair.landing / 2.0)
                + Vec3::Y * (stair.rise - SLAB / 2.0);
            let size = dir.abs() * stair.landing + across * stair.width + Vec3::Y * SLAB;
            solid(commands, meshes, center, size, stair_mat.clone());
            walkables
                .0
                .push(Aabb::from_center_size(origin + center, size));
        }
    }

    for piece in &def.furniture {
        let material = materials.add(cel_from_rgb(piece.color));
        solid(commands, meshes, piece.center, piece.size, material);
        blocking(colliders, piece.center, piece.size);
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

/// True when a walking body with feet at `feet` height overlaps a
/// wall, furniture, or a closed door at point p. Anything at or
/// below CLIMB above the feet is steppable, not blocking.
pub fn blocked<'a>(
    p: Vec3,
    feet: f32,
    colliders: &StaticColliders,
    doors: impl Iterator<Item = &'a Door>,
) -> bool {
    let test = |a: &Aabb| {
        p.x > a.min.x - BODY_RADIUS
            && p.x < a.max.x + BODY_RADIUS
            && p.z > a.min.z - BODY_RADIUS
            && p.z < a.max.z + BODY_RADIUS
            && a.min.y < feet + BODY_TOP
            && a.max.y > feet + CLIMB
    };
    colliders.0.iter().any(test) || doors.into_iter().any(|d| !d.open && test(&d.collider))
}

pub fn point_hits(p: Vec3, a: &Aabb) -> bool {
    p.x > a.min.x - BODY_RADIUS
        && p.x < a.max.x + BODY_RADIUS
        && p.z > a.min.z - BODY_RADIUS
        && p.z < a.max.z + BODY_RADIUS
        && a.min.y < BODY_TOP
        && a.max.y > CLIMB
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::SystemState;

    type SpawnStructureParams<'w, 's> = (
        Commands<'w, 's>,
        ResMut<'w, Assets<Mesh>>,
        ResMut<'w, Assets<CelMaterial>>,
        ResMut<'w, StaticColliders>,
        ResMut<'w, Walkables>,
    );

    fn spawn_into_world(def: &StructureDef) -> (StaticColliders, Walkables, Result<Entity, String>) {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<CelMaterial>>();
        app.init_resource::<StaticColliders>();
        app.init_resource::<Walkables>();
        let world = app.world_mut();
        let mut state: SystemState<SpawnStructureParams<'_, '_>> = SystemState::new(world);
        let result;
        {
            let (mut commands, mut meshes, mut materials, mut colliders, mut walkables) =
                state
                    .get_mut(world)
                    .expect("structure spawn system state is valid");
            result = spawn_structure(
                &mut commands,
                &mut meshes,
                &mut materials,
                &mut colliders,
                &mut walkables,
                def,
                Vec3::ZERO,
            );
        }
        state.apply(world);
        (
            std::mem::take(&mut *app.world_mut().resource_mut::<StaticColliders>()),
            std::mem::take(&mut *app.world_mut().resource_mut::<Walkables>()),
            result,
        )
    }

    #[test]
    fn a_wall_with_an_opening_splits_into_two_segments() {
        let def = StructureDef {
            name: "test room".to_string(),
            rooms: vec![RoomSpec {
                origin: Vec3::ZERO,
                interior: Vec3::new(6.0, 3.0, 8.0),
                wall_thickness: 0.2,
                openings: vec![Opening {
                    side: Side::North,
                    offset: 0.0,
                    width: 1.2,
                    sill: 0.0,
                    door: true,
                }],
                floor: true,
                ceiling: true,
            }],
            stairs: vec![],
            furniture: vec![],
            lights: vec![],
        };
        let (colliders, _, result) = spawn_into_world(&def);
        result.unwrap();
        let north_z = -(8.0 / 2.0 + 0.1);
        assert!(
            !colliders
                .0
                .iter()
                .any(|a| point_hits(Vec3::new(0.0, 1.0, north_z), a)),
            "the doorway gap must not collide"
        );
        assert!(
            colliders
                .0
                .iter()
                .any(|a| point_hits(Vec3::new(2.0, 1.0, north_z), a)),
            "the wall beside the gap must collide"
        );
    }

    #[test]
    fn stairs_are_climbable_step_by_step() {
        let def = StructureDef {
            name: "stairs".to_string(),
            rooms: vec![],
            stairs: vec![StairSpec {
                base: Vec3::ZERO,
                side: Side::North,
                width: 2.0,
                rise: 3.1,
                landing: 0.5,
            }],
            furniture: vec![],
            lights: vec![],
        };
        let (_, walkables, result) = spawn_into_world(&def);
        result.unwrap();
        let mut feet = 0.0_f32;
        // Walk north up the run and onto the landing (10 steps of
        // 0.3 plus the 0.5 landing); every support gain must be
        // within CLIMB. One stride further would be walking off the
        // far edge, which is a fall, not a stairs defect.
        let mut z = -0.15_f32;
        for _ in 0..12 {
            let s = walkables.support(0.0, z, feet + CLIMB);
            assert!(
                s >= feet && s - feet <= CLIMB,
                "unclimbable jump at z {z}: {feet} -> {s}"
            );
            feet = s;
            z -= STEP_DEPTH;
        }
        assert!(
            (feet - 3.1).abs() < 0.01,
            "the top of the stairs must reach the full rise, got {feet}"
        );
    }
}
