//! The HUD: writer systems fill HudRes from game state, one
//! renderer reads only HudRes to paint the screen text.

use bevy::prelude::*;
use modforge::hud::{HudState, prompt_for};

use crate::interact::{LookingAt, WorldItem};
use crate::player::Player;
use crate::survival::Survival;

#[derive(Resource)]
pub struct HudRes(pub HudState);

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

pub fn write_prompt(
    looking_at: Res<LookingAt>,
    items: Query<&WorldItem>,
    mut state: ResMut<HudRes>,
) {
    if let Some(kind) = looking_at.kind {
        let (name, count) = looking_at
            .entity
            .and_then(|e| items.get(e).ok())
            .map(|w| (Some(w.stack.item.as_str()), Some(w.stack.count)))
            .unwrap_or((None, None));
        state.0.prompt = prompt_for(kind, name, count);
    } else {
        state.0.prompt = Default::default();
    }
}

/// Paint the HUD. Reads the player's survival stats in place; the
/// player entity is the one owner, the HUD never copies them.
pub fn render(
    state: Res<HudRes>,
    players: Query<&Survival, With<Player>>,
    mut huds: Query<&mut Text, With<Hud>>,
) {
    let Ok(mut text) = huds.single_mut() else {
        return;
    };
    let Ok(survival) = players.single() else {
        return;
    };
    let mut line = format!(
        "Hunger: {:.0}  Thirst: {:.0}",
        survival.0.hunger, survival.0.thirst
    );
    if state.0.prompt.can_interact {
        line.push('\n');
        line.push_str(&state.0.prompt.text);
    }
    text.0 = line;
}
