//! The AI's hand on an NPC's action queue (life.md "sense, decide,
//! act"). Every fixed step, before the queues drain, this builds each
//! NPC's `Perception`, hands it to modforge's brain, writes the states
//! back, and pushes the returned actions for this tick. From there an
//! NPC is driven exactly like the player: the same `drain_actions`,
//! `aim`, and `movement`. Nothing here decides; it senses and acts.
//! The 2D ground (x, y) is the brain's ground (x, z).

use bevy::prelude::*;
use modforge::actor::ActorId;
use modforge::brain::{Do, NEED_LINE, Perception, reaction_ticks};
use modforge::learn::alive_for;
use modforge::hud::{Holder, SlotRef};
use modforge::item::ItemRegistry;
use modforge::memory::{Did, Known, food_worth, short_of};
use modforge::monument::Roll;
use modforge::survival::Need;

use crate::{Ground, SimTick};
use crate::actor::{
    Actor, ActorCondition, Dead, Facing, Home, Hotbar, Inventory, NextThink,
    Npc, PLACE_KEY, Persona, Store, held_in, site_key,
};
use crate::combat::Health;
use crate::content::{
    ActorRegistryRes, BUNKER_SHORT_DAYS, BUNKER_WATER_DAYS, FactionRegistryRes, ItemRegistryRes,
    MonumentRegistryRes, PLAYER_FACTION,
};
use crate::input::{Action, Actions};
use crate::interact::{Container, OpenContainer, ThingId, WorldItem};
use crate::player::Player;
use crate::sight::Sight;

/// Within this of home counts as home.
pub(crate) const HOME_REACH: f32 = 6.0;

/// The third bit of a place key marks a river's water, one per chunk:
/// the shore last seen there. Not the second, a site's (`actor::SITE`),
/// or a river is taken for a site, a landmark on the way (todo 4b).
const WATER_KEY: u64 = 1 << 61;

/// The remembered key of the water in chunk `c`.
fn water_key(c: modforge::walk::ChunkKey) -> u64 {
    let pack = |v: i32| (v as i64 + (1 << 20)) as u64 & 0x1F_FFFF;
    PLACE_KEY | WATER_KEY | (pack(c.0) << 21) | pack(c.1)
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
            Option<&'static Home>,
            &'static mut Persona,
            &'static mut ActorCondition,
            (
                &'static mut Actions,
                &'static Inventory,
                &'static Hotbar,
                &'static OpenContainer,
                &'static mut NextThink,
                Has<Dead>,
                Option<&'static Store>,
                Option<&'static crate::navmesh::Walker>,
                Option<&'static crate::actor::Map>,
                Option<&'static crate::actor::Asked>,
                Option<&'static mut Holding>,
                Option<&'static crate::prisoner::Prisoner>,
            ),
        ),
        // The dead for good never think again.
        Without<crate::actor::Gone>,
    >,
    /// Their walks on the mesh, through their `Walker`.
    ways: crate::navmesh::Ways<'w, 's>,
    /// The trades they decide, made after (trade::between_people).
    trades: ResMut<'w, crate::trade::PeopleTrades>,
    /// The warnings holders decide, said after (`warn_off`).
    warnings: ResMut<'w, Warnings>,
    others: Query<
        'w,
        's,
        (Entity, &'static Transform, Option<&'static Npc>, Option<&'static Actor>),
        (Or<(With<Npc>, With<Player>)>, Without<Dead>),
    >,
    /// The sites standing, seen as they walk (todo 4b).
    standing: Option<Res<'w, crate::structure::MonumentsRes>>,
}

/// Holding a place (todo 11z; Skyrim's guard package, Mount & Blade's hold
/// this position): the place, and each one they warned off it and when.
/// The brain holds it as a guard holds home, warns once anyone they stand
/// hostile to who comes within `HOLD_REACH`, and fights them if they are
/// still within it `WARNING_GRACE` after.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Holding {
    pub at: Vec3,
    pub warned: Vec<(ActorId, u64)>,
}

/// How near the held place one comes for it, in metres (operator,
/// 2026-10-02).
pub const HOLD_REACH: f32 = 8.0;

/// How long one warned off may stay within reach before the holder fights
/// them, in ticks: 3 game seconds (Claude's number, todo 11z; the operator
/// may change it).
pub const WARNING_GRACE: u64 = 3 * 60;

/// The warnings holders decided this step (`think`): the holder, and whom
/// they warn off. Said after (`warn_off`), as trades are made after.
#[derive(Resource, Default)]
pub struct Warnings(pub Vec<(Entity, ActorId)>);

