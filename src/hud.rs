//! Minimal HUD: the food clock, plus the door prompt when in reach.

use bevy::prelude::*;

use crate::bunker::FoodStore;
use crate::player::Player;
use crate::structure::{DOOR_REACH, Door};

#[derive(Component)]
pub struct Hud;

pub fn setup(mut commands: Commands) {
    commands.spawn((
        Hud,
        Text::new(""),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
            ..default()
        },
    ));
}

pub fn update(
    food: Res<FoodStore>,
    doors: Query<&Door>,
    players: Query<&Transform, With<Player>>,
    mut huds: Query<&mut Text, With<Hud>>,
) {
    let Ok(mut text) = huds.single_mut() else {
        return;
    };
    let mut line = format!("Food: {:.1} days", food.days);
    if let Ok(player) = players.single()
        && let Some(door) = doors
            .iter()
            .find(|d| player.translation.distance(d.closed_center) < DOOR_REACH)
    {
        line.push_str(if door.open {
            "\n[E] close door"
        } else {
            "\n[E] open door"
        });
    }
    text.0 = line;
}
