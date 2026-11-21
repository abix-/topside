//! Recruiting (todo 11u; design.md "Bunker people and topside people":
//! "like RimWorld. Capture someone, jail them, subdue them, force them to
//! work"). RimWorld's way: beaten down to little health, a person is
//! downed and cannot act; the one who beat them uses them (E, as for
//! anything) and they are taken prisoner, walking to their captor's home
//! and staying there, jailed, fighting no one; after a while jailed they
//! work for the bunker that took them, its group, home and store theirs
//! (joining::join), and never leave it on their own. Everyone is equal:
//! anyone can be downed, the player too.

use bevy::prelude::*;

use crate::actor::{Actor, Asked, Dead, Home, Npc};
use crate::combat::Health;
use crate::input::ThisTick;
use modforge::actions::Action;

/// At or under this share of their health a person is downed (Claude's
/// number, for the operator to change).
pub const SUBDUED: f32 = 0.25;
/// How long a downed person lies before getting up, if nobody takes them:
/// 30 game seconds (Claude's number).
const DOWNED_TICKS: u64 = 30 * 60;
/// How long a prisoner is jailed before they work for the bunker: a game
/// hour, in game seconds (Claude's number).
const JAILED_SECS: f32 = 3600.0;
/// How near their jail a prisoner counts as there, metres.
const AT_JAIL: f32 = 2.0;

/// Downed: lying, acting not at all, until `until` (a tick).
#[derive(Component, Clone, Copy, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Downed {
    pub until: u64,
}

/// The health share they were last downed at: they are downed again only
/// beaten lower, or once they have been above `SUBDUED` again.
#[derive(Component, Clone, Copy, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct LastDowned(pub f32);

/// Taken prisoner: by which group, jailed where, since when, and whether
/// they work for it yet.
#[derive(Component, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Prisoner {
    pub of: String,
    pub jail: Vec3,
    pub since: u64,
    pub working: bool,
}

/// Every step, after the walk and before anything is used or struck: a
/// person beaten to `SUBDUED` or under is downed and does nothing; one
/// downed long enough, and not taken, gets up.
pub fn down(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    mut people: Query<(Entity, &Health, &mut ThisTick, Option<&Downed>, Option<&LastDowned>, Has<Prisoner>), Without<Dead>>,
) {
    for (entity, health, mut this_tick, downed, last, prisoner) in &mut people {
        let share = health.0.fraction();
        if share > SUBDUED && last.is_some() {
            commands.entity(entity).remove::<LastDowned>();
        }
        if prisoner {
            if downed.is_some() {
                commands.entity(entity).remove::<Downed>();
            }
            continue;
        }
        match downed {
            Some(d) if tick.0 >= d.until => {
                commands.entity(entity).remove::<Downed>();
            }
            Some(_) => this_tick.0.clear(),
            None if share <= SUBDUED && last.is_none_or(|l| share < l.0) => {
                commands.entity(entity).insert((Downed { until: tick.0 + DOWNED_TICKS }, LastDowned(share)));
                this_tick.0.clear();
            }
            None => {}
        }
    }
}

/// Use (E) on a downed person of another group, looked at: they are taken
/// prisoner by the user's group, and walk to the user's home, jailed there
/// (the brain's "do what was asked"). The Use is spent on it, so it opens
/// no talk.
#[allow(clippy::type_complexity)]
pub fn capture(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    world: Res<crate::terrain::WorldRes>,
    mut captors: Query<(&mut ThisTick, &crate::talk::LookingAt, &Transform, Option<&Npc>, Option<&Home>, Has<crate::player::Player>, Option<&mut crate::actor::Persona>), (Without<Dead>, Without<Downed>)>,
    downed: Query<(Option<&Npc>, Has<crate::player::Player>, Option<&Name>), With<Downed>>,
) {
    for (mut this_tick, looking, at, npc, home, is_player, memory) in &mut captors {
        if !this_tick.0.iter().any(|s| s.action == Action::Use) {
            continue;
        }
        let Some((them, _)) = looking.0 else {
            continue;
        };
        let Ok((their_npc, their_player, name)) = downed.get(them) else {
            continue;
        };
        let group = |player: bool, npc: Option<&Npc>| if player { Some(crate::content::PLAYER_FACTION.to_string()) } else { npc.and_then(|n| n.faction.clone()) };
        let Some(ours) = group(is_player, npc) else {
            continue;
        };
        if group(their_player, their_npc).as_deref() == Some(ours.as_str()) {
            continue;
        }
        let jail = match home {
            Some(home) => home.0,
            None if is_player => Vec3::new(world.bunker.x, 0.0, world.bunker.y),
            None => at.translation,
        };
        commands.entity(them).remove::<Downed>().insert((Prisoner { of: ours, jail, since: tick.0, working: false }, Asked(jail)));
        this_tick.0.retain(|s| s.action != Action::Use);
        if let Some(mut memory) = memory {
            let who = name.map_or("someone", |n| n.as_str());
            memory.0.memory.did(modforge::memory::Did::Done(format!("took {who} prisoner")), tick.0);
        }
    }
}

/// Once a game second: a prisoner jailed `JAILED_SECS` works for the group
/// that took them, joining it (joining::join), no longer held at the jail.
pub fn put_to_work(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    mut joins: ResMut<crate::joining::Joins>,
    mut prisoners: Query<(Entity, &Actor, &Transform, &mut Prisoner), Without<Dead>>,
) {
    if tick.0 % 60 != 0 {
        return;
    }
    let order = crate::actor::in_actor_order(prisoners.iter().map(|(entity, actor, ..)| (Some(actor.0), entity)));
    for entity in order {
        let Ok((entity, _, at, mut prisoner)) = prisoners.get_mut(entity) else {
            continue;
        };
        let jailed = tick.0.saturating_sub(prisoner.since) as f32 / 60.0 >= JAILED_SECS;
        if prisoner.working || !jailed || at.translation.distance(prisoner.jail) > AT_JAIL {
            continue;
        }
        prisoner.working = true;
        joins.0.push((entity, prisoner.of.clone()));
        commands.entity(entity).remove::<Asked>();
    }
}
