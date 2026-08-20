//! The structure vocabulary: every building in the world (the
//! starting bunker, faction bunkers, monuments) is a StructureDef,
//! plain data that hand-authoring or generation can produce alike.
//! spawn_structure is the ONE path from a StructureDef to world
//! geometry and colliders (the one-spawn-path rule in
//! docs/authority.md, applied to structures).
//!
//! Coordinates are structure-local: origin at the center of the
//! room floor, y up, north = negative z.

use bevy::prelude::*;

#[derive(Clone, Copy, PartialEq)]
pub enum Side {
    North,
    South,
    East,
    West,
}

/// A gap in one wall. `offset` runs along the wall from its center
/// (+x on north/south walls, +z on east/west walls). With `door`,
/// a hinged door fills the gap; without, it stays open.
#[derive(Clone)]
pub struct Opening {
    pub side: Side,
    pub offset: f32,
    pub width: f32,
    pub door: bool,
}

/// One rectangular room: interior size (x width, y height,
/// z length), wall thickness, and the openings in its walls.
#[derive(Clone)]
pub struct RoomSpec {
    pub interior: Vec3,
    pub wall_thickness: f32,
    pub openings: Vec<Opening>,
}

/// A solid block: furniture, a crate, any obstacle. Collides.
#[derive(Clone)]
pub struct SolidSpec {
    pub center: Vec3,
    pub size: Vec3,
    pub color: Color,
}

#[derive(Clone)]
pub struct LightSpec {
    pub position: Vec3,
    pub color: Color,
    pub intensity: f32,
}

/// A whole building as data.
#[derive(Clone)]
pub struct StructureDef {
    pub name: String,
    pub room: RoomSpec,
    pub furniture: Vec<SolidSpec>,
    pub lights: Vec<LightSpec>,
}

#[derive(Clone, Copy)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn from_center_size(center: Vec3, size: Vec3) -> Self {
        Self {
            min: center - size / 2.0,
            max: center + size / 2.0,
        }
    }
}

/// Walls and furniture the player cannot walk through.
#[derive(Resource, Default)]
pub struct StaticColliders(pub Vec<Aabb>);

pub const DOOR_HEIGHT: f32 = 2.6;
const DOOR_THICKNESS: f32 = 0.1;
pub const DOOR_REACH: f32 = 2.5;
const DOOR_SWING_DEGREES: f32 = 100.0;

/// A hinged door. All positions are world space, fixed at spawn;
/// the collider applies only while closed.
#[derive(Component)]
pub struct Door {
    pub open: bool,
    pub closed_center: Vec3,
    pub hinge: Vec3,
    pub closed_rotation: Quat,
    pub collider: Aabb,
}

impl Door {
    fn transform(&self) -> Transform {
        if !self.open {
            return Transform::from_translation(self.closed_center)
                .with_rotation(self.closed_rotation);
        }
        let swing = Quat::from_rotation_y(DOOR_SWING_DEGREES.to_radians());
        Transform::from_translation(self.hinge + swing * (self.closed_center - self.hinge))
            .with_rotation(swing * self.closed_rotation)
    }
}

/// Wall run axis and outward wall-center position for one side.
fn side_frame(side: Side, interior: Vec3, t: f32) -> (Vec3, Vec3, f32) {
    let (w, l) = (interior.x, interior.z);
    match side {
        Side::North => (Vec3::X, Vec3::new(0.0, 0.0, -(l / 2.0 + t / 2.0)), w + 2.0 * t),
        Side::South => (Vec3::X, Vec3::new(0.0, 0.0, l / 2.0 + t / 2.0), w + 2.0 * t),
        Side::East => (Vec3::Z, Vec3::new(w / 2.0 + t / 2.0, 0.0, 0.0), l + 2.0 * t),
        Side::West => (Vec3::Z, Vec3::new(-(w / 2.0 + t / 2.0), 0.0, 0.0), l + 2.0 * t),
    }
}

