//! The AI's hand on an NPC's action queue (life.md "sense, decide,
//! act"). Every fixed step, before the queues drain, this builds each
//! NPC's `Perception`, hands it to modforge's brain, writes the states
//! back, and pushes the returned actions for this tick. From there an
//! NPC is driven exactly like the player: the same `drain_actions`,
//! `aim`, and `movement`. Nothing here decides; it senses and acts.
//! The 2D ground (x, y) is the brain's ground (x, z).

use bevy::prelude::*;
use modforge::actor::ActorId;
use modforge::brain::{Do, NEED_LINE, Perception, Think, reaction_ticks, situation};
use modforge::learn::alive_for;
use modforge::hud::{Holder, SlotRef};
use modforge::item::ItemRegistry;
use modforge::memory::{Did, Known, food_worth, short_of};
use modforge::monument::Roll;
use modforge::survival::Need;

use crate::{Ground, SimTick};
use crate::actor::{
    Actor, ActorCondition, ActorRecord, Dead, Facing, Home, Hotbar, Inventory, Memory, NextThink,
    Npc, PLACE_KEY, Personality, Store, Survival, held_in,
};
use crate::combat::Health;
use crate::content::{
    ActorRegistryRes, BUNKER_PEOPLE, BUNKER_SHORT_DAYS, FactionRegistryRes, ItemRegistryRes,
    MonumentRegistryRes, PLAYER_FACTION,
};
use crate::input::{Action, Actions};
use crate::interact::{Container, OpenContainer, WorldItem};
use crate::player::Player;
use crate::sight::Sight;

/// Within this of home counts as home.
pub(crate) const HOME_REACH: f32 = 6.0;

/// The second bit of a place key marks a river's water, one per chunk:
/// the shore last seen there.
const WATER_KEY: u64 = 1 << 62;

