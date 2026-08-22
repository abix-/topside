//! The AI's hand on an NPC's action queue (life.md "sense, decide,
//! act"). Every fixed step, before the queues drain, this builds
//! each NPC's `Perception` from the world, hands it to modforge's
//! brain with the NPC's own `Activity` and `CombatState`, writes the
//! two states back, pushes the returned actions stamped for this
//! tick, and carries out the one `Do` with the world's data (take
//! food from that box, sleep). From there an NPC is driven exactly
//! like the player: the same `drain_actions`, `look`, `movement`,
//! `fire`. Nothing here decides; it senses and acts.

use bevy::prelude::*;
use modforge::brain::{Activity, CombatState, Decision, Do, Perception, REACH, decide};
use modforge::monument::Roll;

use crate::SimTick;
use crate::actor::{Facing, Home, Inventory, Memory, Npc, Personality};
use crate::actor_map::Actor;
use crate::combat::Health;
use crate::content::{ActorRegistryRes, FactionRegistryRes, ItemRegistryRes, PLAYER_FACTION};
use crate::input::Actions;
use crate::interact::Container;
use crate::player::Player;
use crate::survival::{ActorCondition, Survival};

/// What an NPC is doing, kept between thinks (modforge::brain).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct ActorActivity(pub Activity);

impl Default for ActorActivity {
    fn default() -> Self {
        Self(Activity::Idle)
    }
}

/// Whether an NPC is fighting, kept between thinks.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ActorCombat(pub CombatState);

impl Default for ActorCombat {
    fn default() -> Self {
        Self(CombatState::None)
    }
}

/// Within this of home counts as home.
const HOME_REACH: f32 = 6.0;

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
            &'static mut Survival,
            &'static Personality,
            Option<&'static Home>,
            &'static mut Memory,
            &'static mut ActorActivity,
            &'static mut ActorCombat,
            &'static mut ActorCondition,
            &'static mut Actions,
            &'static mut Inventory,
        ),
    >,
    others: Query<
        'w,
        's,
        (Entity, &'static Transform, Option<&'static Npc>, Option<&'static Actor>),
        Or<(With<Npc>, With<Player>)>,
    >,
    containers: Query<'w, 's, &'static mut Container>,
}

/// Every fixed step: sense, decide, act, for every NPC.
pub fn think(
    tick: Res<SimTick>,
    registry: Res<ActorRegistryRes>,
    factions: Res<FactionRegistryRes>,
    items: Res<ItemRegistryRes>,
    mut thinkers: Thinkers,
) {
    // Everyone's position and faction first, so the pass below holds
    // no second borrow.
    let actors: Vec<(Entity, modforge::actor::ActorId, Vec3, Option<String>)> = thinkers
        .others
        .iter()
        .map(|(e, t, npc, actor)| {
            let faction = match npc {
                Some(n) => n.faction.clone(),
                None => Some(PLAYER_FACTION.to_string()),
            };
            // The player carries no id yet: zero names it.
            let id = actor.map_or(modforge::actor::ActorId(0), |a| a.0);
            (e, id, t.translation, faction)
        })
        .collect();
    for (
        entity,
        actor,
        npc,
        transform,
        facing,
        health,
        mut needs,
        personality,
        home,
        mut memory,
        mut activity,
        mut combat,
        mut condition,
        mut actions,
        mut inventory,
    ) in &mut thinkers.npcs
    {
        let Some(def) = registry.0.def(&npc.def) else {
            continue;
        };
        let here = transform.translation;
        // Sense. No line of sight until 11f: the nearest hostile by
        // the faction table within the def's sight.
        let hostile = actors
            .iter()
            .filter(|(e, _, at, faction)| {
                *e != entity
                    && at.distance(here) <= def.sight
                    && factions.0.hostile(npc.faction.as_deref(), faction.as_deref())
            })
            .min_by(|a, b| a.2.distance(here).total_cmp(&b.2.distance(here)))
            .map(|(_, id, at, _)| (*id, *at));
        let home_position = home.map(|h| h.position);
        let at_home = home_position.is_some_and(|h| h.distance(here.with_y(h.y)) <= HOME_REACH);
        let target = match &activity.0 {
            Activity::Going { to, .. } | Activity::Looking { to, .. } | Activity::Wander { to } => {
                Some(*to)
            }
            Activity::GoHome => home_position,
            _ => None,
        };
        let arrived = target.is_some_and(|t| (t - here).with_y(0.0).length() <= REACH);
        condition.0.threat_in_sight = hostile.is_some();
        condition.0.at_home = at_home;
        if let Some((who, at)) = hostile {
            memory.0.threat(who, at, tick.0);
        }
        memory.0.forget_old(tick.0);
        let perception = Perception {
            now: tick.0,
            position: here,
            yaw: facing.yaw,
            health_fraction: health.0.fraction(),
            needs: needs.0,
            home: home_position,
            behaviour: def.behaviour,
            at_home,
            asleep: condition.0.asleep,
            hostile,
            arrived,
            memory: &memory.0,
            personality: &personality.0,
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
            Some(Do::Check { key }) => {
                let had = thinkers
                    .containers
                    .get(Entity::from_bits(key))
                    .map(|c| c.0.slots.iter().any(|s| s.is_some()));
                match had {
                    Ok(had) => memory.0.checked(key, had, tick.0),
                    Err(_) => memory.0.gone(key),
                }
            }
            Some(Do::Eat { key }) => {
                // Take one food item from the box into the inventory,
                // then eat it through the one eating path the player
                // uses (`eat_from_slot`).
                let Ok(mut container) = thinkers.containers.get_mut(Entity::from_bits(key)) else {
                    memory.0.gone(key);
                    continue;
                };
                let food_slot = container.0.slots.iter().position(|s| {
                    s.as_ref()
                        .and_then(|s| items.0.def(&s.item))
                        .is_some_and(|d| d.food.is_some())
                });
                let Some(slot) = food_slot else {
                    memory.0.checked(key, false, tick.0);
                    continue;
                };
                let Some(taken) = container.0.remove(slot, 1) else {
                    continue;
                };
                let name = taken.item.clone();
                let max = items.0.def(&name).map_or(1, |d| d.max_stack);
                if let Some(rest) = inventory.0.add(taken, max) {
                    // No room: put it back.
                    container.0.add(rest, max);
                    continue;
                }
                if let Some(mine) = inventory
                    .0
                    .slots
                    .iter()
                    .position(|s| s.as_ref().is_some_and(|s| s.item == name))
                {
                    let _ = needs.0.eat_from_slot(&mut inventory.0, mine, &items.0);
                }
                let left = container.0.slots.iter().any(|s| {
                    s.as_ref()
                        .and_then(|s| items.0.def(&s.item))
                        .is_some_and(|d| d.food.is_some())
                });
                memory.0.checked(key, left, tick.0);
            }
        }
    }
}
