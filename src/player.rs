//! The player and movement. The player is one picture (a coloured
//! square until pictures exist) at the bunker. Movement reads each
//! actor's drained actions, never keys, so the player and every NPC
//! walk through the same system. WASD moves on the screen: W is up.

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::input::{ActionQueue, Actions, ThisTick};
use crate::terrain::WorldRes;

/// Metres per second on foot (the 3D game's walking speed).
pub const WALK_SPEED: f32 = 5.0;
/// The player's square, in metres.
pub const PLAYER_SIZE: f32 = 0.8;
/// Above the ground.
const PLAYER_Z: f32 = 1.0;
const PLAYER_COLOR: Color = Color::srgb(0.9, 0.75, 0.3);

/// The one player. Carries everything an actor needs to be told what
/// to do, and a body that walls stop.
#[derive(Component, Default)]
#[require(
    Actions,
    ThisTick,
    Transform,
    RigidBody = RigidBody::Kinematic,
    Collider = Collider::circle(PLAYER_SIZE / 2.0)
)]
pub struct Player;

/// The one place the player enters the world: at the bunker.
pub fn spawn_player(mut commands: Commands, world: Res<WorldRes>) {
    commands.spawn((
        Player,
        Sprite {
            color: PLAYER_COLOR,
            custom_size: Some(Vec2::splat(PLAYER_SIZE)),
            ..default()
        },
        Transform::from_translation(world.0.def.bunker.extend(PLAYER_Z)),
    ));
}

/// Every actor walks the way its move this tick says, at walking
/// speed, and slides along whatever stops it (Quake III's
/// PM_SlideMove, as shipped by Avian). Diagonal is no faster than
/// straight.
pub fn movement(
    time: Res<Time<Fixed>>,
    move_and_slide: MoveAndSlide,
    mut actors: Query<(Entity, &ThisTick, &Collider, &mut Transform)>,
) {
    let config = MoveAndSlideConfig::default();
    for (entity, this_tick, shape, mut transform) in &mut actors {
        let (x, y) = ActionQueue::movement(&this_tick.0);
        let velocity = Vec2::new(x, y).normalize_or_zero() * WALK_SPEED;
        if velocity == Vec2::ZERO {
            continue;
        }
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        let to = move_and_slide
            .move_and_slide(
                shape,
                transform.translation.truncate(),
                0.0,
                velocity,
                time.delta(),
                &config,
                &filter,
                |_| MoveAndSlideHitResponse::Accept,
            )
            .position;
        transform.translation = to.extend(transform.translation.z);
    }
}

/// The camera stays on the player.
pub fn camera_follow(
    player: Query<&Transform, (With<Player>, Without<Camera2d>)>,
    mut cameras: Query<&mut Transform, With<Camera2d>>,
) {
    let Ok(player) = player.single() else {
        return;
    };
    for mut camera in &mut cameras {
        camera.translation.x = player.translation.x;
        camera.translation.y = player.translation.y;
    }
}
