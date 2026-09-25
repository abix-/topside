//! The AI's hand on an NPC's action queue (life.md "sense, decide,
//! act"). Every fixed step, before the queues drain, this builds each
//! NPC's `Perception`, hands it to modforge's brain, writes the states
//! back, and pushes the returned actions for this tick. From there an
//! NPC is driven exactly like the player: the same `drain_actions`,
//! `aim`, and `movement`. Nothing here decides; it senses and acts.
//! The 2D ground (x, y) is the brain's ground (x, z).

use bevy::prelude::*;
use modforge::actor::ActorId;
use modforge::brain::{Activity, CombatState, Decision, Do, Perception, REACH, decide, reaction_ticks};
use modforge::hud::{Holder, SlotRef};
use modforge::item::ItemRegistry;
use modforge::memory::{Known, food_worth};
use modforge::monument::Roll;
use modforge::survival::Need;

use crate::SimTick;
use crate::actor::{
    Actor, ActorActivity, ActorCombat, ActorCondition, Dead, Facing, Home, Hotbar, Inventory, Memory, NextThink,
    Npc, PLACE_KEY, Personality, Survival,
};
use crate::combat::Health;
use crate::content::{ActorRegistryRes, FactionRegistryRes, ItemRegistryRes, MonumentRegistryRes, PLAYER_FACTION};
use crate::input::{Action, Actions};
use crate::interact::{Container, OpenContainer, WorldItem};
use crate::player::Player;
use crate::sight::Sight;

/// What the people have done since the world began: proof they live,
/// read by the control plane and the scale test.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct LifeStats {
    pub ate: u32,
    pub looked: u32,
    pub slept: u32,
    pub fights: u32,
    pub died: u32,
}

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
            (
                &'static mut Actions,
                &'static Inventory,
                &'static Hotbar,
                &'static OpenContainer,
                &'static mut NextThink,
                Has<Dead>,
            ),
        ),
    >,
    others: Query<
        'w,
        's,
        (Entity, &'static Transform, Option<&'static Npc>, Option<&'static Actor>),
        (Or<(With<Npc>, With<Player>)>, Without<Dead>),
    >,
}

/// Half the width of what a person sees ahead: a 120 degree cone
/// (life.md "Perception with belief").
const HALF_CONE: f32 = std::f32::consts::FRAC_PI_3;

/// Inside the cone ahead of someone facing `facing` at `here`.
fn in_cone(facing: &Facing, here: Vec2, at: Vec2) -> bool {
    let to = at - here;
    to.length() < 1e-3 || facing.direction().angle_to(to).abs() <= HALF_CONE
}

/// The kinds of stacks in a box: what a person remembers after looking
/// inside (life.md: memory stores what it saw).
fn kinds_in(inventory: &modforge::item::Inventory) -> Vec<String> {
    inventory.slots.iter().flatten().map(|s| s.item.clone()).collect()
}