/// Each warning decided, said: the one warning line called to them, the
/// knock's way (talk::called_line, `Call::Warning`), heard where they are,
/// no conversation opened. The holder remembers saying it, the one warned
/// hearing it.
#[allow(clippy::type_complexity)]
pub fn warn_off(
    tick: Res<SimTick>,
    map: Res<crate::actor::ActorMapRes>,
    episode: Res<crate::episode::Episode>,
    lines: Res<crate::talk::Lines>,
    world: Res<crate::terrain::WorldRes>,
    seed: Res<crate::WorldSeed>,
    mut warnings: ResMut<Warnings>,
    mut people: Query<(&Actor, &Transform, &mut Persona, Has<crate::player::Player>)>,
    items: Res<crate::content::ItemRegistryRes>,
) {
    for (holder, to) in std::mem::take(&mut warnings.0) {
        let Some(warned) = map.0.handle_of(to) else {
            continue;
        };
        let is_player = people.get(warned).is_ok_and(|p| p.3);
        let Ok([(by, at, mut holder_is, _), (_, _, mut warned_is, _)]) = people.get_many_mut([holder, warned]) else {
            continue;
        };
        let feet = crate::interact::feet_of(at);
        let Some(line) = crate::talk::called_line(
            (&episode, &lines, world.bunker, seed.0, &items.0),
            (by.0, &holder_is, feet),
            (to, is_player),
            crate::talk::Call::Warning,
            tick.0,
        ) else {
            continue;
        };
        holder_is.0.memory.did(Did::Talked(to, line.clone()), tick.0);
        warned_is.0.memory.did(Did::Heard(by.0, line), tick.0);
    }
}

/// Leading someone somewhere (life.md "Going with someone"): who, to
/// where, and the spot they wait at while that person catches up.
#[derive(Component, Debug, Clone, Copy, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Leading {
    pub who: ActorId,
    pub to: Vec3,
    pub waiting_at: Option<Vec3>,
}

/// Every tick, once the queues have drained, as following: each leader is
/// sent where they lead (`Action::Go`), or, left behind by the one they
/// lead, stands where they are until they catch up (modforge
/// `brain::waits_for`). Once both are there (`brain::led_there`), the lead
/// is done.
pub fn lead(
    mut commands: Commands,
    tick: Res<SimTick>,
    mut leaders: Query<(Entity, &Transform, &mut crate::input::ThisTick, &mut Leading), Without<Dead>>,
    people: Query<(&Actor, &Transform), (Without<Leading>, Without<Dead>)>,
) {
    use modforge::actions::Stamped;
    for (entity, at, mut this_tick, mut leading) in &mut leaders {
        let here = crate::interact::feet_of(at);
        let Some(follower) = people.iter().find(|(a, _)| a.0 == leading.who).map(|(_, t)| crate::interact::feet_of(t)) else {
            continue;
        };
        // Both there: the lead is done.
        if modforge::brain::led_there(here, follower, leading.to) {
            commands.entity(entity).remove::<Leading>();
            continue;
        }
        this_tick.0.retain(|s| !matches!(s.action, Action::Go { .. }));
        // Waiting, where they stand (never walking back to where they
        // noticed): no Go, so they stop.
        if modforge::brain::waits_for(here, follower, leading.waiting_at.is_some()) {
            leading.waiting_at = Some(here);
            continue;
        }
        leading.waiting_at = None;
        let to = leading.to;
        this_tick.0.push(Stamped { tick: tick.0, action: Action::Go { x: to.x, y: to.y, z: to.z } });
    }
}

/// Following someone (life.md "Going with someone"): whom. Anyone can
/// follow anyone, the player too.
#[derive(Component, Debug, Clone, Copy, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Following(pub ActorId);

/// Every tick, once the queues have drained: each follower goes after the
/// one they follow (modforge `brain::follows`), sent where that person's
/// feet are (`Action::Go`), the way walked on the navigation mesh. A move
/// of their own this tick (the keys: a move with no aim) ends it, as in
/// World of Warcraft.
pub fn follow(
    mut commands: Commands,
    tick: Res<SimTick>,
    mut followers: Query<(Entity, &Actor, &Transform, &mut crate::input::ThisTick, &Following, Has<Dead>)>,
    people: Query<(&Actor, &Transform, Option<&Leading>), (Without<Following>, Without<Dead>)>,
) {
    use modforge::actions::{ActionQueue, Stamped};
    for (entity, me, at, mut this_tick, following, dead) in &mut followers {
        let (x, y) = ActionQueue::movement(&this_tick.0);
        let leader = people.iter().find(|(a, ..)| a.0 == following.0).map(|(_, t, leading)| (crate::interact::feet_of(t), leading));
        // Their own move, their death, or the one they follow gone: it ends.
        let own_move = (x != 0.0 || y != 0.0) && ActionQueue::aim(&this_tick.0).is_none();
        let Some((leader, leading)) = leader.filter(|_| !own_move && !dead) else {
            commands.entity(entity).remove::<Following>();
            continue;
        };
        let feet = crate::interact::feet_of(at);
        // Led somewhere and the leader there: to the spot itself, so both
        // are there (modforge `brain::led_there`); else after the leader.
        let to = match leading.filter(|l| l.who == me.0 && leader.distance(l.to) <= modforge::brain::LEAD_DONE) {
            Some(l) if !modforge::brain::walk_toward(feet, l.to).is_empty() => l.to,
            Some(_) => continue,
            None if modforge::brain::follows(feet, leader) => leader,
            None => continue,
        };
        this_tick.0.retain(|s| !matches!(s.action, Action::Go { .. }));
        this_tick.0.push(Stamped { tick: tick.0, action: Action::Go { x: to.x, y: to.y, z: to.z } });
    }
}

/// How long someone refused a trade is not asked again by the one they
/// refused, in ticks: a game minute.
const TRADE_AGAIN: u64 = 3600;

