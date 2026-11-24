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
#[derive(Component, Default, Debug, PartialEq, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Actions(pub ActionQueue);

/// The actions drained for the current fixed tick, for systems to
/// read. Refilled each fixed step from the queue.
#[derive(Component, Default, Debug, PartialEq, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
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
        KeyCode::KeyC => Key::C,
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

/// What the keys and the mouse did since a tick last took them: every key
/// and button pressed (a tap let go again before any tick ran included),
/// and how far the mouse looked. Filled every frame (`gather_input`) and
/// taken whole by the next tick (`input_to_actions`), so a press acts at
/// the next tick the game runs, however many frames that takes, paused or
/// stepped (authority.md rule 5). The real keys and the `key` and
/// `pointer` ops come through here alike.
#[derive(Resource, Default)]
pub struct PendingInput {
    pressed: Vec<Key>,
    look: Vec2,
}

/// Keys the `key` op pressed, not yet taken by a tick, and keys it holds
/// down until it lets them go: the op's hands, taken by the next tick
/// whatever frame it comes in, as the real keys are (`input_to_actions`).
static OP_KEYS: std::sync::Mutex<(Vec<Key>, Vec<Key>)> = std::sync::Mutex::new((Vec::new(), Vec::new()));

/// The `key` op: `code` tapped (None), held down (Some(true)) or let go
/// (Some(false)), the left mouse button when `code` is None; answers
/// whether it is a key the bindings know.
pub fn press_from_op(code: Option<KeyCode>, hold: Option<bool>) -> bool {
    let Some(key) = code.map_or(Some(Key::MouseLeft), key_of) else {
        return false;
    };
    if let Ok(mut keys) = OP_KEYS.lock() {
        let (pressed, held) = &mut *keys;
        match hold {
            Some(false) => held.retain(|k| *k != key),
            Some(true) => {
                pressed.push(key);
                held.push(key);
            }
            None => pressed.push(key),
        }
    }
    true
}

/// Mouse movement the `look` op gave, in the mouse's pixels, not yet taken
/// by a tick: added to the real mouse's look, and turned by the same
/// LOOK_PER_PIXEL (`input_to_actions`).
static OP_LOOK: std::sync::Mutex<Vec2> = std::sync::Mutex::new(Vec2::ZERO);

/// The `look` op: move the mouse by `by` pixels (x right, y down), as a
/// hand does while the game holds the cursor.
pub fn look_from_op(by: Vec2) {
    if let Ok(mut look) = OP_LOOK.lock() {
        *look += by;
    }
}

/// Every frame: the keys and buttons pressed this frame, and the mouse's
/// look while the cursor is held by the game in the world, kept until a
/// tick takes them.
pub fn gather_input(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    cursors: Query<&CursorOptions, With<PrimaryWindow>>,
    players: Query<&Hud, With<Player>>,
    mut pending: ResMut<PendingInput>,
) {
    let pressed: Vec<Key> = keys.get_just_pressed().filter_map(|k| key_of(*k)).chain(buttons.get_just_pressed().filter_map(|b| button_of(*b))).collect();
    for key in pressed {
        if !pending.pressed.contains(&key) {
            pending.pressed.push(key);
        }
    }
    let in_world = players.single().is_ok_and(|hud| hud.0.open_panel == OpenPanel::None);
    // The mouse looks only while the cursor is held by the game.
    let grabbed = cursors.single().is_ok_and(|c| c.grab_mode != CursorGrabMode::None);
    if in_world && grabbed {
        pending.look += motion.delta;
    }
}

/// First in every fixed step: what the keys and mouse did since the last
/// tick becomes the player's actions for this tick: movement, use,
/// attack, where they look, and the panel and hotbar keys. While a panel
/// is open only the panel keys, the hotbar, and use get through (the 3D
/// game's rule): no walking, looking, or swinging. WASD is read relative
/// to where the player faces (W forward) and pushed as a direction on the
/// ground, the same `Move` every NPC's brain pushes.
pub fn input_to_actions(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    bindings: Res<BindingsRes>,
    tick: Res<SimTick>,
    hovered: Res<HoveredSlot>,
    mut pending: ResMut<PendingInput>,
    mut players: Query<(&mut Actions, &Hud, &crate::actor::Facing), With<Player>>,
) {
    let mut taken = std::mem::take(&mut *pending);
    // The `look` op's mouse movement, the same as the real mouse's.
    if let Ok(mut look) = OP_LOOK.lock() {
        taken.look += std::mem::take(&mut *look);
    }
    // The `key` op's presses and held keys, the same as real ones.
    let op_held: Vec<Key> = match OP_KEYS.lock() {
        Ok(mut keys) => {
            for key in std::mem::take(&mut keys.0) {
                if !taken.pressed.contains(&key) {
                    taken.pressed.push(key);
                }
            }
            keys.1.clone()
        }
        Err(_) => Vec::new(),
    };
    let Ok((mut actions, hud, facing)) = players.single_mut() else {
        return;
    };
    // The tick about to run (this runs before it counts).
    let next = tick.0 + 1;
    let in_world = hud.0.open_panel == OpenPanel::None;
    if in_world && taken.look != Vec2::ZERO {
        actions.0.push(
            next,
            Action::Look {
                yaw: taken.look.x * LOOK_PER_PIXEL,
                pitch: -taken.look.y * LOOK_PER_PIXEL,
            },
        );
    }
    // Held now, and pressed since the last tick (a tap counts as held for
    // the tick that takes it).
    let mut pressed: Vec<Key> = keys.get_pressed().filter_map(|k| key_of(*k)).chain(buttons.get_pressed().filter_map(|b| button_of(*b))).collect();
    for key in taken.pressed.iter().chain(&op_held) {
        if !pressed.contains(key) {
            pressed.push(*key);
        }
    }
    let just_pressed = taken.pressed;
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
            Action::Move { .. } | Action::Attack | Action::Roll | Action::Jump => in_world,
            Action::Use
            | Action::Inventory
            | Action::Cancel
            | Action::Hotbar { .. }
            | Action::Transfer { .. }
            | Action::Drop { .. }
            | Action::UseSlot { .. }
            | Action::Fill { .. }
            | Action::Pour { .. }
            | Action::Craft { .. }
            | Action::Map
            | Action::Talk { .. }
            | Action::Respawn => true,
            Action::Look { .. } | Action::Aim { .. } | Action::Go { .. } | Action::Knock | Action::OfferSlot { .. } | Action::Deal | Action::Fly => false,
        };
        if allowed {
            moved |= matches!(action, Action::Move { .. });
            actions.0.push(next, action);
        }
    }
    // Every tick states its movement, zero included, so a released key
    // stops the body.
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
