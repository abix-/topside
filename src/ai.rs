//! The AI's hand on an NPC's action queue (life.md "sense, decide,
//! act"). Every fixed step, before the queues drain, this builds each
//! NPC's `Perception`, hands it to modforge's brain, writes the states
//! back, and pushes the returned actions for this tick. From there an
//! NPC is driven exactly like the player: the same `drain_actions`,
//! `aim`, and `movement`. Nothing here decides; it senses and acts.
//! The 2D ground (x, y) is the brain's ground (x, z).

use bevy::prelude::*;
use modforge::actor::ActorId;
use modforge::brain::{Activity, Decision, Do, Perception, REACH, decide};
use modforge::memory::Known;
use modforge::monument::Roll;
use modforge::survival::Need;

use crate::SimTick;
use crate::actor::{
    Actor, ActorActivity, ActorCombat, ActorCondition, Dead, Facing, Home, Memory, Npc, Personality, Survival,
};
use crate::combat::Health;
use crate::content::{ActorRegistryRes, FactionRegistryRes, ItemRegistryRes, MonumentRegistryRes, PLAYER_FACTION};
use crate::input::Actions;
use crate::player::Player;

/// Within this of home counts as home.
const HOME_REACH: f32 = 6.0;

/// A point on the 2D ground as the brain's ground point.
fn ground(p: Vec2) -> Vec3 {
    Vec3::new(p.x, 0.0, p.y)
}

#[derive(bevy::ecs::system::SystemParam)]
pub struct Thinkers<'w, 's> {
    npcs: Query<
        'w,
        's,
        (
            Entity,
            &'static Actor,
            &'static Npc,
            &'static Transform,
            &'static Facing,
            &'static Health,
            &'static Survival,
            &'static Personality,
            Option<&'static Home>,
            &'static mut Memory,
            &'static mut ActorActivity,
            &'static mut ActorCombat,
            &'static mut ActorCondition,
            &'static mut Actions,
        ),
    >,
    others: Query<
        'w,
        's,
        (Entity, &'static Transform, Option<&'static Npc>, Option<&'static Actor>),
        (Or<(With<Npc>, With<Player>)>, Without<Dead>),
    >,
}

/// Every fixed step: sense, decide, act, for every NPC.
pub fn think(
    tick: Res<SimTick>,
    registry: Res<ActorRegistryRes>,
    factions: Res<FactionRegistryRes>,
    items: Res<ItemRegistryRes>,
    monuments: Res<MonumentRegistryRes>,
    mut thinkers: Thinkers,
) {
    // What a remembered kind of thing is worth for a need: the
    // registries' answer. An item's kind is an item def; a place's kind
    // is a monument type.
    let worth = |known: &Known, need: Need| -> f32 {
        if let Some(def) = items.0.def(&known.kind) {
            return def.good_for.satisfies(need);
        }
        monuments
            .monuments
            .def(&known.kind)
            .map_or(0.0, |d| d.good_for.satisfies(need))
    };
    // Everyone's position and faction first, so the pass below holds
    // no second borrow. The player carries no id yet: zero names it.
    let everyone: Vec<(Entity, ActorId, Vec2, Option<String>)> = thinkers
        .others
        .iter()
        .map(|(e, t, npc, actor)| {
            let faction = match npc {
                Some(n) => n.faction.clone(),
                None => Some(PLAYER_FACTION.to_string()),
            };
            (e, actor.map_or(ActorId(0), |a| a.0), t.translation.truncate(), faction)
        })
        .collect();
    for (
        entity,
        actor,
        npc,
        transform,
        facing,
        health,
        needs,
        personality,
        home,
        mut memory,
        mut activity,
        mut combat,
        mut condition,
        mut actions,
    ) in &mut thinkers.npcs
    {
        let Some(def) = registry.0.def(&npc.def) else {
            continue;
        };
        let here = transform.translation.truncate();
        // Sense: the nearest hostile by the faction table within the
        // def's sight (seeing through walls until sight like Zomboid's).
        let hostile = everyone
            .iter()
            .filter(|(e, _, at, faction)| {
                *e != entity
                    && at.distance(here) <= def.sight
                    && factions.0.hostile(npc.faction.as_deref(), faction.as_deref())
            })
            .min_by(|a, b| a.2.distance(here).total_cmp(&b.2.distance(here)))
            .map(|(_, id, at, _)| (*id, ground(*at)));
        let home_at = home.map(|h| ground(h.0));
        let at_home = home.is_some_and(|h| h.0.distance(here) <= HOME_REACH);
        let target = match &activity.0 {
            Activity::Going { to, .. } | Activity::Looking { to, .. } | Activity::Wander { to } => Some(*to),
            Activity::GoHome => home_at,
            _ => None,
        };
        let arrived = target.is_some_and(|t| Vec2::new(t.x, t.z).distance(here) <= REACH);
        condition.0.threat_in_sight = hostile.is_some();
        condition.0.at_home = at_home;
        if let Some((who, at)) = hostile {
            memory.0.threat(who, at, tick.0);
        }
        memory.0.forget_old(tick.0);
        let perception = Perception {
            now: tick.0,
            position: ground(here),
            yaw: facing.0,
            health_fraction: health.0.fraction(),
            needs: needs.0,
            home: home_at,
            behaviour: def.behaviour,
            at_home,
            asleep: condition.0.asleep,
            hostile,
            arrived,
            memory: &memory.0,
            personality: &personality.0,
            worth: &worth,
        };

        // Decide: seeded by the tick and the person, so a replay
        // decides the same.
        let mut roll = Roll::new(tick.0 ^ actor.0.0.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let Decision {
            activity: next_activity,
            combat: next_combat,
            actions: decided,
            do_now,
        } = decide(&perception, &activity.0, &combat.0, &mut roll);

        // Act.
        activity.0 = next_activity;
        combat.0 = next_combat;
        for action in decided {
            actions.0.push(tick.0, action);
        }
        match do_now {
            None => {}
            Some(Do::Sleep) => condition.0.asleep = true,
            Some(Do::Wake) => condition.0.asleep = false,
            // Boxes to look in or eat from arrive with loot (todo row
            // 11); until then the thing is gone from what it knows.
            Some(Do::Check { key } | Do::Eat { key }) => memory.0.gone(key),
        }
    }
}
