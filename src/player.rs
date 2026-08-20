//! First-person camera on a flat ground plane. Skeleton controls:
//! click grabs the cursor, mouse looks, WASD walks, Escape releases.

use crate::cel::CelMaterial;
use crate::structure::{BODY_TOP, CLIMB, body_shape};
use avian3d::prelude::*;
use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use modforge::item::Inventory;

#[derive(Component)]
#[require(PlayerInventory)]
pub struct Player {
    pub yaw: f32,
    pub pitch: f32,
}

/// The player's one inventory, owned by the player entity. Gameplay
/// writes it here; the HUD reads it to paint and never copies it.
#[derive(Component)]
pub struct PlayerInventory(pub Inventory);

pub const INVENTORY_SLOTS: usize = 20;

impl Default for PlayerInventory {
    fn default() -> Self {
        Self(Inventory::new(INVENTORY_SLOTS))
    }
}

#[derive(Resource, Default)]
pub struct MovementInput(pub Vec2);

const EYE_HEIGHT: f32 = 1.7;
const WALK_SPEED: f32 = 5.0;
const MOUSE_SENS: f32 = 0.002;
const PITCH_LIMIT: f32 = 1.54;

pub fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CelMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(200.0, 200.0))),
        MeshMaterial3d(materials.add(CelMaterial::from_srgb(0.35, 0.40, 0.30))),
        RigidBody::Static,
        Collider::half_space(Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 10_000.0,
            shadow_maps_enabled: true,
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

pub fn capture_movement_input(keys: Res<ButtonInput<KeyCode>>, mut input: ResMut<MovementInput>) {
    input.0 = Vec2::new(
        axis(&keys, KeyCode::KeyD, KeyCode::KeyA),
        axis(&keys, KeyCode::KeyS, KeyCode::KeyW),
    );
}

fn axis(keys: &ButtonInput<KeyCode>, positive: KeyCode, negative: KeyCode) -> f32 {
    f32::from(keys.pressed(positive)) - f32::from(keys.pressed(negative))
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

/// Walk the body against the Avian world: move and slide along
/// walls (Quake III PM_SlideMove, as shipped by Avian), retry the
/// move raised by CLIMB when blocked so steps are climbed (Quake's
/// step slide move), then cast down to land on whatever is under
/// the feet. Nothing under the feet within CLIMB means the body
/// stays at its height; there is no gravity yet.
pub fn movement(
    input: Res<MovementInput>,
    time: Res<Time<Fixed>>,
    move_and_slide: MoveAndSlide,
    mut players: Query<(Entity, &mut Transform, &Player)>,
) {
    let wish = Vec3::new(input.0.x, 0.0, input.0.y);
    if wish == Vec3::ZERO {
        return;
    }
    let shape = body_shape();
    let config = MoveAndSlideConfig::default();
    let half = BODY_TOP / 2.0;
    for (entity, mut transform, player) in &mut players {
        let velocity = (Quat::from_rotation_y(player.yaw) * wish).normalize() * WALK_SPEED;
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        let start = transform.translation - Vec3::Y * EYE_HEIGHT + Vec3::Y * half;
        let slide = |from: Vec3| {
            move_and_slide
                .move_and_slide(
                    &shape,
                    from,
                    Quat::IDENTITY,
                    velocity,
                    time.delta(),
                    &config,
                    &filter,
                    |_| MoveAndSlideHitResponse::Accept,
                )
                .position
        };
        let flat = slide(start);
        let raised = slide(start + Vec3::Y * CLIMB);
        let progress = |p: Vec3| (p - start).with_y(0.0).length();
        let mut center = if progress(raised) > progress(flat) + 1e-4 {
            raised
        } else {
            flat
        };
        // Land: cast down by up to CLIMB plus the step lift.
        let drop = Vec3::NEG_Y * (CLIMB + (center.y - start.y).max(0.0));
        if let Some(hit) =
            move_and_slide.cast_move(&shape, center, Quat::IDENTITY, drop, 0.0, &filter)
        {
            center += drop.normalize() * hit.distance;
        } else {
            center.y = start.y;
        }
        transform.translation = center - Vec3::Y * half + Vec3::Y * EYE_HEIGHT;
    }
}
