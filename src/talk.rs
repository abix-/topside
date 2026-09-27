//! The one talk path (operator, 2026-09-27): every person's `Talk` this
//! tick, the player's and the brain's alike, reaches one person in reach;
//! modforge `talk` decides what comes of it and writes both memories.

use bevy::prelude::*;
use modforge::actions::Said;
use modforge::actor::ActorId;

use crate::Ground;
use crate::actor::{Actor, ActorMapRes, Dead, Inventory, Memory};
use crate::input::{Action, ThisTick};

/// How far away a person can be talked to, in metres.
pub const TALK_REACH: f32 = 3.0;

/// Every person's `Talk` this tick: the listener, by ActorId, alive and
/// within `TALK_REACH`; the offered item from the speaker's inventory.
pub fn talk(
    tick: Res<crate::SimTick>,
    map: Res<ActorMapRes>,
    episode: Res<crate::Episode>,
    speakers: Query<(Entity, &Actor, &ThisTick), Without<Dead>>,
    mut people: Query<(&Transform, &mut Memory, &Inventory), Without<Dead>>,
) {
    for (speaker, id, this_tick) in &speakers {
        for said in this_tick.0.iter().filter_map(|s| match s.action {
            Action::Talk { to, said } => Some((to, said)),
            _ => None,
        }) {
            let (to, said) = said;
            let Some(listener) = map.0.handle_of(ActorId(to)) else {
                continue;
            };
            let Ok([(from, mut speaker_memory, inventory), (at, mut listener_memory, _)]) = people.get_many_mut([speaker, listener]) else {
                continue;
            };
            if from.translation.ground().distance(at.translation.ground()) > TALK_REACH {
                continue;
            }
            let offered = match said {
                Said::Offer { slot } => inventory.0.slots.get(slot).and_then(Option::as_ref).map(|s| s.item.clone()),
                _ => None,
            };
            // A line's words, held by the episode that wrote them.
            let words = match said {
                Said::Line { line, .. } => episode.spoken.get(line as usize).map(String::as_str),
                _ => None,
            };
            let answer = modforge::talk::talk(
                said,
                (id.0, &mut speaker_memory.0),
                (ActorId(to), &mut listener_memory.0),
                (offered.as_deref(), words),
                tick.0,
            );
            debug!("{:?} to {to}: {said:?}, {answer:?}", id.0);
        }
    }
}
