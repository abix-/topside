//! Faction acts (todo 11j): once a game hour each faction with a home
//! votes on what to do (modforge acts: the franchise vote over its
//! living members at home, its genome leaned by each one's personality).
//! A raid carried sends the members for it out of their bunker to the
//! player's bunker, to fight the player there (the same commands anyone
//! is given: move to, then attack). A faction raids at most once a game
//! day. The other acts are voted and said, not yet done. Numbers
//! Claude's, for the operator to change.

use std::collections::{BTreeMap, VecDeque};

use bevy::prelude::*;
use modforge::acts::{Act, vote};
use modforge::command::Command;

use crate::actor::{Actor, Dead, Home, Npc, Persona};
use crate::command::Commanded;
use crate::content::PLAYER_FACTION;

/// A band is at least this many (Claude's number).
const BAND_AT_LEAST: usize = 2;

/// Each faction's last raid, the tick it left (saved).
#[derive(Resource, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct Raids(pub BTreeMap<String, u64>);

#[allow(clippy::type_complexity)]
pub fn vote_and_raid(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    day: Res<crate::survival::Day>,
    world: Res<crate::terrain::WorldRes>,
    mut raids: ResMut<Raids>,
    player: Query<&Actor, (With<crate::player::Player>, Without<Dead>)>,
    people: Query<(Entity, &Actor, &Npc, &Persona, &Home, &Transform, Has<Commanded>), (Without<Dead>, Without<crate::prisoner::Prisoner>)>,
) {
    let day_ticks = (day.0 * 60.0).round().max(1.0) as u64;
    let hour = (day_ticks / 24).max(1);
    if tick.0 == 0 || tick.0 % hour != 0 {
        return;
    }
    let Ok(target) = player.single() else {
        return;
    };
    // Each faction's members at home, in ActorId order.
    let mut factions: BTreeMap<String, Vec<(i64, Entity, modforge::actor::Personality)>> = BTreeMap::new();
    for (entity, actor, npc, personality, home, at, commanded) in &people {
        let Some(faction) = npc.faction.as_deref().filter(|f| *f != PLAYER_FACTION) else {
            continue;
        };
        if commanded || at.translation.distance(home.0) > 6.0 {
            continue;
        }
        factions.entry(faction.to_string()).or_default().push((actor.0.0 as i64, entity, personality.0.personality));
    }
    let bunker = Vec3::new(world.bunker.x, 0.0, world.bunker.y);
    for (faction, mut members) in factions {
        members.sort_by_key(|(id, ..)| *id);
        let voters: Vec<(i64, modforge::actor::Personality)> = members.iter().map(|(id, _, p)| (*id, *p)).collect();
        let Some((act, ballot)) = vote(&faction, &voters) else {
            continue;
        };
        info!("{faction} votes to {act:?}: {} of {}", ballot.votes_for, ballot.franchise);
        if act != Act::Raid || raids.0.get(&faction).is_some_and(|last| tick.0 < last + day_ticks) {
            continue;
        }
        let band = modforge::acts::for_it(&faction, Act::Raid, &voters);
        if band.len() < BAND_AT_LEAST {
            continue;
        }
        for (id, entity, _) in members.iter().filter(|(id, ..)| band.contains(id)) {
            let left: VecDeque<Command> = [Command::MoveTo { to: bunker.into() }, Command::Attack { who: target.0.0 }].into();
            commands.entity(*entity).insert(Commanded { left, started: false });
            info!("{id} leaves {faction}'s bunker to raid");
        }
        raids.0.insert(faction, tick.0);
    }
}
