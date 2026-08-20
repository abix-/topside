//! Minimal HUD: the food clock, plus the interact prompt from the
//! raycast. Reads the LookingAt resource for the prompt.

use bevy::prelude::*;
use modforge::hud::{prompt_for, InteractKind};

use crate::bunker::FoodStore;
use crate::interact::LookingAt;

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
    looking_at: Res<LookingAt>,
    mut huds: Query<&mut Text, With<Hud>>,
) {
    let Ok(mut text) = huds.single_mut() else {
        return;
    };
    let mut line = format!("Food: {:.1} days", food.days);
    if let Some(kind) = looking_at.kind {
        let prompt = prompt_for(kind, None, None);
        line.push_str("\n");
        line.push_str(&prompt.text);
    }
    text.0 = line;
}