/// How long someone in view stays one sighting before another is
/// remembered, in ticks: a game minute (Claude's number, for the operator
/// to change).
const SAW_AGAIN: u64 = 3600;

/// Half the width of what a person sees ahead: a 120 degree cone
/// (life.md "Perception with belief").
const HALF_CONE: f32 = std::f32::consts::FRAC_PI_3;

/// Inside the cone ahead of someone facing `facing` at `here`.
fn in_cone(facing: &Facing, here: Vec3, at: Vec3) -> bool {
    let to = at.ground() - here.ground();
    to.length() < 1e-3 || facing.direction().angle_to(to).abs() <= HALF_CONE
}

/// Topside's PersonaBinder (modforge persona): what a person senses in
/// topside's world, worked out by `think` from their components, and what
/// the brain decided, carried out by `think` with their own actions.
struct TopsideBinder<'w> {
    sensed: Perception<'w>,
    worth: &'w dyn Fn(&Known, Need) -> f32,
    decided: Vec<Action>,
    do_now: Option<Do>,
}

impl modforge::persona::PersonaBinder for TopsideBinder<'_> {
    fn perceive<'a>(
        &'a self,
        persona: &'a modforge::persona::Persona,
        worth: &'a dyn Fn(&Known, Need) -> f32,
    ) -> Perception<'a> {
        Perception {
            needs: persona.needs,
            memory: &persona.memory,
            personality: &persona.personality,
            worth,
            ..self.sensed.clone()
        }
    }

    fn worth(&self, known: &Known, need: Need) -> f32 {
        (self.worth)(known, need)
    }

    fn act(&mut self, actions: Vec<Action>, do_now: Option<Do>) {
        self.decided = actions;
        self.do_now = do_now;
    }
}

