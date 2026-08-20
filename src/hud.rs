//! The HUD: writer systems fill HudRes from game state, one
//! renderer reads only HudRes to paint the screen text.

use bevy::prelude::*;
use modforge::hud::{self, prompt_for};

use crate::bunker::FoodStore;
use crate::interact::LookingAt;

#[derive(Resource)]
pub struct HudRes(pub hud::HudState);

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

pub fn write_vitals(food: Res<FoodStore>, mut state: ResMut<HudRes>) {
    state.0.vitals.food = food.days;
    state.0.vitals.food_max = 3.0;
}

pub fn write_prompt(looking_at: Res<LookingAt>, mut state: ResMut<HudRes>) {
    if let Some(kind) = looking_at.kind {
        state.0.prompt = prompt_for(kind, None, None);
    } else {
        state.0.prompt = Default::default();
    }
}

pub fn render(state: Res<HudRes>, mut huds: Query<&mut Text, With<Hud>>) {
    let Ok(mut text) = huds.single_mut() else {
        return;
    };
    let mut line = format!("Food: {:.1} days", state.0.vitals.food);
    if state.0.prompt.can_interact {
        line.push('\n');
        line.push_str(&state.0.prompt.text);
    }
    text.0 = line;
}
