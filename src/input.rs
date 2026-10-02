//! The ONE place raw input is read (references.md "Input": Quake 3's
//! event loop). Every frame this maps Bevy's keys through modforge's
//! `Bindings` to actions and pushes them onto the player's queue,
//! stamped with the fixed tick they apply to. Movement drains the
//! queue and never knows what a key is; an NPC's AI will push the same
//! actions.

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use modforge::hud::OpenPanel;
pub use modforge::actions::{Action, ActionQueue, Bindings, Key, Stamped};

use crate::SimTick;
use crate::hud::HoveredSlot;
use crate::interact::Hud;
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
        KeyCode::KeyM => Key::M,
        KeyCode::KeyT => Key::T,
        KeyCode::Tab => Key::Tab,
        KeyCode::Escape => Key::Escape,
        KeyCode::Enter => Key::Enter,
        KeyCode::Space => Key::Space,
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

/// Radians turned per pixel the mouse moves. Tuning.
const LOOK_PER_PIXEL: f32 = 0.0025;

/// Read the keys and the mouse once per frame and push the player's
/// actions for the next fixed tick: movement, use, attack, where they
/// look, and the panel and hotbar keys. While a panel is open only the
/// panel keys, the hotbar, and use get through (the 3D game's rule): no
/// walking, looking, or swinging. WASD is read relative to where the
/// player faces (W forward) and pushed as a direction on the ground, the
/// same `Move` every NPC's brain pushes.
pub fn read_input(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    bindings: Res<BindingsRes>,
    tick: Res<SimTick>,
    hovered: Res<HoveredSlot>,
    motion: Res<AccumulatedMouseMotion>,
    cursors: Query<&CursorOptions, With<PrimaryWindow>>,
    mut players: Query<(&mut Actions, &Hud, &crate::actor::Facing), With<Player>>,
) {
    let Ok((mut actions, hud, facing)) = players.single_mut() else {
        return;
    };
    let in_world = hud.0.open_panel == OpenPanel::None;
    // The mouse looks only while the cursor is held by the game.
    let grabbed = cursors.single().is_ok_and(|c| c.grab_mode != CursorGrabMode::None);
    if in_world && grabbed && motion.delta != Vec2::ZERO {
        actions.0.push(
            tick.0 + 1,
            Action::Look {
                yaw: motion.delta.x * LOOK_PER_PIXEL,
                pitch: -motion.delta.y * LOOK_PER_PIXEL,
            },
        );
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
        // T moves the stack under the cursor.
        let action = match action {
            Action::Transfer { half, slot: None } => match hovered.0 {
                Some(slot) => Action::Transfer { half, slot: Some(slot) },
                None => continue,
            },
            // While a conversation's choices show, a number key says the
            // choice of that number, not the hotbar (episodes.md "How a
            // conversation looks").
            Action::Hotbar { index } => match hud.0.talking.as_ref().filter(|t| !t.choices.is_empty()) {
                Some(talking) => match modforge::hud::choice_for_key(talking, index) {
                    Some(said) => Action::Talk { to: talking.with, said },
                    None => continue,
                },
                None => Action::Hotbar { index },
            },
            // O drops the stack under the cursor.
            Action::Drop { slot: None } => match hovered.0 {
                Some(slot) => Action::Drop { slot: Some(slot) },
                None => continue,
            },
            // W forward, D to the right of where the player faces, turned
            // into a direction on the ground (the camera's right is the
            // facing turned a quarter counter-clockwise on the ground).
            Action::Move { x, y } => {
                let forward = facing.direction();
                let right = forward.perp();
                let on_ground = forward * y + right * x;
                Action::Move {
                    x: on_ground.x,
                    y: on_ground.y,
                }
            }
            other => other,
        };
        let allowed = match action {
            Action::Move { .. } | Action::Attack | Action::Roll => in_world,
            Action::Use
            | Action::Inventory
            | Action::Cancel
            | Action::Hotbar { .. }
            | Action::Transfer { .. }
            | Action::Drop { .. }
            | Action::UseSlot { .. }
            | Action::Craft { .. }
            | Action::Map
            | Action::Talk { .. }
            | Action::Respawn => true,
            Action::Look { .. } | Action::Aim { .. } | Action::Go { .. } | Action::Knock | Action::Fly => false,
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

/// The cursor is the one raw-input rule that is not a game action (the 3D
/// game's): a click in the world grabs it so the mouse looks; Escape, or
/// a panel opening, frees it for the panel.
pub fn cursor_grab(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>,
    players: Query<&Hud, With<Player>>,
) {
    let Ok(mut cursor) = cursors.single_mut() else {
        return;
    };
    let panel_open = players.single().is_ok_and(|hud| hud.0.open_panel != OpenPanel::None);
    if panel_open || keys.just_pressed(KeyCode::Escape) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    } else if buttons.just_pressed(MouseButton::Left) {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
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
/// nothing new keeps the held actions from the step before: move,
/// attack, the point aimed at, and where they go (two fixed steps in one
/// frame, and a person between decisions keeps doing what they decided).
pub fn drain_actions(tick: Res<SimTick>, mut actors: Query<(&mut Actions, &mut ThisTick)>) {
    for (mut queue, mut this_tick) in &mut actors {
        let due = queue.0.drain_through(tick.0);
        if due.is_empty() {
            this_tick
                .0
                .retain(|s| matches!(s.action, Action::Move { .. } | Action::Attack | Action::Aim { .. } | Action::Go { .. }));
        } else {
            this_tick.0 = due;
        }
    }
}