/// Every fixed step: sense, decide, act, for every NPC.
pub fn think(
    tick: Res<SimTick>,
    registry: Res<ActorRegistryRes>,
    factions: Res<FactionRegistryRes>,
    episodes: Res<crate::content::EpisodeRegistryRes>,
    items: Res<ItemRegistryRes>,
    monuments: Res<MonumentRegistryRes>,
    tree: Res<crate::content::brain::BrainTreeRes>,
    day: Res<crate::survival::Day>,
    (storms, reality): (Res<crate::storm::StormRes>, Res<crate::Reality>),
    nav: Res<crate::navmesh::Nav>,
    archipelagos: Query<&bevy_landmass::prelude::Archipelago3d>,
    sight: Sight,
    mut thinkers: Thinkers,
    containers: Query<(Entity, &Container, &Transform, &WorldItem, &ThingId)>,
    lying: Query<(&ThingId, &Transform, &WorldItem), Without<Container>>,
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
    // Every box once: its id (what memory knows it by), where it rests
    // (its height too), what kind, for looking round.
    let boxes: Vec<(u64, Vec3, String)> = containers
        .iter()
        .map(|(_, _, t, w, id)| (id.0, t.translation, w.stack.item.clone()))
        .collect();
    // The box a remembered key names, if it is still in the world.
    let the_box = |key: u64| containers.iter().find(|c| c.4.0 == key).map(|c| c.0);
    // Every source once (a well, a berry bush: an item of kind `Source`):
    // its id, where, what kind.
    let sources_at: Vec<(u64, Vec3, String)> = lying
        .iter()
        .filter(|(_, _, w)| items.0.def(&w.stack.item).is_some_and(|d| d.kind == modforge::item::ItemKind::Source))
        .map(|(id, t, w)| (id.0, t.translation, w.stack.item.clone()))
        .collect();
    let source_at = |key: u64| {
        sources_at
            .iter()
            .find(|(id, _, _)| *id == key)
            .map(|(_, at, _)| *at)
    };
    // Where someone can go: a place on the navigation mesh, at its height
    // (pathing.md: never head for a place nobody can stand on).
    let archipelago = archipelagos.get(nav.archipelago).ok();
    let can_go = |at: Vec3| archipelago.is_some_and(|a| crate::navmesh::standable(a, at));
    // The storm is the same for everyone: coming or here, hide.
    let storm_coming = crate::storm::phase(&tick, *day, &storms) != modforge::storm::StormPhase::Calm;
    // Night is the same for everyone too: it sends them home to sleep
    // (todo 11e4, modforge brain::sleeps_tonight).
    let night = crate::survival::night(&tick, *day, &reality);
    // Everyone's position and faction first, so the pass below holds
    // no second borrow. The player carries no id yet: zero names it.
    let everyone: Vec<(Entity, ActorId, Vec3, Option<String>)> = thinkers
        .others
        .iter()
        .map(|(e, t, npc, actor)| {
            let faction = match npc {
                Some(n) => n.faction.clone(),
                None => Some(PLAYER_FACTION.to_string()),
            };
            (e, actor.map_or(ActorId(0), |a| a.0), crate::interact::feet_of(t), faction)
        })
        .collect();
    // Every standing site once: its key, what it is, where it stands, and
    // the ground of each of its buildings and props, any of which in sight
    // is the site seen.
    let sites: Vec<(u64, String, Vec3, Vec<Vec3>)> = thinkers
        .standing
        .as_ref()
        .map(|s| {
            s.0.iter()
                .map(|(at, def)| {
                    let part = |offset: Vec3| crate::on_ground(*at + Vec2::new(offset.x, offset.z), 0.0);
                    let parts = def.members.iter().map(|m| part(m.offset)).chain(def.props.iter().map(|p| part(p.position))).collect();
                    (site_key(*at), def.kind.clone(), crate::on_ground(*at, 0.0), parts)
                })
                .collect()
        })
        .unwrap_or_default();
    // How hearing words feels: the episode choice that says them.
    let heard = |words: &str| episodes.felt(words);
    // How each faction stands toward each person someone thinking now has
    // in sight (life.md "How a person feels about others"): worked out
    // here from its members' feelings, never stored.
    let mut wanted: Vec<(String, ActorId)> = Vec::new();
    for (entity, _, npc, transform, .., (_, _, _, _, next_think, dead, ..)) in thinkers.npcs.iter() {
        let (Some(faction), Some(def)) = (npc.faction.as_ref(), registry.0.def(&npc.def)) else {
            continue;
        };
        if tick.0 < next_think.0 || dead {
            continue;
        }
        let here = crate::interact::feet_of(transform);
        for (e, id, at, _) in &everyone {
            if *e != entity && at.distance(here) <= def.sight && !wanted.iter().any(|w| w.0 == *faction && w.1 == *id) {
                wanted.push((faction.clone(), *id));
            }
        }
    }
    let standings: Vec<(String, ActorId, Option<f32>)> = wanted
        .into_iter()
        .map(|(faction, toward)| {
            let feelings = thinkers
                .npcs
                .iter()
                .filter(|member| member.2.faction.as_ref() == Some(&faction))
                .map(|member| modforge::relationship::feeling(&member.7.0.memory, toward, heard));
            let standing = modforge::relationship::standing(feelings);
            (faction, toward, standing)
        })
        .collect();
    // What each person could spare of food and of water: all but one of
    // what they carry of it (design.md "Trading": one with extra of what
    // another needs), for the trader in sight below.
    let spare: Vec<(Entity, u32, u32)> = thinkers
        .npcs
        .iter()
        .map(|(entity, .., (_, inventory, ..))| {
            let count = |need: Need| inventory.0.slots.iter().flatten().filter(|s| answers(&items.0, s, need)).map(|s| s.count).sum::<u32>();
            (entity, count(Need::Hunger).saturating_sub(1), count(Need::Thirst).saturating_sub(1))
        })
        .collect();
    // In ActorId order (authority.md rule 5): the trades they decide are
    // made in this order, so who gets a seller's last water is the same
    // after a load.
    let order = crate::actor::in_actor_order(thinkers.npcs.iter().map(|(entity, actor, ..)| (Some(actor.0), entity)));
    for entity in order {
        let Ok((
            entity,
            actor,
            npc,
            transform,
            facing,
            health,
            home,
            mut person,
            mut condition,
            (mut actions, inventory, hotbar, open, mut next_think, dead, store, way, map, asked, mut holding, prisoner),
        )) = thinkers.npcs.get_mut(entity)
        else {
            continue;
        };
        // Who they are, their Persona: their needs and personality read
        // as they are now; their memory and what they are doing changed in
        // it, and it thinks.
        let (needs, personality) = (person.0.needs, person.0.personality);
        // Not their moment yet: they keep doing what they decided.
        if tick.0 < next_think.0 {
            continue;
        }
        // Dead and not gone for good (a bunker person, or a topside person
        // where death does not stick; `combat::die` decides): at their
        // moment they respawn, the same Respawn the player's Enter pushes.
        if dead {
            actions.0.push(tick.0, Action::Respawn);
            next_think.0 = tick.0 + (modforge::brain::AWAKE_REACTION * 60.0) as u64;
            continue;
        }
        let Some(def) = registry.0.def(&npc.def) else {
            continue;
        };
        let here = crate::interact::feet_of(transform);
        // Sense: the nearest they stand hostile to (their own feeling,
        // else their faction's standing, else the faction table) within
        // the def's sight, inside the cone ahead, with nothing blocking
        // the view (sight.rs).
        let stands_hostile = |id: ActorId, faction: Option<&str>| {
            let own = modforge::relationship::feeling(&person.0.memory, id, heard);
            let standing = npc
                .faction
                .as_ref()
                .and_then(|f| standings.iter().find(|s| s.0 == *f && s.1 == id))
                .and_then(|s| s.2);
            let table = factions.0.relation(npc.faction.as_deref(), faction);
            modforge::relationship::stands(own, standing, table) == modforge::faction::Relation::Hostile
        };
        let hostile = everyone
            .iter()
            .filter(|(e, id, at, faction)| {
                *e != entity
                    && at.distance(here) <= def.sight
                    && in_cone(facing, here, *at)
                    && stands_hostile(*id, faction.as_deref())
                    && sight.clear(here, *at)
            })
            .min_by(|a, b| a.2.distance(here).total_cmp(&b.2.distance(here)))
            .map(|(_, id, at, _)| (*id, *at));
        // Who in sight they do not stand hostile to: whom they might trade
        // with (below).
        let not_hostile_in_sight: Vec<(Entity, ActorId, Vec3)> = everyone
            .iter()
            .filter(|(e, id, at, faction)| {
                *e != entity && at.distance(here) <= def.sight && in_cone(facing, here, *at) && !stands_hostile(*id, faction.as_deref()) && sight.clear(here, *at)
            })
            .map(|(e, id, at, _)| (*e, *id, *at))
            .collect();
        // Holding a place (`Holding`): anyone they stand hostile to within
        // `HOLD_REACH` of it, in view, is warned once (said after,
        // `warn_off`); one still within it `WARNING_GRACE` after is the one
        // they fight, by the guard's own fight; nobody else.
        let hostile = match holding.as_deref_mut() {
            None => hostile,
            Some(holding) => {
                let place = holding.at;
                let mut coming: Vec<(ActorId, Vec3)> = everyone
                    .iter()
                    .filter(|(e, id, at, faction)| *e != entity && at.distance(place) <= HOLD_REACH && stands_hostile(*id, faction.as_deref()) && sight.clear(here, *at))
                    .map(|(_, id, at, _)| (*id, *at))
                    .collect();
                // In ActorId order (authority.md rule 5).
                coming.sort_by_key(|(id, _)| id.0);
                for (id, _) in &coming {
                    if !holding.warned.iter().any(|(w, _)| w == id) {
                        holding.warned.push((*id, tick.0));
                        thinkers.warnings.0.push((entity, *id));
                    }
                }
                coming
                    .into_iter()
                    .filter(|(id, _)| holding.warned.iter().any(|(w, when)| w == id && tick.0 >= when + WARNING_GRACE))
                    .min_by(|a, b| a.1.distance(here).total_cmp(&b.1.distance(here)))
            }
        };
        // A prisoner fights no one (todo 11u, prisoner.rs).
        let hostile = hostile.filter(|_| prisoner.is_none());
        // A guard holds home; one holding a place holds it the same way.
        let home_at = holding.as_ref().map(|h| h.at).or(home.map(|h| h.0));
        let at_home = home_at.is_some_and(|h| h.distance(here) <= HOME_REACH);
        condition.0.threat_in_sight = hostile.is_some();
        condition.0.at_home = at_home;
        if let Some((who, at)) = hostile {
            person.0.memory.threat(who, at, tick.0);
        }
        // Everyone in sight, as the hostile and the not hostile above are
        // seen, is remembered seen (modforge `Memory::saw`: once a
        // `SAW_AGAIN`, not once a think).
        for (e, id, at, _) in &everyone {
            if *e != entity && at.distance(here) <= def.sight && in_cone(facing, here, *at) && sight.clear(here, *at) {
                person.0.memory.saw(*id, tick.0, SAW_AGAIN);
            }
        }
        // Looking round as they think: every box in the cone with a clear
        // line goes into memory (seen, not yet looked inside).
        for (key, at, kind) in boxes.iter().chain(&sources_at) {
            if at.distance(here) <= def.sight && in_cone(facing, here, *at) && sight.clear(here, *at) {
                person.0.memory.see(*key, kind, *at, tick.0);
            }
        }
        // A site seen (todo 4b): any of its buildings or props in the cone
        // within sight. No line is checked, as the building itself is what
        // stands in the way (Claude's, for the operator to change).
        for (key, kind, at, parts) in &sites {
            if parts.iter().any(|p| p.distance(here) <= def.sight && in_cone(facing, here, *p)) {
                person.0.memory.see(*key, kind, *at, tick.0);
            }
        }
        // A river seen: the nearest shore in sight, one memory per chunk,
        // where they can stand and drink (design.md: rivers drinkable).
        if let Some(chunks) = chunks.as_ref() {
            let c = crate::terrain::chunk_of(here.ground());
            // A shore is on the ground: its land tile at the ground's height.
            let seen = (c.1 - 1..=c.1 + 1)
                .flat_map(|y| (c.0 - 1..=c.0 + 1).map(move |x| (x, y)))
                .filter_map(|k| chunks.shores.get(&k))
                .flatten()
                .map(|s| crate::on_ground(modforge::walk::centre(s.land), 0.0))
                .filter(|p| p.distance(here) <= def.sight && in_cone(facing, here, *p))
                .min_by(|a, b| a.distance(here).total_cmp(&b.distance(here)));
            if let Some(shore) = seen.filter(|p| sight.clear(here, *p)) {
                let key = water_key(crate::terrain::chunk_of(shore.ground()));
                person.0.memory.see(key, crate::interact::RIVER, shore, tick.0);
            }
        }
        // A box they have open, they see into, as the player sees the
        // open box's panel.
        if let Some((key, container)) = open.0.and_then(|e| containers.get(e).ok().map(|(_, c, _, _, id)| (id.0, c))) {
            person.0.memory.checked(key, held_in(&container.0), modforge::fluid::litres_in(&container.0), tick.0);
        }
        person.0.memory.forget_old(tick.0);
        let carried = carried_slot(inventory, hotbar, &items.0, Need::Hunger);
        let carries_drink = carried_slot(inventory, hotbar, &items.0, Need::Thirst).is_some();
        // Their bunker: the store where they last saw it, and what the
        // bunker is short of by what they remember in it plus what they
        // carry home (design.md "Taking loot").
        let store_key = store.and_then(|s| containers.get(s.0).ok()).map(|c| c.4.0);
        let store_at = store_key.and_then(|key| person.0.memory.known.iter().find(|k| k.key == key).map(|k| (key, k.position)));
        let bags = held_in(&inventory.0);
        // Food they carry belongs in the store; their water is poured into
        // its water (modforge fluid), counted in litres.
        let carries_for_bunker = inventory.0.slots.iter().flatten().any(|s| answers(&items.0, s, Need::Hunger));
        let carried_litres = modforge::fluid::litres_in(&inventory.0) + modforge::fluid::litres_in(&hotbar.0.slots);
        let water_room = modforge::fluid::room_in(&inventory.0, &items.0) + modforge::fluid::room_in(&hotbar.0.slots, &items.0);
        let bunker_short = store_at
            .map(|(key, _)| {
                let known = person.0.memory.known.iter().find(|k| k.key == key);
                let in_store = known.and_then(|k| k.held.clone()).unwrap_or_default();
                let litres = known.map_or(0.0, |k| k.litres) + carried_litres;
                let per_day = day.per_day();
                let supplies = in_store.iter().chain(&bags).map(|(k, n)| (k.as_str(), *n));
                // What they stock to: what is needed, and past it by their
                // fear of not having enough, up to twice it (todo 11ag).
                let fear = 1.0 + personality.get(modforge::actor::Axis::Fear).clamp(0.0, 1.0);
                // The bunker's drinking is its living people's (the player
                // in the player's bunker), not a set number (orchestrator,
                // 2026-10-03).
                let living = everyone.iter().filter(|(.., faction)| faction.is_some() && *faction == npc.faction).count().max(1) as u32;
                short_of(supplies, litres, living, (BUNKER_SHORT_DAYS * fear, BUNKER_WATER_DAYS * fear), per_day, &items.0)
            })
            .unwrap_or_default();
        let bags_full = inventory.0.slots.iter().all(|s| s.is_some());
        // How long they can stay alive from here (life.md "Learning to
        // stay alive"): the change since they last thought scores the
        // choices that led to it.
        let mut carrying = bags.clone();
        carrying.extend(held_in(&hotbar.0.slots));
        let alive = alive_for(
            &needs,
            day.rates(),
            health.0.fraction(),
            &carrying,
            store_key,
            &person.0.memory,
            &items.0,
        );
        person.0.memory.learned.score(tick.0, alive);
        // A trip the navigation mesh has no way for (pathing.md) is
        // dropped: they pick again from the root. A thing they could not
        // reach counts as looked at, so the same one is not picked again
        // at once.
        let unreached = match person.0.doing.target {
            modforge::statetree::Target::Point(_) => Some(None),
            modforge::statetree::Target::Thing { key, .. } => Some(Some(key)),
            _ => None,
        };
        let no_way = crate::navmesh::way_of(way, &thinkers.ways)
            .is_some_and(|(s, _)| matches!(s, bevy_landmass::AgentState::NoPath | bevy_landmass::AgentState::TargetNotOnNavMesh));
        if let Some(thing) = unreached
            && no_way
        {
            match thing {
                Some(key) if key & PLACE_KEY != 0 || source_at(key).is_some() => person.0.memory.visited(key, tick.0),
                Some(key) => person.0.memory.checked(key, Vec::new(), 0.0, tick.0),
                None => {}
            }
            person.0.doing = Default::default();
        }
        let standable = |at: Vec3| can_go(at);
        // Places to stroll to: points round them every 3 m, 15 m each way,
        // on the mesh at their height (the mesh, not a grid, knows where
        // anyone can stand).
        let reachable = || {
            (-5..=5)
                .flat_map(|z| (-5..=5).map(move |x| here + Vec3::new(x as f32 * 3.0, 0.0, z as f32 * 3.0)))
                .filter(|p| can_go(*p))
                .collect()
        };
        let unknown = |at: Vec3| !map.is_some_and(|m| m.map.seen(modforge::walk::cell_of(Vec2::new(at.x, at.z))));
        // Someone to trade with (design.md "Trading"): in sight, not one
        // they stand hostile to, nor one who refused them lately, carrying
        // more than they need of what answers a need of theirs under the
        // need line that they carry nothing for.
        let refused_lately = |id: ActorId| {
            person.0.memory.done.iter().rev().take_while(|(t, _)| tick.0.saturating_sub(*t) < TRADE_AGAIN).any(|(_, d)| matches!(d, Did::TradeRefused(by) if *by == id))
        };
        let short: Vec<Need> = [(Need::Thirst, needs.thirst, carries_drink), (Need::Hunger, needs.hunger, carried.is_some())]
            .into_iter()
            .filter(|(_, value, carries)| *value < NEED_LINE && !carries)
            .map(|(need, ..)| need)
            .collect();
        let trader = not_hostile_in_sight
            .iter()
            .filter(|(_, id, _)| !refused_lately(*id))
            .filter_map(|(e, id, at)| {
                let (_, food, water) = spare.iter().find(|s| s.0 == *e)?;
                let need = short.iter().copied().find(|n| if *n == Need::Hunger { *food > 0 } else { *water > 0 })?;
                Some((*id, *at, need))
            })
            .min_by(|a, b| a.1.distance(here).total_cmp(&b.1.distance(here)));
        // Topside's own facts: the storm, the bunker, the water carried.
        let facts = crate::content::brain::TopsideFacts {
            storm_coming,
            store: store_at,
            bunker_short: bunker_short.clone(),
            carries_for_bunker,
            carried_litres,
            water_room,
        };
        // What they sense, but for what the persona brings (their needs,
        // memory, personality): the binder puts those in each think.
        let blank = modforge::memory::Memory::default();
        let sensed = Perception {
            now: tick.0,
            position: here,
            yaw: facing.0,
            health_fraction: health.0.fraction(),
            needs,
            home: home_at,
            behaviour: if holding.is_some() { modforge::actor::Behaviour::Guard } else { def.behaviour },
            at_home,
            asleep: condition.0.asleep,
            hostile,
            game: &facts,
            night,
            carries_food: carried.is_some(),
            carries_drink,
            bags_full,
            memory: &blank,
            personality: &personality,
            worth: &worth,
            standable: &standable,
            reachable: &reachable,
            unknown: &unknown,
            // Holding a place, they are asked to be there: they go back to
            // it and wait, the one errand path.
            asked: holding.as_ref().map(|h| h.at).or(asked.map(|a| a.0)),
            trader,
        };

        // Decide through the one tree: their own Persona thinks through
        // topside's binder, seeded by the tick and the person, so a replay
        // decides the same; what they chose goes to what they learn from.
        let mut roll = Roll::new(tick.0 ^ actor.0.0.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let mut binder = TopsideBinder { sensed, worth: &worth, decided: Vec::new(), do_now: None };
        modforge::persona::think(&mut binder, &mut person.0, &tree.0, &mut roll);
        let TopsideBinder { decided, do_now, .. } = binder;

        // Act. Their next decision comes at their reaction time.
        let fighting = crate::content::brain::fighting(&person.0.doing);
        next_think.0 = tick.0 + reaction_ticks(condition.0.asleep, fighting, needs.rest, &personality, 60.0);
        for action in decided {
            actions.0.push(tick.0, action);
        }
        // What the brain wants done is done with the player's own actions:
        // Use opens a box, Transfer takes a stack out of the open box,
        // UseSlot eats or drinks, Cancel closes the box.
        let mut act = |action: Action| actions.0.push(tick.0, action);
        // Fighting with the melee weapon put away to eat or drink (below):
        // it comes back out of its hotbar slot (modforge hotbar_key).
        if fighting && !crate::interact::melee_in_hand(&hotbar.0, &items.0) {
            let armed = hotbar.0.slots.slots.iter().position(|s| {
                s.as_ref()
                    .and_then(|s| items.0.def(&s.item))
                    .and_then(|d| d.combat.as_ref())
                    .is_some_and(|c| c.ammo.is_none())
            });
            if let Some(index) = armed {
                act(Action::Hotbar { index });
            }
        }
        let where_is = |e: Entity| containers.get(e).map_or(here, |(_, _, t, _, _)| t.translation);
        let mut keep_open = None;
        match do_now {
            None => {}
            Some(Do::Sleep) => {
                condition.0.asleep = true;
                person.0.memory.did(Did::Slept, tick.0);
            }
            Some(Do::Wake) => condition.0.asleep = false,
            // A trade moves things between two people: made after, from
            // both their bags (trade::between_people).
            Some(Do::Trade { with, need }) => thinkers.trades.0.push((entity, with, need)),
            // A key is a place (PLACE_KEY set) or a box (its ThingId): a
            // place is visited; a box is opened to see inside (noted
            // above at the next think); a box that is gone is forgotten.
            // A place, a river's water, or a source (a well, a bush) is
            // visited: there is no inside to look into.
            Some(Do::Check { key }) if key & PLACE_KEY != 0 || source_at(key).is_some() => person.0.memory.visited(key, tick.0),
            Some(Do::Check { key }) => match the_box(key) {
                Some(found) => {
                    keep_open = Some(found);
                    open_box(&mut act, open, found, where_is(found));
                }
                None => person.0.memory.gone(key),
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
                match chunks.as_ref().and_then(|c| crate::interact::nearest_water(c, here.ground(), here.ground())) {
                    Some(water) => {
                        act(Action::Aim { x: water.x, y: water.y });
                        act(Action::Use);
                    }
                    None => person.0.memory.gone(key),
                }
            }
            // At a source (a well, a berry bush): use it where it stands,
            // a mouthful a think, until the need is met.
            // A melee weapon in hand goes out of hand first (its own
            // hotbar slot again empties the hands, modforge hotbar_key):
            // used with one, a thing that breaks when struck is broken
            // (interact::interact_on_use).
            Some(Do::Eat { key, .. }) if source_at(key).is_some() => {
                let at = source_at(key).expect("checked above");
                if let Some(index) = hotbar.0.held.filter(|_| crate::interact::melee_in_hand(&hotbar.0, &items.0)) {
                    act(Action::Hotbar { index });
                }
                act(Action::Aim { x: at.x, y: at.z });
                act(Action::Use);
            }
            // At the box: open it, then eat or drink one thing out of it
            // for the need that sent them, one a think, until it is met.
            // You eat what you need; nothing is carried off.
            Some(Do::Eat { key, need }) => {
                let Some(found) = the_box(key) else {
                    person.0.memory.gone(key);
                    continue;
                };
                keep_open = Some(found);
                if open.0 != Some(found) {
                    open_box(&mut act, open, found, where_is(found));
                    continue;
                }
                let (_, container, ..) = containers.get(found).expect("checked above");
                let food = container.0.slots.iter().position(|s| s.as_ref().is_some_and(|s| answers(&items.0, s, need)));
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
                    person.0.memory.gone(key);
                    continue;
                };
                keep_open = Some(found);
                if open.0 != Some(found) {
                    open_box(&mut act, open, found, where_is(found));
                    continue;
                }
                let needed = |stack: &modforge::item::ItemStack| {
                    [Need::Hunger, Need::Thirst].into_iter().any(|n| {
                        let own = match n {
                            Need::Hunger => needs.hunger,
                            _ => needs.thirst,
                        };
                        (bunker_short.contains(&n) || own < NEED_LINE) && answers(&items.0, stack, n)
                    })
                };
                let (_, container, ..) = containers.get(found).expect("checked above");
                for (index, stack) in container.0.slots.iter().enumerate() {
                    if stack.as_ref().is_some_and(|s| needed(s)) {
                        let slot = SlotRef {
                            holder: Holder::Container,
                            index,
                        };
                        act(Action::Transfer { half: false, slot: Some(slot) });
                    }
                }
            }
            // At their store: open it, then put in every stack of food
            // they carry.
            Some(Do::Stock { key }) => {
                let Some(found) = the_box(key) else {
                    person.0.memory.gone(key);
                    continue;
                };
                keep_open = Some(found);
                if open.0 != Some(found) {
                    open_box(&mut act, open, found, where_is(found));
                    continue;
                }
                for (index, stack) in inventory.0.slots.iter().enumerate() {
                    if stack.as_ref().is_some_and(|s| answers(&items.0, s, Need::Hunger)) {
                        let slot = SlotRef {
                            holder: Holder::Inventory,
                            index,
                        };
                        act(Action::Transfer { half: false, slot: Some(slot) });
                    }
                }
            }
            // At their store: open it, then pour every container they carry
            // into its water (Action::Pour, the player's own).
            Some(Do::Pour { key }) => {
                let Some(found) = the_box(key) else {
                    person.0.memory.gone(key);
                    continue;
                };
                keep_open = Some(found);
                if open.0 != Some(found) {
                    open_box(&mut act, open, found, where_is(found));
                    continue;
                }
                for slot in containers_holding(inventory, hotbar, |s| s.litres > 0.0) {
                    act(Action::Pour { slot });
                }
            }
            // At a source (a river's water, a well, the tap): aim at it, the
            // weapon out of hand, then fill every container they carry with
            // room (Action::Fill, the player's own).
            Some(Do::Fill { key }) => {
                let at = if key & WATER_KEY != 0 {
                    chunks.as_ref().and_then(|c| crate::interact::nearest_water(c, here.ground(), here.ground())).map(|w| Vec3::new(w.x, here.y, w.y))
                } else {
                    source_at(key)
                };
                let Some(at) = at else {
                    person.0.memory.gone(key);
                    continue;
                };
                if let Some(index) = hotbar.0.held.filter(|_| crate::interact::melee_in_hand(&hotbar.0, &items.0)) {
                    act(Action::Hotbar { index });
                }
                act(Action::Aim { x: at.x, y: at.z });
                for slot in containers_holding(inventory, hotbar, |s| modforge::fluid::room(s, &items.0) > 0.0) {
                    act(Action::Fill { slot });
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
fn open_box(act: &mut impl FnMut(Action), open: &OpenContainer, found: Entity, at: Vec3) {
    if open.0 == Some(found) {
        return;
    }
    if open.0.is_some() {
        act(Action::Cancel);
    }
    act(Action::Aim { x: at.x, y: at.z });
    act(Action::Use);
}

use crate::interact::answers;

/// The first slot in a person's bags (hotbar, then inventory) holding
/// something that answers `need`.
fn carried_slot(inventory: &Inventory, hotbar: &Hotbar, items: &ItemRegistry, need: Need) -> Option<SlotRef> {
    let find = |slots: &modforge::item::Inventory, holder| {
        slots
            .slots
            .iter()
            .position(|s| s.as_ref().is_some_and(|s| answers(items, s, need)))
            .map(|index| SlotRef { holder, index })
    };
    find(&hotbar.0.slots, Holder::Hotbar).or_else(|| find(&inventory.0, Holder::Inventory))
}

/// Every slot in a person's bags (hotbar, then inventory) holding a
/// container (modforge fluid) that `fits`.
fn containers_holding(inventory: &Inventory, hotbar: &Hotbar, fits: impl Fn(&modforge::item::ItemStack) -> bool) -> Vec<SlotRef> {
    let find = |slots: &modforge::item::Inventory, holder| {
        slots
            .slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.as_ref().is_some_and(|s| s.count == 1 && fits(s)))
            .map(move |(index, _)| SlotRef { holder, index })
            .collect::<Vec<_>>()
    };
    let mut found = find(&hotbar.0.slots, Holder::Hotbar);
    found.extend(find(&inventory.0, Holder::Inventory));
    found
}