/// The one path from a StructureDef to world geometry and
/// colliders, at a world origin (center of the room floor).
pub fn spawn_structure(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    colliders: &mut StaticColliders,
    def: &StructureDef,
    origin: Vec3,
) {
    let concrete = materials.add(Color::srgb(0.45, 0.45, 0.47));
    let floor_mat = materials.add(Color::srgb(0.35, 0.35, 0.36));
    let door_mat = materials.add(Color::srgb(0.25, 0.28, 0.30));

    let room = &def.room;
    let t = room.wall_thickness;
    let height = room.interior.y;

    let solid = |commands: &mut Commands,
                     meshes: &mut Assets<Mesh>,
                     colliders: &mut StaticColliders,
                     center: Vec3,
                     size: Vec3,
                     material: Handle<StandardMaterial>| {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
            MeshMaterial3d(material),
            Transform::from_translation(origin + center),
        ));
        colliders
            .0
            .push(Aabb::from_center_size(origin + center, size));
    };

    for side in [Side::North, Side::South, Side::East, Side::West] {
        let (axis, wall_center, length) = side_frame(side, room.interior, t);
        // Split the wall run into segments around this side's
        // openings, sorted along the axis.
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
            solid(commands, meshes, colliders, center, size, concrete.clone());
        }
    }

    // Floor slab and ceiling: visual only, the y-overlap check in
    // player movement never collides with them.
    let slab = Vec3::new(room.interior.x + 2.0 * t, 0.1, room.interior.z + 2.0 * t);
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(slab.x, slab.y, slab.z))),
        MeshMaterial3d(floor_mat),
        Transform::from_translation(origin + Vec3::Y * 0.05),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(slab.x, 0.2, slab.z))),
        MeshMaterial3d(concrete.clone()),
        Transform::from_translation(origin + Vec3::Y * (height + 0.1)),
    ));

    for piece in &def.furniture {
        let material = materials.add(piece.color);
        solid(commands, meshes, colliders, piece.center, piece.size, material);
    }

    for opening in room.openings.iter().filter(|o| o.door) {
        let (axis, wall_center, _) = side_frame(opening.side, room.interior, t);
        let door_h = DOOR_HEIGHT.min(height);
        let closed_center =
            origin + wall_center + axis * opening.offset + Vec3::Y * (door_h / 2.0);
        let hinge = closed_center - axis * (opening.width / 2.0);
        let closed_rotation = if axis == Vec3::X {
            Quat::IDENTITY
        } else {
            Quat::from_rotation_y(90f32.to_radians())
        };
        let collider_size =
            axis * opening.width + Vec3::Y * door_h + (Vec3::ONE - axis - Vec3::Y) * DOOR_THICKNESS;
        let door = Door {
            open: false,
            closed_center,
            hinge,
            closed_rotation,
            collider: Aabb::from_center_size(closed_center, collider_size),
        };
        let transform = door.transform();
        commands.spawn((
            door,
            Mesh3d(meshes.add(Cuboid::new(opening.width, door_h, DOOR_THICKNESS))),
            MeshMaterial3d(door_mat.clone()),
            transform,
        ));
    }

    for light in &def.lights {
        commands.spawn((
            PointLight {
                intensity: light.intensity,
                color: light.color,
                ..default()
            },
            Transform::from_translation(origin + light.position),
        ));
    }
}

pub const BODY_RADIUS: f32 = 0.35;
pub const BODY_TOP: f32 = 1.8;
pub const STEP_OVER: f32 = 0.2;

pub fn point_hits(p: Vec3, a: &Aabb) -> bool {
    p.x > a.min.x - BODY_RADIUS
        && p.x < a.max.x + BODY_RADIUS
        && p.z > a.min.z - BODY_RADIUS
        && p.z < a.max.z + BODY_RADIUS
        && a.min.y < BODY_TOP
        && a.max.y > STEP_OVER
}

/// True when a walking body at this point overlaps a wall, a piece
/// of furniture, or a closed door.
pub fn blocked<'a>(
    p: Vec3,
    colliders: &StaticColliders,
    doors: impl Iterator<Item = &'a Door>,
) -> bool {
    colliders.0.iter().any(|a| point_hits(p, a))
        || doors.into_iter().any(|d| !d.open && point_hits(p, &d.collider))
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

    fn one_door_room() -> StructureDef {
        StructureDef {
            name: "test room".to_string(),
            room: RoomSpec {
                interior: Vec3::new(6.0, 3.0, 8.0),
                wall_thickness: 0.2,
                openings: vec![Opening {
                    side: Side::North,
                    offset: 0.0,
                    width: 1.2,
                    door: true,
                }],
            },
            furniture: vec![],
            lights: vec![],
        }
    }

    #[test]
    fn a_wall_with_an_opening_splits_into_two_segments() {
        // Pure math check through the public surface: colliders for
        // the north wall must leave a 1.2m gap around x=0.
        let def = one_door_room();
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<StandardMaterial>>();
        app.init_resource::<StaticColliders>();
        let world = app.world_mut();
        let mut state: SystemState<(
            Commands,
            ResMut<Assets<Mesh>>,
            ResMut<Assets<StandardMaterial>>,
            ResMut<StaticColliders>,
        )> = SystemState::new(world);
        {
            let (mut commands, mut meshes, mut materials, mut colliders) = state.get_mut(world);
            spawn_structure(
                &mut commands,
                &mut meshes,
                &mut materials,
                &mut colliders,
                &def,
                Vec3::ZERO,
            );
        }
        state.apply(world);

        let colliders = world.resource::<StaticColliders>();
        let north_z = -(8.0 / 2.0 + 0.1);
        let in_gap = Vec3::new(0.0, 1.0, north_z);
        let in_wall = Vec3::new(2.0, 1.0, north_z);
        assert!(
            !colliders.0.iter().any(|a| point_hits(in_gap, a)),
            "the doorway gap must not collide"
        );
        assert!(
            colliders.0.iter().any(|a| point_hits(in_wall, a)),
            "the wall beside the gap must collide"
        );
    }
}
