//! The one interact path: a person's Use this tick reaches the nearest
//! interactable thing in reach, modforge's `hud::interact` decides
//! what happens, and topside carries out the engine side.

use std::f32::consts::FRAC_PI_2;

use avian2d::prelude::*;
use bevy::prelude::*;
use modforge::hud::{HudState, InteractKind, InteractResult, interact};

use crate::actor::Inventory;
use crate::input::{Action, ThisTick};
use crate::player::Player;

/// How far a person reaches to use something, in metres.
pub const REACH: f32 = 1.5;

/// Something a person can use, and what kind of thing it is.
#[derive(Component)]
pub struct Interactable {
    pub kind: InteractKind,
}

/// A door. Open, it swings a quarter turn about its hinge and stops
/// nothing.
#[derive(Component)]
pub struct Door {
    pub open: bool,
    pub closed_center: Vec2,
    pub hinge: Vec2,
}

/// The HUD's state, modforge's.
#[derive(Resource)]
pub struct HudRes(pub HudState);

impl Default for HudRes {
    fn default() -> Self {
        Self(HudState::new())
    }
}

/// The player's Use this tick reaches the nearest interactable within
/// `REACH`; modforge says what happens and topside does it.
pub fn interact_on_use(
    mut commands: Commands,
    mut hud: ResMut<HudRes>,
    mut players: Query<(&Transform, &ThisTick, &mut Inventory), With<Player>>,
    mut things: Query<(Entity, &mut Interactable, Option<&mut Door>, &mut Transform), Without<Player>>,
) {
    let Ok((player, this_tick, mut inventory)) = players.single_mut() else {
        return;
    };
    if !this_tick.0.iter().any(|s| matches!(s.action, Action::Use)) {
        return;
    }
    let at = player.translation.truncate();
    let distance = |door: Option<&Door>, transform: &Transform| {
        door.map_or(transform.translation.truncate(), |d| d.closed_center).distance(at)
    };
    let Some((entity, mut thing, door, mut transform)) = things
        .iter_mut()
        .filter(|(_, _, door, transform)| distance(door.as_deref(), transform) <= REACH)
        .min_by(|a, b| distance(a.2.as_deref(), &a.3).total_cmp(&distance(b.2.as_deref(), &b.3)))
    else {
        return;
    };
    if interact(thing.kind, &mut inventory.0, None, 0, &mut hud.0) == InteractResult::ToggleDoor
        && let Some(mut door) = door
    {
        door.open = !door.open;
        thing.kind = InteractKind::Door { open: door.open };
        let turn = if door.open { FRAC_PI_2 } else { 0.0 };
        let rotation = Quat::from_rotation_z(turn);
        let from_hinge = (door.closed_center - door.hinge).extend(0.0);
        transform.translation = (door.hinge.extend(0.0) + rotation * from_hinge).with_z(transform.translation.z);
        transform.rotation = rotation;
        if door.open {
            commands.entity(entity).insert(ColliderDisabled);
        } else {
            commands.entity(entity).remove::<ColliderDisabled>();
        }
    }
}
