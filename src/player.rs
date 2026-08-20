//! First-person camera on a flat ground plane. Skeleton controls:
//! click grabs the cursor, mouse looks, WASD walks, Escape releases.

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

#[derive(Component)]
pub struct Player {
    pub yaw: f32,
    pub pitch: f32,
}

const EYE_HEIGHT: f32 = 1.7;
const WALK_SPEED: f32 = 5.0;
const MOUSE_SENS: f32 = 0.002;
const PITCH_LIMIT: f32 = 1.54;

pub fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(200.0, 200.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.35, 0.40, 0.30))),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 10_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, 0.4, 0.0)),
    ));
    // Spawn inside the bunker, facing the door (negative z).
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, EYE_HEIGHT, 2.0),
        Player {
            yaw: 0.0,
            pitch: 0.0,
        },
    ));
}

pub fn cursor_grab(
    mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
) {
    let Ok(mut cursor) = cursors.single_mut() else {
        return;
    };
    if buttons.just_pressed(MouseButton::Left) {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
    if keys.just_pressed(KeyCode::Escape) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}

pub fn mouse_look(
    mut motions: MessageReader<MouseMotion>,
    cursors: Query<&CursorOptions, With<PrimaryWindow>>,
    mut players: Query<(&mut Transform, &mut Player)>,
) {
    let Ok(cursor) = cursors.single() else {
        return;
    };
    if cursor.grab_mode == CursorGrabMode::None {
        motions.clear();
        return;
    }
    let mut delta = Vec2::ZERO;
    for m in motions.read() {
        delta += m.delta;
    }
    if delta == Vec2::ZERO {
        return;
    }
    for (mut transform, mut player) in &mut players {
        player.yaw -= delta.x * MOUSE_SENS;
        player.pitch = (player.pitch - delta.y * MOUSE_SENS).clamp(-PITCH_LIMIT, PITCH_LIMIT);
        transform.rotation = Quat::from_euler(EulerRot::YXZ, player.yaw, player.pitch, 0.0);
    }
}

pub fn movement(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    colliders: Res<crate::structure::StaticColliders>,
    walkables: Res<crate::structure::Walkables>,
    doors: Query<&crate::structure::Door>,
    mut players: Query<(&mut Transform, &Player), Without<crate::structure::Door>>,
) {
    let mut wish = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        wish.z -= 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        wish.z += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        wish.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        wish.x += 1.0;
    }
    if wish == Vec3::ZERO {
        return;
    }
    for (mut transform, player) in &mut players {
        let dir = (Quat::from_rotation_y(player.yaw) * wish).normalize();
        let step = dir * WALK_SPEED * time.delta_secs();
        let feet = transform.translation.y - EYE_HEIGHT;
        // Per-axis resolution: a blocked axis stops, the other
        // slides. Enough for axis-aligned walls.
        let mut pos = transform.translation;
        pos.x += step.x;
        if crate::structure::blocked(pos, feet, &colliders, doors.iter()) {
            pos.x = transform.translation.x;
        }
        pos.z += step.z;
        if crate::structure::blocked(pos, feet, &colliders, doors.iter()) {
            pos.z = transform.translation.z;
        }
        // Feet follow the highest reachable surface: floor slabs,
        // steps, landings, or the bare ground at 0.
        let support = walkables.support(pos.x, pos.z, feet + crate::structure::CLIMB);
        transform.translation = Vec3::new(pos.x, support + EYE_HEIGHT, pos.z);
    }
}