/// Every fixed step: sense, decide, act, for every NPC.
pub fn think(
    tick: Res<SimTick>,
    registry: Res<ActorRegistryRes>,
    factions: Res<FactionRegistryRes>,
    items: Res<ItemRegistryRes>,
    monuments: Res<MonumentRegistryRes>,
    sight: Sight,
    mut life: ResMut<LifeStats>,
    mut thinkers: Thinkers,
    containers: Query<(Entity, &Container, &Transform, &WorldItem)>,
) {
    // What a remembered thing is worth for a need, the RimWorld way: for
    // hunger and thirst, the food seen inside it (modforge
    // `food_worth`); for rest and safety, what its place's type offers.
    let worth = |known: &Known, need: Need| -> f32 {
        match need {
            Need::Hunger | Need::Thirst => food_worth(known, need, &items.0),
            Need::Rest | Need::Safety => monuments
                .monuments
                .def(&known.kind)
                .map_or(0.0, |d| d.good_for.satisfies(need)),
        }
    };
    // Every box once: where, what kind, for looking round.
    let boxes: Vec<(Entity, Vec2, String)> = containers
        .iter()
        .map(|(e, _, t, w)| (e, t.translation.truncate(), w.stack.item.clone()))
        .collect();
    // The storm is the same for everyone: coming or here, hide.
    let storm_coming = crate::storm::phase(&tick) != modforge::storm::StormPhase::Calm;
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
        (mut actions, inventory, hotbar, open, mut next_think, dead),
    ) in &mut thinkers.npcs
    {
        // Not their moment yet: they keep doing what they decided.
        if tick.0 < next_think.0 {
            continue;
        }
        // Dead with a home to wake in: at their moment they respawn, the
        // same Respawn the player's Enter pushes.
        if dead {
            actions.0.push(tick.0, Action::Respawn);
            next_think.0 = tick.0 + (modforge::brain::AWAKE_REACTION * 60.0) as u64;
            continue;
        }
        let Some(def) = registry.0.def(&npc.def) else {
            continue;
        };
        let here = transform.translation.truncate();
        // Sense: the nearest hostile by the faction table within the
        // def's sight, inside the cone ahead, with nothing blocking the
        // view (sight.rs).
        let hostile = everyone
            .iter()
            .filter(|(e, _, at, faction)| {
                *e != entity
                    && at.distance(here) <= def.sight
                    && factions.0.hostile(npc.faction.as_deref(), faction.as_deref())
                    && in_cone(facing, here, *at)
                    && sight.clear(here, *at)
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
        // Looking round as they think: every box in the cone with a clear
        // line goes into memory (seen, not yet looked inside).
        for (key, at, kind) in &boxes {
            if at.distance(here) <= def.sight && in_cone(facing, here, *at) && sight.clear(here, *at) {
                memory.0.see(key.to_bits(), kind, ground(*at), tick.0);
            }
        }
        // A box they have open, they see into, as the player sees the
        // open box's panel.
        if let Some((key, container)) = open.0.and_then(|e| containers.get(e).ok().map(|(_, c, _, _)| (e, c))) {
            memory.0.checked(key.to_bits(), kinds_in(&container.0), tick.0);
            life.looked += 1;
        }
        memory.0.forget_old(tick.0);
        let carried = carried_slot(inventory, hotbar, &items.0, Need::Hunger);
        let carries_drink = carried_slot(inventory, hotbar, &items.0, Need::Thirst).is_some();
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
            storm_coming,
            carries_food: carried.is_some(),
            carries_drink,
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
        if matches!(next_combat, CombatState::Fighting { .. }) && !matches!(combat.0, CombatState::Fighting { .. }) {
            life.fights += 1;
        }
        activity.0 = next_activity;
        combat.0 = next_combat;
        // Their next decision comes at their reaction time.
        next_think.0 = tick.0
            + reaction_ticks(condition.0.asleep, &combat.0, needs.0.rest, &personality.0, 60.0);
        for action in decided {
            actions.0.push(tick.0, action);
        }
        // What the brain wants done is done with the player's own actions:
        // Use opens a box, Transfer takes a stack out of the open box,
        // UseSlot eats or drinks, Cancel closes the box.
        let mut act = |action: Action| actions.0.push(tick.0, action);
        let the_box = |key: u64| Entity::try_from_bits(key).filter(|e| containers.contains(*e));
        let mut keep_open = None;
        match do_now {
            None => {}
            Some(Do::Sleep) => {
                condition.0.asleep = true;
                life.slept += 1;
            }
            Some(Do::Wake) => condition.0.asleep = false,
            // A key is a place (PLACE_KEY set) or a box (an entity): a
            // place is visited; a box is opened to see inside (noted
            // above at the next think); a box that is gone is forgotten.
            Some(Do::Check { key }) if key & PLACE_KEY != 0 => memory.0.visited(key, tick.0),
            Some(Do::Check { key }) => match the_box(key) {
                Some(found) => {
                    keep_open = Some(found);
                    open_box(&mut act, open, found);
                }
                None => memory.0.gone(key),
            },
            Some(Do::EatCarried { need }) => {
                if let Some(slot) = carried_slot(inventory, hotbar, &items.0, need) {
                    act(Action::UseSlot { slot });
                    life.ate += 1;
                }
            }
            // At the box: eat what they carry, else take food out of the
            // open box, else open it.
            Some(Do::Eat { key }) => {
                let Some(found) = the_box(key) else {
                    memory.0.gone(key);
                    continue;
                };
                keep_open = Some(found);
                let need = if needs.0.hunger <= needs.0.thirst { Need::Hunger } else { Need::Thirst };
                let other = if need == Need::Hunger { Need::Thirst } else { Need::Hunger };
                if let Some(slot) = carried_slot(inventory, hotbar, &items.0, need)
                    .or_else(|| carried_slot(inventory, hotbar, &items.0, other))
                {
                    act(Action::UseSlot { slot });
                    life.ate += 1;
                } else if open.0 == Some(found) {
                    let (_, container, _, _) = containers.get(found).expect("checked above");
                    let food = [need, other].into_iter().find_map(|n| {
                        container.0.slots.iter().position(|s| s.as_ref().is_some_and(|s| answers(&items.0, &s.item, n)))
                    });
                    if let Some(index) = food {
                        let slot = SlotRef {
                            holder: Holder::Container,
                            index,
                        };
                        act(Action::Transfer { half: false, slot: Some(slot) });
                    }
                } else {
                    open_box(&mut act, open, found);
                }
            }
        }
        // Done with a box: close it, as the player closes the panel.
        if open.0.is_some() && open.0 != keep_open {
            act(Action::Cancel);
        }
    }
}

/// Open `found`: close whatever else is open, then use it.
fn open_box(act: &mut impl FnMut(Action), open: &OpenContainer, found: Entity) {
    if open.0 == Some(found) {
        return;
    }
    if open.0.is_some() {
        act(Action::Cancel);
    }
    act(Action::Use);
}

/// Whether `item` answers `need` when eaten or drunk.
fn answers(items: &ItemRegistry, item: &str, need: Need) -> bool {
    items.def(item).and_then(|d| d.food.as_ref()).is_some_and(|f| match need {
        Need::Hunger => f.hunger > 0.0,
        Need::Thirst => f.thirst > 0.0,
        Need::Rest | Need::Safety => false,
    })
}

/// The first slot in a person's bags (hotbar, then inventory) holding
/// something that answers `need`.
fn carried_slot(inventory: &Inventory, hotbar: &Hotbar, items: &ItemRegistry, need: Need) -> Option<SlotRef> {
    let find = |slots: &modforge::item::Inventory, holder| {
        slots
            .slots
            .iter()
            .position(|s| s.as_ref().is_some_and(|s| answers(items, &s.item, need)))
            .map(|index| SlotRef { holder, index })
    };
    find(&hotbar.0.slots, Holder::Hotbar).or_else(|| find(&inventory.0, Holder::Inventory))
}
