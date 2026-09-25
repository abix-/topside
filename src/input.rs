//! The ONE place raw input is read (references.md "Input": Quake 3's
//! event loop). Every frame this maps Bevy's keys through modforge's
//! `Bindings` to actions and pushes them onto the player's queue,
//! stamped with the fixed tick they apply to. Movement drains the
//! queue and never knows what a key is; an NPC's AI will push the same
//! actions.

use bevy::prelude::*;
pub use modforge::actions::{Action, ActionQueue, Bindings, Key, Stamped};

use crate::SimTick;
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
        _ => return None,
    })
}

/// Read the keys once per frame and push the player's movement for the
/// next fixed tick.
pub fn read_input(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<BindingsRes>,
    tick: Res<SimTick>,
    mut players: Query<&mut Actions, With<Player>>,
) {
    let Ok(mut actions) = players.single_mut() else {
        return;
    };
    let pressed: Vec<Key> = keys.get_pressed().filter_map(|k| key_of(*k)).collect();
    let just_pressed: Vec<Key> = keys.get_just_pressed().filter_map(|k| key_of(*k)).collect();
    // The next fixed step consumes what this frame produced.
    let next = tick.0 + 1;
    let mut moved = false;
    for action in bindings.0.resolve(&pressed, &just_pressed) {
        if let Action::Move { .. } = action {
            moved = true;
            actions.0.push(next, action);
        }
    }
    // Every frame states its movement, zero included, so a released
    // key stops the body.
    if !moved {
        actions.0.push(next, Action::Move { x: 0.0, y: 0.0 });
    }
}

/// First thing each fixed step: every actor's due actions move from
/// its queue into `ThisTick` for the systems that follow. A step with
/// nothing new (two fixed steps in one frame) keeps the held move from
/// the step before.
pub fn drain_actions(tick: Res<SimTick>, mut actors: Query<(&mut Actions, &mut ThisTick)>) {
    for (mut queue, mut this_tick) in &mut actors {
        let due = queue.0.drain_through(tick.0);
        if due.is_empty() {
            this_tick.0.retain(|s| matches!(s.action, Action::Move { .. }));
        } else {
            this_tick.0 = due;
        }
    }
}
