//! Minimal HUD: the food clock, plus the door prompt when in reach.

use bevy::prelude::*;

use crate::bunker::{DoorState, FoodStore, near_door};
use crate::player::Player;

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
    door: Option<Res<DoorState>>,
    players: Query<&Transform, With<Player>>,
    mut huds: Query<&mut Text, With<Hud>>,
) {
    let Ok(mut text) = huds.single_mut() else {
        return;
    };
    let mut line = format!("Food: {:.1} days", food.days);
    if let (Some(door), Ok(player)) = (door, players.single())
        && near_door(player.translation)
    {
        line.push_str(if door.open {
            "\n[E] close door"
        } else {
            "\n[E] open door"
        });
    }
    text.0 = line;
}
