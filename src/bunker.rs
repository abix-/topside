//! The opening bunker (design.md "The bunker"): a concrete room
//! around the spawn point holding a bed, a crafting bench, and a
//! crate with three days of food. Seamless: the bunker is geometry
//! in the world scene and the door is a door, not a level
//! transition.

use bevy::prelude::*;

/// One game day in real seconds (Valheim-scale pacing; tune later).
pub const GAME_DAY_SECS: f32 = 1200.0;

/// The first clock: three days of food forcing the player topside.
#[derive(Resource)]
pub struct FoodStore {
    pub days: f32,
}

impl Default for FoodStore {
    fn default() -> Self {
        Self { days: 3.0 }
    }
}

pub fn food_tick_system(time: Res<Time>, mut food: ResMut<FoodStore>) {
    food.days = (food.days - time.delta_secs() / GAME_DAY_SECS).max(0.0);
}

#[derive(Clone, Copy)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    fn from_center_size(center: Vec3, size: Vec3) -> Self {
        Self {
            min: center - size / 2.0,
            max: center + size / 2.0,
        }
    }
}

/// Walls and furniture the player cannot walk through.
#[derive(Resource, Default)]
pub struct StaticColliders(pub Vec<Aabb>);

#[derive(Resource)]
pub struct DoorState {
    pub open: bool,
    pub collider: Aabb,
}

#[derive(Component)]
pub struct Door;

const DOOR_SIZE: Vec3 = Vec3::new(1.2, 2.6, 0.1);
const DOOR_CLOSED_CENTER: Vec3 = Vec3::new(0.0, 1.3, -4.1);
pub const DOOR_REACH: f32 = 2.5;

pub fn near_door(p: Vec3) -> bool {
    p.distance(DOOR_CLOSED_CENTER) < DOOR_REACH
}

/// Closed: flush in the wall gap. Open: swung 100 degrees around
/// the hinge on its left edge.
fn door_transform(open: bool) -> Transform {
    if !open {
        return Transform::from_translation(DOOR_CLOSED_CENTER);
    }
    let hinge = DOOR_CLOSED_CENTER - Vec3::new(DOOR_SIZE.x / 2.0, 0.0, 0.0);
    let swing = Quat::from_rotation_y(100f32.to_radians());
    Transform::from_translation(hinge + swing * (DOOR_CLOSED_CENTER - hinge))
        .with_rotation(swing)
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut colliders: ResMut<StaticColliders>,
) {
    let concrete = materials.add(Color::srgb(0.45, 0.45, 0.47));
    let solid = |center: Vec3,
                     size: Vec3,
                     material: Handle<StandardMaterial>,
                     meshes: &mut Assets<Mesh>,
                     commands: &mut Commands,
                     colliders: &mut StaticColliders| {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
            MeshMaterial3d(material),
            Transform::from_translation(center),
        ));
        colliders.0.push(Aabb::from_center_size(center, size));
    };

    // Room: interior 6m wide (x) by 8m long (z), 3m high. The door
    // gap is in the north wall (negative z), facing topside.
    let walls = [
        (Vec3::new(-3.1, 1.5, 0.0), Vec3::new(0.2, 3.0, 8.4)),
        (Vec3::new(3.1, 1.5, 0.0), Vec3::new(0.2, 3.0, 8.4)),
        (Vec3::new(0.0, 1.5, 4.1), Vec3::new(6.4, 3.0, 0.2)),
        (Vec3::new(-1.9, 1.5, -4.1), Vec3::new(2.6, 3.0, 0.2)),
        (Vec3::new(1.9, 1.5, -4.1), Vec3::new(2.6, 3.0, 0.2)),
    ];
    for (center, size) in walls {
        solid(center, size, concrete.clone(), &mut meshes, &mut commands, &mut colliders);
    }

    // Floor slab and ceiling: visual only, the y-overlap check in
    // player movement never collides with them.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(6.4, 0.1, 8.4))),
        MeshMaterial3d(materials.add(Color::srgb(0.35, 0.35, 0.36))),
        Transform::from_xyz(0.0, 0.05, 0.0),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(6.4, 0.2, 8.4))),
        MeshMaterial3d(concrete.clone()),
        Transform::from_xyz(0.0, 3.1, 0.0),
    ));

    // What the bunker starts with: a bed, a crafting bench, a food
    // crate (design.md: exactly three things).
    let bed = materials.add(Color::srgb(0.30, 0.35, 0.55));
    let bench = materials.add(Color::srgb(0.45, 0.32, 0.18));
    let crate_mat = materials.add(Color::srgb(0.55, 0.45, 0.20));
    solid(Vec3::new(-2.4, 0.25, 3.0), Vec3::new(1.0, 0.5, 2.0), bed, &mut meshes, &mut commands, &mut colliders);
    solid(Vec3::new(2.2, 0.45, 3.5), Vec3::new(1.6, 0.9, 0.7), bench, &mut meshes, &mut commands, &mut colliders);
    solid(Vec3::new(2.5, 0.35, -3.3), Vec3::new(0.7, 0.7, 0.7), crate_mat, &mut meshes, &mut commands, &mut colliders);

    // The door. Its collider lives in DoorState, checked only while
    // closed.
    commands.spawn((
        Door,
        Mesh3d(meshes.add(Cuboid::new(DOOR_SIZE.x, DOOR_SIZE.y, DOOR_SIZE.z))),
        MeshMaterial3d(materials.add(Color::srgb(0.25, 0.28, 0.30))),
        door_transform(false),
    ));
    commands.insert_resource(DoorState {
        open: false,
        collider: Aabb::from_center_size(DOOR_CLOSED_CENTER, DOOR_SIZE),
    });

    commands.spawn((
        PointLight {
            intensity: 400_000.0,
            color: Color::srgb(1.0, 0.9, 0.7),
            ..default()
        },
        Transform::from_xyz(0.0, 2.7, 0.0),
    ));
}

pub fn door_interact(
    keys: Res<ButtonInput<KeyCode>>,
    door_state: Option<ResMut<DoorState>>,
    mut doors: Query<&mut Transform, With<Door>>,
    players: Query<&Transform, (With<crate::player::Player>, Without<Door>)>,
) {
    let Some(mut door_state) = door_state else {
        return;
    };
    if !keys.just_pressed(KeyCode::KeyE) {
        return;
    }
    let Ok(player) = players.single() else {
        return;
    };
    if !near_door(player.translation) {
        return;
    }
    door_state.open = !door_state.open;
    for mut t in &mut doors {
        *t = door_transform(door_state.open);
    }
}