/// The remembered key of the water in chunk `c`.
fn water_key(c: modforge::walk::ChunkKey) -> u64 {
    let pack = |v: i32| (v as i64 + (1 << 20)) as u64 & 0x1F_FFFF;
    PLACE_KEY | WATER_KEY | (pack(c.0) << 21) | pack(c.1)
}

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
            &'static mut ActorRecord,
            &'static mut ActorCondition,
            (
                &'static mut Actions,
                &'static Inventory,
                &'static Hotbar,
                &'static OpenContainer,
                &'static mut NextThink,
                Has<Dead>,
                Option<&'static Store>,
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

/// Every fixed step: sense, decide, act, for every NPC.
pub fn think(
    tick: Res<SimTick>,
    registry: Res<ActorRegistryRes>,
    factions: Res<FactionRegistryRes>,
    items: Res<ItemRegistryRes>,
    monuments: Res<MonumentRegistryRes>,
    tree: Res<crate::content::brain::BrainTreeRes>,
    day: Res<crate::survival::Day>,
    mut walk: ResMut<crate::nav::Walk>,
    sight: Sight,
    mut thinkers: Thinkers,
    containers: Query<(Entity, &Container, &Transform, &WorldItem)>,
    lying: Query<(Entity, &Transform, &WorldItem), Without<Container>>,
    chunks: Option<Res<crate::terrain::Chunks>>,
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
        .map(|(e, _, t, w)| (e, t.translation.ground(), w.stack.item.clone()))
        .collect();
    // Every source once (a well, a berry bush: an item of kind `Source`):
    // where, what kind.
    let sources_at: Vec<(Entity, Vec2, String)> = lying
        .iter()
        .filter(|(_, _, w)| items.0.def(&w.stack.item).is_some_and(|d| d.kind == modforge::item::ItemKind::Source))
        .map(|(e, t, w)| (e, t.translation.ground(), w.stack.item.clone()))
        .collect();
    let source_at = |key: u64| {
        sources_at
            .iter()
            .find(|(e, _, _)| e.to_bits() == key)
            .map(|(_, at, _)| *at)
    };
    // Where someone can go from `here`: a tile they can stand on
    // (pathing.md: never head for a place nobody can stand on) that the
    // pathing finds a way to (life.md "Wander: random, reachable").
    let walk = std::cell::RefCell::new(&mut walk.0);
    let can_go = |here: Vec2, at: Vec3| {
        let to = Vec2::new(at.x, at.z);
        let mut walk = walk.borrow_mut();
        walk.standable(to) && walk.find_way(here, to, crate::nav::PLAN_LIMIT).is_some()
    };
    // The storm is the same for everyone: coming or here, hide.
    let storm_coming = crate::storm::phase(&tick, *day) != modforge::storm::StormPhase::Calm;
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
            (e, actor.map_or(ActorId(0), |a| a.0), t.translation.ground(), faction)
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
        mut record,
        mut condition,
        (mut actions, inventory, hotbar, open, mut next_think, dead, store),
    ) in &mut thinkers.npcs
    {
        // Not their moment yet: they keep doing what they decided.
        if tick.0 < next_think.0 {
            continue;
        }
        // Dead with a home to wake in: at their moment they respawn, the
        // same Respawn the player's Enter pushes. Dead with none (a topside
        // person): never back.
        if dead {
            if home.is_some() {
                actions.0.push(tick.0, Action::Respawn);
                next_think.0 = tick.0 + (modforge::brain::AWAKE_REACTION * 60.0) as u64;
            }
            continue;
        }
        let Some(def) = registry.0.def(&npc.def) else {
            continue;
        };
        let here = transform.translation.ground();
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
        condition.0.threat_in_sight = hostile.is_some();
        condition.0.at_home = at_home;
        if let Some((who, at)) = hostile {
            memory.0.threat(who, at, tick.0);
        }
        // Looking round as they think: every box in the cone with a clear
        // line goes into memory (seen, not yet looked inside).
        for (key, at, kind) in boxes.iter().chain(&sources_at) {
            if at.distance(here) <= def.sight && in_cone(facing, here, *at) && sight.clear(here, *at) {
                memory.0.see(key.to_bits(), kind, ground(*at), tick.0);
            }
        }
        // A river seen: the nearest shore in sight, one memory per chunk,
        // where they can stand and drink (design.md: rivers drinkable).
        if let Some(chunks) = chunks.as_ref() {
            let c = crate::terrain::chunk_of(here);
            let seen = (c.1 - 1..=c.1 + 1)
                .flat_map(|y| (c.0 - 1..=c.0 + 1).map(move |x| (x, y)))
                .filter_map(|k| chunks.shores.get(&k))
                .flatten()
                .map(|s| modforge::walk::centre(s.land))
                .filter(|p| p.distance(here) <= def.sight && in_cone(facing, here, *p))
                .min_by(|a, b| a.distance(here).total_cmp(&b.distance(here)));
            if let Some(shore) = seen.filter(|p| sight.clear(here, *p)) {
                let key = water_key(crate::terrain::chunk_of(shore));
                memory.0.see(key, crate::interact::WATER, ground(shore), tick.0);
            }
        }
        // A box they have open, they see into, as the player sees the
        // open box's panel.
        if let Some((key, container)) = open.0.and_then(|e| containers.get(e).ok().map(|(_, c, _, _)| (e, c))) {
            memory.0.checked(key.to_bits(), held_in(&container.0), tick.0);
        }
        memory.0.forget_old(tick.0);
        let carried = carried_slot(inventory, hotbar, &items.0, Need::Hunger);
        let carries_drink = carried_slot(inventory, hotbar, &items.0, Need::Thirst).is_some();
        // Their bunker: the store where they last saw it, and what the
        // bunker is short of by what they remember in it plus what they
        // carry home (design.md "Taking loot").
        let store_at = store.and_then(|s| {
            let key = s.0.to_bits();
            memory.0.known.iter().find(|k| k.key == key).map(|k| (key, k.position))
        });
        let bags = held_in(&inventory.0);
        let carries_for_bunker = bags.iter().any(|(kind, _)| {
            answers(&items.0, kind, Need::Hunger) || answers(&items.0, kind, Need::Thirst)
        });
        let bunker_short = store_at
            .map(|(key, _)| {
                let in_store = memory.0.known.iter().find(|k| k.key == key).and_then(|k| k.held.clone()).unwrap_or_default();
                let rates = day.rates();
                let per_day = (rates.hunger_per_sec * day.0, rates.thirst_per_sec * day.0);
                let supplies = in_store.iter().chain(&bags).map(|(k, n)| (k.as_str(), *n));
                short_of(supplies, BUNKER_PEOPLE as u32, BUNKER_SHORT_DAYS, per_day, &items.0)
            })
            .unwrap_or_default();
        let bags_full = inventory.0.slots.iter().all(|s| s.is_some());
        // How long they can stay alive from here (life.md "Learning to
        // stay alive"): the change since they last thought scores the
        // choices that led to it.
        let mut carrying = bags.clone();
        carrying.extend(held_in(&hotbar.0.slots));
        let alive = alive_for(
            &needs.0,
            day.rates(),
            health.0.fraction(),
            &carrying,
            store.map(|s| s.0.to_bits()),
            &memory.0,
            &items.0,
        );
        memory.0.learned.score(tick.0, alive);
        let standable = |at: Vec3| can_go(here, at);
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
            storm_coming,
            carries_food: carried.is_some(),
            carries_drink,
            store: store_at,
            bunker_short: bunker_short.clone(),
            carries_for_bunker,
            bags_full,
            memory: &memory.0,
            personality: &personality.0,
            worth: &worth,
            standable: &standable,
        };

        // Decide through the one tree: seeded by the tick and the person,
        // so a replay decides the same. Each learned choice made goes to
        // what they learn from.
        let mut roll = Roll::new(tick.0 ^ actor.0.0.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let situation = situation(&perception);
        let mut thought = Think::new(&perception, &mut roll);
        let chose = tree.0.think(&mut thought, &mut record.0);
        let Think {
            actions: decided,
            do_now,
            ..
        } = thought;
        for choice in chose {
            memory.0.learned.chose(situation, choice);
        }

        // Act. Their next decision comes at their reaction time.
        let fighting = crate::content::brain::fighting(&record.0);
        next_think.0 = tick.0 + reaction_ticks(condition.0.asleep, fighting, needs.0.rest, &personality.0, 60.0);
        for action in decided {
            actions.0.push(tick.0, action);
        }
        // What the brain wants done is done with the player's own actions:
        // Use opens a box, Transfer takes a stack out of the open box,
        // UseSlot eats or drinks, Cancel closes the box.
        let mut act = |action: Action| actions.0.push(tick.0, action);
        let the_box = |key: u64| Entity::try_from_bits(key).filter(|e| containers.contains(*e));
        let where_is = |e: Entity| containers.get(e).map_or(here, |(_, _, t, _)| t.translation.ground());
        let mut keep_open = None;
        match do_now {
            None => {}
            Some(Do::Sleep) => {
                condition.0.asleep = true;
                memory.0.did(Did::Slept, tick.0);
            }
            Some(Do::Wake) => condition.0.asleep = false,
            // A key is a place (PLACE_KEY set) or a box (an entity): a
            // place is visited; a box is opened to see inside (noted
            // above at the next think); a box that is gone is forgotten.
            // A place, a river's water, or a source (a well, a bush) is
            // visited: there is no inside to look into.
            Some(Do::Check { key }) if key & PLACE_KEY != 0 || source_at(key).is_some() => memory.0.visited(key, tick.0),
            Some(Do::Check { key }) => match the_box(key) {
                Some(found) => {
                    keep_open = Some(found);
                    open_box(&mut act, open, found, where_is(found));
                }
                None => memory.0.gone(key),
            },
            Some(Do::EatCarried { need }) => {
                if let Some(slot) = carried_slot(inventory, hotbar, &items.0, need) {
                    act(Action::UseSlot { slot });
                }
            }
            // At a river's shore: drink from the water in reach, a
            // mouthful a think, until the thirst is met; no water there,
            // forget it.
            Some(Do::Eat { key, .. }) if key & WATER_KEY != 0 => {
                match chunks.as_ref().and_then(|c| crate::interact::nearest_water(c, here, here)) {
                    Some(water) => {
                        act(Action::Aim { x: water.x, y: water.y });
                        act(Action::Use);
                    }
                    None => memory.0.gone(key),
                }
            }
            // At a source (a well, a berry bush): use it where it stands,
            // a mouthful a think, until the need is met.
            Some(Do::Eat { key, .. }) if source_at(key).is_some() => {
                let at = source_at(key).expect("checked above");
                act(Action::Aim { x: at.x, y: at.y });
                act(Action::Use);
            }
            // At the box: open it, then eat or drink one thing out of it
            // for the need that sent them, one a think, until it is met.
            // You eat what you need; nothing is carried off.
            Some(Do::Eat { key, need }) => {
                let Some(found) = the_box(key) else {
                    memory.0.gone(key);
                    continue;
                };
                keep_open = Some(found);
                if open.0 != Some(found) {
                    open_box(&mut act, open, found, where_is(found));
                    continue;
                }
                let (_, container, _, _) = containers.get(found).expect("checked above");
                let food = container.0.slots.iter().position(|s| s.as_ref().is_some_and(|s| answers(&items.0, &s.item, need)));
                if let Some(index) = food {
                    act(Action::UseSlot {
                        slot: SlotRef {
                            holder: Holder::Container,
                            index,
                        },
                    });
                }
            }
            // At a box topside: open it, then take every stack that
            // answers a need of theirs or their bunker's, as much as the
            // bags hold (design.md "Taking loot").
            Some(Do::Take { key }) => {
                let Some(found) = the_box(key) else {
                    memory.0.gone(key);
                    continue;
                };
                keep_open = Some(found);
                if open.0 != Some(found) {
                    open_box(&mut act, open, found, where_is(found));
                    continue;
                }
                let needed = |kind: &str| {
                    [Need::Hunger, Need::Thirst].into_iter().any(|n| {
                        let own = match n {
                            Need::Hunger => needs.0.hunger,
                            _ => needs.0.thirst,
                        };
                        (bunker_short.contains(&n) || own < NEED_LINE) && answers(&items.0, kind, n)
                    })
                };
                let (_, container, _, _) = containers.get(found).expect("checked above");
                for (index, stack) in container.0.slots.iter().enumerate() {
                    if stack.as_ref().is_some_and(|s| needed(&s.item)) {
                        let slot = SlotRef {
                            holder: Holder::Container,
                            index,
                        };
                        act(Action::Transfer { half: false, slot: Some(slot) });
                    }
                }
            }
            // At their store: open it, then put in every stack of food and
            // water they carry.
            Some(Do::Stock { key }) => {
                let Some(found) = the_box(key) else {
                    memory.0.gone(key);
                    continue;
                };
                keep_open = Some(found);
                if open.0 != Some(found) {
                    open_box(&mut act, open, found, where_is(found));
                    continue;
                }
                for (index, stack) in inventory.0.slots.iter().enumerate() {
                    if stack
                        .as_ref()
                        .is_some_and(|s| answers(&items.0, &s.item, Need::Hunger) || answers(&items.0, &s.item, Need::Thirst))
                    {
                        let slot = SlotRef {
                            holder: Holder::Inventory,
                            index,
                        };
                        act(Action::Transfer { half: false, slot: Some(slot) });
                    }
                }
            }
        }
        // Done with a box: close it, as the player closes the panel.
        if open.0.is_some() && open.0 != keep_open {
            act(Action::Cancel);
        }
    }
}

/// Open `found` standing at `at`: close whatever else is open, point at
/// it, then use it (the player's aim and E).
fn open_box(act: &mut impl FnMut(Action), open: &OpenContainer, found: Entity, at: Vec2) {
    if open.0 == Some(found) {
        return;
    }
    if open.0.is_some() {
        act(Action::Cancel);
    }
    act(Action::Aim { x: at.x, y: at.y });
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
