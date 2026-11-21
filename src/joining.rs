//! Joining (todo 11af, operator 2026-10-02): a person moves to another
//! group, their home, store and standing moving with them. A stranger the
//! player asks to stay says yes feeling well enough toward the player (the
//! "agrees to stay" line, content `AGREES_TO_STAY`); anyone leaves on their
//! own for a group they feel better toward than their own (life.md "How a
//! person feels about others": a group felt as the average of the feelings
//! toward its members they remember). Prior art: RimWorld's pawns joining
//! and Mount & Blade's lords changing sides.

use bevy::prelude::*;
use modforge::actor::ActorId;

use crate::actor::{Actor, Dead, Home, Npc, Persona, Store};
use crate::content::{EpisodeRegistryRes, PLAYER_FACTION};

/// The joins decided this step: who, and the group they join. Made after
/// (`join`), one path for being taken in and for leaving.
#[derive(Resource, Default)]
pub struct Joins(pub Vec<(Entity, String)>);

/// Each join decided, made: their faction becomes the group's, and their
/// home and store a living member's of it (the lowest ActorId: the same
/// after a load). A group with no living member who has a home takes no
/// one in.
#[allow(clippy::type_complexity)]
pub fn join(mut commands: Commands, mut joins: ResMut<Joins>, mut people: Query<(Entity, &Actor, &mut Npc, Option<&Home>, Option<&Store>, Has<Dead>)>) {
    for (who, group) in std::mem::take(&mut joins.0) {
        let member = people
            .iter()
            .filter(|(e, _, npc, home, store, dead)| *e != who && !dead && npc.faction.as_deref() == Some(group.as_str()) && home.is_some() && store.is_some())
            .min_by_key(|(_, actor, ..)| actor.0.0)
            .and_then(|(_, _, _, home, store, _)| Some((*home?, *store?)));
        let Some((home, store)) = member else {
            continue;
        };
        let Ok((_, actor, mut npc, ..)) = people.get_mut(who) else {
            continue;
        };
        info!("{:?} joins {group}", actor.0);
        npc.faction = Some(group);
        commands.entity(who).insert((home, store));
    }
}

/// Once a game second: anyone in a group who feels better toward another
/// group than their own leaves for it. A group is felt as the average of
/// their feelings toward its members they remember (orchestrator,
/// 2026-10-02); one they remember no one of counts nothing. Only groups
/// with a home can take anyone in (`join`).
#[allow(clippy::type_complexity)]
pub fn leave_for_better(
    tick: Res<crate::SimTick>,
    episodes: Res<EpisodeRegistryRes>,
    mut joins: ResMut<Joins>,
    // A prisoner never leaves on their own (todo 11u).
    people: Query<(Entity, &Actor, Option<&Npc>, &Persona, Has<crate::player::Player>, Has<Home>), (Without<Dead>, Without<crate::prisoner::Prisoner>)>,
) {
    if tick.0 % 60 != 0 {
        return;
    }
    // Every living person's group: the player's bunker for the player.
    let group_of: std::collections::BTreeMap<ActorId, String> = people
        .iter()
        .filter_map(|(_, actor, npc, _, is_player, _)| {
            let group = if is_player { Some(PLAYER_FACTION.to_string()) } else { npc.and_then(|n| n.faction.clone()) };
            group.map(|g| (actor.0, g))
        })
        .collect();
    let homed: std::collections::BTreeSet<&str> =
        people.iter().filter(|(.., homed)| *homed).filter_map(|(_, _, npc, ..)| npc.and_then(|n| n.faction.as_deref())).collect();
    let order = crate::actor::in_actor_order(people.iter().map(|(entity, actor, ..)| (Some(actor.0), entity)));
    for entity in order {
        let Ok((entity, actor, Some(npc), memory, _, _)) = people.get(entity) else {
            continue;
        };
        let Some(own) = npc.faction.as_deref() else {
            continue;
        };
        // Their feeling toward each group, averaged over the members they
        // remember.
        let mut felt: std::collections::BTreeMap<&str, (f32, u32)> = Default::default();
        for (who, feeling) in modforge::relationship::feelings(&memory.0.memory, |line| episodes.felt(line)) {
            if who == actor.0 {
                continue;
            }
            if let Some(group) = group_of.get(&who) {
                let entry = felt.entry(group.as_str()).or_default();
                entry.0 += feeling;
                entry.1 += 1;
            }
        }
        let average = |group: &str| felt.get(group).map_or(0.0, |(sum, n)| sum / *n as f32);
        let mine = average(own);
        let better = felt
            .keys()
            .filter(|group| **group != own && homed.contains(**group))
            .map(|group| (*group, average(group)))
            .filter(|(_, feeling)| *feeling > mine)
            .max_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((group, _)) = better {
            joins.0.push((entity, group.to_string()));
        }
    }
}
