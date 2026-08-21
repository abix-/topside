//! The ONE place raw input is read (design.md, references.md
//! "Input": Quake 3's event loop). Every frame this maps Bevy's keys,
//! mouse buttons, and mouse motion through modforge's `Bindings` to
//! actions and pushes them onto the player's queue, stamped with the
//! fixed tick they apply to. Movement, fire, interaction, and the HUD
//! drain the queue; none of them knows what a key is. An NPC's AI
//! pushes the same actions. The journal records what was pushed.

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
pub use modforge::actions::{Action, ActionQueue, Bindings, Journal, Key, Stamped};

use crate::SimTick;
use crate::player::Player;

/// What this actor has been told to do. The keyboard fills the
/// player's; AI fills an NPC's.
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

/// Mouse look sensitivity: radians per pixel.
pub const MOUSE_SENS: f32 = 0.002;

/// The session journal: every action the player took, by tick, and
/// the world seed. Written on exit; replayed headless by tests.
#[derive(Resource, Default)]
pub struct JournalRes(pub Journal);

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
        KeyCode::KeyO => Key::O,
        KeyCode::KeyT => Key::T,
        KeyCode::Tab => Key::Tab,
        KeyCode::Escape => Key::Escape,
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

fn button_of(button: MouseButton) -> Option<Key> {
    Some(match button {
        MouseButton::Left => Key::MouseLeft,
        MouseButton::Right => Key::MouseRight,
        _ => return None,
    })
}

/// Read every raw input once per frame and push the player's
/// actions for the next fixed tick. While a panel is open or the
/// cursor is free, only the actions that make sense there get
/// through: the panel keys, cancel, and the hotbar; no walking, no
/// looking, no swinging.
pub fn read_input(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut motions: MessageReader<MouseMotion>,
    bindings: Res<BindingsRes>,
    tick: Res<SimTick>,
    hud: Res<crate::hud::HudRes>,
    cursors: Query<&CursorOptions, With<PrimaryWindow>>,
    mut players: Query<&mut Actions, With<Player>>,
    mut journal: ResMut<JournalRes>,
) {
    let Ok(mut actions) = players.single_mut() else {
        motions.clear();
        return;
    };
    let panel_open = hud.0.open_panel != modforge::hud::OpenPanel::None;
    let grabbed = cursors
        .single()
        .map(|c| c.grab_mode != CursorGrabMode::None)
        .unwrap_or(true);
    let in_world = !panel_open && grabbed;

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
            Action::Move { .. } | Action::Attack | Action::Use => in_world,
            Action::Drop | Action::Transfer { .. } => panel_open,
            Action::Inventory | Action::Cancel | Action::Hotbar { .. } => true,
            Action::Look { .. } => in_world,
        };
        if allowed {
            moved |= matches!(action, Action::Move { .. });
            push(&mut actions, &mut journal, next, action);
        }
    }
    // Every in-world frame states its movement, zero included, so a
    // released key stops the body and a step with no new frame can
    // tell "nothing new" from "standing still".
    if in_world && !moved {
        push(&mut actions, &mut journal, next, Action::Move { x: 0.0, y: 0.0 });
    }
    let mut delta = Vec2::ZERO;
    for m in motions.read() {
        delta += m.delta;
    }
    if in_world && delta != Vec2::ZERO {
        push(
            &mut actions,
            &mut journal,
            next,
            Action::Look {
                yaw: -delta.x * MOUSE_SENS,
                pitch: -delta.y * MOUSE_SENS,
            },
        );
    }
}

fn push(actions: &mut Actions, journal: &mut JournalRes, tick: u64, action: Action) {
    actions.0.push(tick, action);
    journal.0.record(Stamped { tick, action });
}

/// First thing each fixed step: every actor's due actions move from
/// its queue into `ThisTick` for the systems that follow. A step
/// with nothing new (two fixed steps in one frame) keeps the held
/// actions, move and attack, from the step before; taps and look
/// never repeat.
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
