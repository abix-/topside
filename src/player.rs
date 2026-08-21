//! First-person camera on a flat ground plane. Skeleton controls:
//! click grabs the cursor, mouse looks, WASD walks, Escape releases.

use crate::actor::{Equipment, Facing, Hotbar, Inventory};
use crate::combat::{Firing, Health, Protection};
use crate::input::{Action, ActionQueue, Actions, ThisTick};
use crate::structure::{BODY_TOP, CLIMB, body_shape};
use crate::survival::Survival;
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

pub use crate::actor::EYE_HEIGHT;

/// The human-driven actor. Carries exactly what an NPC carries
/// (actor.rs, combat.rs, input.rs); the camera and who fills the
/// action queue are the only difference.
#[derive(Component, Default)]
#[require(
    Facing, Inventory, Hotbar, Equipment, Survival, Health, Protection, Firing, Actions, ThisTick
)]
pub struct Player;

const WALK_SPEED: f32 = 5.0;
/// Admin flight (Quake's noclip): fast, along the look direction,
/// through everything.
const FLY_SPEED: f32 = 40.0;
const PITCH_LIMIT: f32 = 1.54;

/// The actor is flying through everything (V toggles it).
#[derive(Component)]
pub struct Flying;

/// `Fly` among this tick's actions toggles `Flying` on the player.
pub fn toggle_fly(
    mut commands: Commands,
    players: Query<(Entity, &ThisTick, Has<Flying>), With<Player>>,
) {
    for (entity, this_tick, flying) in &players {
        if !ActionQueue::has(&this_tick.0, Action::Fly) {
            continue;
        }
        if flying {
            commands.entity(entity).remove::<Flying>();
        } else {
            commands.entity(entity).insert(Flying);
        }
    }
}

/// The sun and the camera. The ground is spawned by bunker::setup
/// once it knows where the basements are (ground.rs).
pub fn setup_scene(mut commands: Commands) {
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
        Player,
    ));
}

/// The cursor is the one raw-input rule that is not a game action:
/// a click into the window grabs it, and `Cancel` with no panel open
/// frees it (the panel keys own it while a panel is open).
pub fn cursor_grab(
    mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>,
    buttons: Res<ButtonInput<MouseButton>>,
    hud: Res<crate::hud::HudRes>,
    players: Query<&ThisTick, With<Player>>,
) {
    let Ok(mut cursor) = cursors.single_mut() else {
        return;
    };
    let panel_open = hud.0.open_panel != modforge::hud::OpenPanel::None;
    if buttons.just_pressed(MouseButton::Left) && !panel_open {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
    let cancelled = players
        .single()
        .is_ok_and(|t| ActionQueue::has(&t.0, Action::Cancel));
    if cancelled && !panel_open {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}

/// Turn every actor by this tick's look actions.
pub fn look(mut actors: Query<(&mut Transform, &mut Facing, &ThisTick)>) {
    for (mut transform, mut facing, this_tick) in &mut actors {
        let (yaw, pitch) = ActionQueue::look(&this_tick.0);
        if yaw == 0.0 && pitch == 0.0 {
            continue;
        }
        facing.yaw += yaw;
        facing.pitch = (facing.pitch + pitch).clamp(-PITCH_LIMIT, PITCH_LIMIT);
        transform.rotation = facing.rotation();
    }
}

/// Walk the body against the Avian world: move and slide along
/// walls (Quake III PM_SlideMove, as shipped by Avian), retry the
/// move raised by CLIMB when blocked so steps are climbed (Quake's
/// step slide move), then cast down to land on whatever is under
/// the feet. Nothing under the feet within CLIMB means the body
/// stays at its height; there is no gravity yet.
pub fn movement(
    time: Res<Time<Fixed>>,
    move_and_slide: MoveAndSlide,
    mut actors: Query<(
        Entity,
        &mut Transform,
        &Facing,
        &ThisTick,
        Option<&Children>,
        Has<Flying>,
    )>,
) {
    let shape = body_shape();
    let config = MoveAndSlideConfig::default();
    let half = BODY_TOP / 2.0;
    for (entity, mut transform, facing, this_tick, children, flying) in &mut actors {
        // Actions say y is forward; the world's forward is -z.
        let (x, y) = ActionQueue::movement(&this_tick.0);
        let wish = Vec3::new(x, 0.0, -y);
        if wish == Vec3::ZERO {
            continue;
        }
        if flying {
            // Along the full look direction, pitch included, through
            // everything.
            let step = (facing.rotation() * wish).normalize() * FLY_SPEED * time.delta_secs();
            transform.translation += step;
            continue;
        }
        let velocity = (Quat::from_rotation_y(facing.yaw) * wish).normalize() * WALK_SPEED;
        // An actor never collides with itself: not its root, not the
        // body hanging under it.
        let own: Vec<Entity> = std::iter::once(entity)
            .chain(children.into_iter().flat_map(|c| c.iter()))
            .collect();
        let filter = SpatialQueryFilter::from_excluded_entities(own);
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
