//! What the player reads on screen, painted from modforge's `HudState`
//! (modforge decides what shows; this only draws it).

use bevy::prelude::*;
use modforge::hud::OpenPanel;

use crate::interact::HudRes;

/// The panel that shows a note's words while it is being read.
#[derive(Component)]
pub struct NotePanel;

pub fn setup(mut commands: Commands) {
    commands.spawn((
        NotePanel,
        Text::new(""),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(40.0),
            left: Val::Px(40.0),
            max_width: Val::Px(520.0),
            padding: UiRect::all(Val::Px(16.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.08, 0.07, 0.06, 0.9)),
        Visibility::Hidden,
    ));
}

/// The note panel shows while modforge says a note is open.
pub fn render_note(hud: Res<HudRes>, mut panels: Query<(&mut Text, &mut Visibility), With<NotePanel>>) {
    let Ok((mut text, mut visibility)) = panels.single_mut() else {
        return;
    };
    match (&hud.0.open_panel, &hud.0.reading) {
        (OpenPanel::Note, Some(note)) => {
            text.0 = format!("{}\n\n  {}", note.text, note.signed);
            *visibility = Visibility::Visible;
        }
        _ => *visibility = Visibility::Hidden,
    }
}
