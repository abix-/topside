//! The ONE place raw input is read (references.md "Input": Quake 3's
//! event loop). Every frame this maps Bevy's keys through modforge's
//! `Bindings` to actions and pushes them onto the player's queue,
//! stamped with the fixed tick they apply to. Movement drains the
//! queue and never knows what a key is; an NPC's AI will push the same
//! actions.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use modforge::hud::OpenPanel;
pub use modforge::actions::{Action, ActionQueue, Bindings, Key, Stamped};

use crate::SimTick;
use crate::interact::HudRes;
use crate::player::Player;

/// What this actor has been told to do. The keyboard fills the
/// player's; AI will fill an NPC's.
#[derive(Component, Default, Debug, PartialEq)]
pub struct Actions(pub ActionQueue);

/// The actions drained for the current fixed tick, for systems to
/// read. Refilled each fixed step from the queue.
#[derive(Component, Default, Debug, PartialEq)]
pub struct ThisTick(pub Vec<Stamped>);

/// The key bindings, as data.
#[derive(Resource)]
pub struct BindingsRes(pub Bindings);

impl Default for BindingsRes {
    fn default() -> Self {
        Self(Bindings::defaults())
    }
}

/// Bevy's key codes to the names bindings use. Unlisted keys are
/// unbound.
fn key_of(code: KeyCode) -> Option<Key> {
    Some(match code {
        KeyCode::KeyW => Key::W,
        KeyCode::KeyA => Key::A,
        KeyCode::KeyS => Key::S,
        KeyCode::KeyD => Key::D,
        KeyCode::KeyE => Key::E,
        KeyCode::KeyI => Key::I,
        KeyCode::KeyT => Key::T,
        KeyCode::Tab => Key::Tab,
        KeyCode::Escape => Key::Escape,
        KeyCode::Enter => Key::Enter,
        KeyCode::ShiftLeft => Key::ShiftLeft,
        KeyCode::ShiftRight => Key::ShiftRight,
        KeyCode::Digit1 => Key::Digit1,
        KeyCode::Digit2 => Key::Digit2,
        KeyCode::Digit3 => Key::Digit3,
        KeyCode::Digit4 => Key::Digit4,
        KeyCode::Digit5 => Key::Digit5,
        KeyCode::Digit6 => Key::Digit6,
        KeyCode::Digit7 => Key::Digit7,
        KeyCode::Digit8 => Key::Digit8,
        KeyCode::Digit9 => Key::Digit9,
        KeyCode::Digit0 => Key::Digit0,
        _ => return None,
    })
}

/// Read the keys and the mouse once per frame and push the player's
/// actions for the next fixed tick: movement, use, attack, the point
/// under the cursor to face, and the panel and hotbar keys. While a
/// panel is open only the panel keys, the hotbar, and use get through
/// (the 3D game's rule): no walking, aiming, or swinging.
pub fn read_input(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    bindings: Res<BindingsRes>,
    tick: Res<SimTick>,
    hud: Res<HudRes>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    mut players: Query<&mut Actions, With<Player>>,
) {
    let Ok(mut actions) = players.single_mut() else {
        return;
    };
    let in_world = hud.0.open_panel == OpenPanel::None;
    if in_world
        && let (Ok(window), Ok((camera, camera_at))) = (windows.single(), cameras.single())
        && let Some(cursor) = window.cursor_position()
        && let Ok(point) = camera.viewport_to_world_2d(camera_at, cursor)
    {
        actions.0.push(tick.0 + 1, Action::Aim { x: point.x, y: point.y });
    }
    let pressed: Vec<Key> = keys
        .get_pressed()
        .filter_map(|k| key_of(*k))
        .chain(buttons.get_pressed().filter_map(|b| button_of(*b)))
        .collect();
    let just_pressed: Vec<Key> = keys
        .get_just_pressed()
        .filter_map(|k| key_of(*k))
        .chain(buttons.get_just_pressed().filter_map(|b| button_of(*b)))
        .collect();
    // The next fixed step consumes what this frame produced.
    let next = tick.0 + 1;
    let mut moved = false;
    for action in bindings.0.resolve(&pressed, &just_pressed) {
        let allowed = match action {
            Action::Move { .. } | Action::Attack => in_world,
            Action::Use
            | Action::Inventory
            | Action::Cancel
            | Action::Hotbar { .. }
            | Action::Transfer { .. }
            | Action::Respawn => true,
            Action::Look { .. } | Action::Aim { .. } | Action::Drop | Action::Fly => false,
        };
        if allowed {
            moved |= matches!(action, Action::Move { .. });
            actions.0.push(next, action);
        }
    }
    // Every frame states its movement, zero included, so a released
    // key stops the body.
    if !moved {
        actions.0.push(next, Action::Move { x: 0.0, y: 0.0 });
    }
}

fn button_of(button: MouseButton) -> Option<Key> {
    Some(match button {
        MouseButton::Left => Key::MouseLeft,
        MouseButton::Right => Key::MouseRight,
        _ => return None,
    })
}

/// First thing each fixed step: every actor's due actions move from
/// its queue into `ThisTick` for the systems that follow. A step with
/// nothing new (two fixed steps in one frame) keeps the held actions,
/// move and attack, from the step before.
pub fn drain_actions(tick: Res<SimTick>, mut actors: Query<(&mut Actions, &mut ThisTick)>) {
    for (mut queue, mut this_tick) in &mut actors {
        let due = queue.0.drain_through(tick.0);
        if due.is_empty() {
            this_tick.0.retain(|s| matches!(s.action, Action::Move { .. } | Action::Attack));
        } else {
            this_tick.0 = due;
        }
    }
}
