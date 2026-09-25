//! The player and movement. The player is one picture (a coloured
//! square until pictures exist) at the bunker. Movement reads each
//! actor's drained actions, never keys, so the player and every NPC
//! walk through the same system. WASD moves on the screen: W is up.

use bevy::prelude::*;

use crate::input::{Action, Actions, ThisTick};
use crate::terrain::WorldRes;

/// Metres per second on foot (the 3D game's walking speed).
pub const WALK_SPEED: f32 = 5.0;
/// The player's square, in metres.
pub const PLAYER_SIZE: f32 = 0.8;
/// Above the ground.
const PLAYER_Z: f32 = 1.0;
const PLAYER_COLOR: Color = Color::srgb(0.9, 0.75, 0.3);

/// The one player. Carries everything an actor needs to be told what
/// to do.
#[derive(Component, Default)]
#[require(Actions, ThisTick, Transform)]
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

/// Every actor walks the way its latest move this tick says, at
/// walking speed; diagonal is no faster than straight.
pub fn movement(time: Res<Time>, mut actors: Query<(&ThisTick, &mut Transform)>) {
    let dt = time.delta_secs();
    for (this_tick, mut transform) in &mut actors {
        let wish = this_tick
            .0
            .iter()
            .rev()
            .find_map(|s| match s.action {
                Action::Move { x, y } => Some(Vec2::new(x, y)),
                _ => None,
            })
            .unwrap_or(Vec2::ZERO);
        let step = wish.normalize_or_zero() * WALK_SPEED * dt;
        transform.translation += step.extend(0.0);
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
