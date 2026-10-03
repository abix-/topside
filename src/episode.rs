//! The episode running now (episodes.md): started by the storm after the
//! reality, its parts cast from the bunkers' people once they exist.

use bevy::prelude::*;
use modforge::actor::ActorId;
use modforge::storyteller::{CastBunker, cast};

use modforge::item;
use modforge::memory::Did;

use crate::Ground;
use crate::actor::{Actor, Dead, Home, Memory, Npc, Store, site_key};
use crate::content::{BUNKER_AT, EpisodeRegistryRes, ItemRegistryRes, PLAYER_FACTION};
use crate::interact::{DrawsFrom, spawn_item};

/// The episode running now: one that fits the reality, started by the
/// storm that handed the reality over; its cast and the pivot points
/// reached so far.
#[derive(Resource, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct Episode {
    /// None when no episode fits the reality.
    pub name: Option<String>,
    /// Each part and the person cast for it.
    pub cast: Vec<(String, ActorId)>,
    /// Whether the cast has been filled for this episode.
    pub cast_done: bool,
    /// Each pivot point reached: its name, who, and when.
    pub reached: Vec<(String, ActorId, u64)>,
    /// What the episode brought into the world, where (in 3D, its height
    /// included), and the key memory knows it by.
    pub things: Vec<(String, Vec3, u64)>,
    /// The tick it started: what was said before it is not its.
    pub started: u64,
    /// Who played which part in the episode before, for the parts cast from
    /// memory (modforge `CastFrom::PlayedBefore`).
    pub cast_before: Vec<(String, ActorId)>,
}

impl Episode {
    /// A new episode starts at this tick: nobody cast, nothing reached, the
    /// last one's cast kept as who played what before.
    pub fn start(&mut self, name: Option<String>, tick: u64) {
        let before = std::mem::take(&mut self.cast);
        *self = Self::new(name, tick);
        self.cast_before = before;
    }

    fn new(name: Option<String>, started: u64) -> Self {
        Self {
            name,
            cast: Vec::new(),
            cast_done: false,
            reached: Vec::new(),
            things: Vec::new(),
            started,
            cast_before: Vec::new(),
        }
    }

    /// A pivot point reached, once.
    fn reach(&mut self, pivot: &str, who: ActorId, tick: u64) {
        if !self.reached.iter().any(|(p, ..)| p == pivot) {
            info!("{:?}: {pivot}, by {who:?} at tick {tick}", self.name);
            self.reached.push((pivot.to_string(), who, tick));
        }
    }

    fn reached(&self, pivot: &str) -> bool {
        self.reached.iter().any(|(p, ..)| p == pivot)
    }

    /// The names of the pivot points reached, for matching lines.
    pub fn reached_names(&self) -> Vec<String> {
        self.reached.iter().map(|(p, ..)| p.clone()).collect()
    }

    /// The thing the episode brought in by this name.
    pub fn thing(&self, name: &str) -> Option<(Vec3, u64)> {
        self.things.iter().find(|(n, ..)| n == name).map(|(_, at, key)| (*at, *key))
    }

    /// The person cast for a part.
    pub fn cast_as(&self, part: &str) -> Option<ActorId> {
        self.cast.iter().find(|(p, _)| p == part).map(|(_, id)| *id)
    }

    /// The part a person is cast as, if any.
    pub fn part_of(&self, who: ActorId) -> Option<&str> {
        self.cast.iter().find(|(_, id)| *id == who).map(|(p, _)| p.as_str())
    }
}

impl FromWorld for Episode {
    fn from_world(world: &mut World) -> Self {
        let reality = world.get_resource_or_init::<crate::Reality>().0.name.clone();
        let seed = world.resource::<crate::WorldSeed>().0;
        let episodes = world.get_resource_or_init::<EpisodeRegistryRes>();
        Self::new(episodes.0.pick(&reality, seed, 0).map(|e| e.name.clone()), 0)
    }
}

/// Once the bunker people exist, cast the episode's parts from them
/// (modforge `cast`): each bunker's living people, the player's bunker
/// marked, each by its distance from the player's.
pub fn cast_episode(
    mut episode: ResMut<Episode>,
    episodes: Res<EpisodeRegistryRes>,
    seed: Res<crate::WorldSeed>,
    people: Query<(&Actor, &Npc, &Home), Without<Dead>>,
    (memories, player): (Query<(&Actor, &Memory)>, Query<&Actor, With<crate::player::Player>>),
) {
    if episode.cast_done {
        return;
    }
    let Some(def) = episode.name.as_deref().and_then(|n| episodes.0.def(n)) else {
        episode.cast_done = true;
        return;
    };
    if people.is_empty() {
        return;
    }
    let mut bunkers: Vec<(Option<String>, CastBunker<ActorId>)> = Vec::new();
    for (actor, npc, home) in &people {
        let at = bunkers.iter().position(|(f, _)| *f == npc.faction);
        let at = at.unwrap_or_else(|| {
            let player = npc.faction.as_deref() == Some(PLAYER_FACTION);
            bunkers.push((
                npc.faction.clone(),
                CastBunker {
                    player,
                    distance: home.0.ground().distance(BUNKER_AT),
                    people: Vec::new(),
                },
            ));
            bunkers.len() - 1
        });
        bunkers[at].1.people.push(actor.0);
    }
    // The same people in the same order each time, so the seed casts
    // the same way.
    bunkers.sort_by(|a, b| a.0.cmp(&b.0));
    for (_, bunker) in &mut bunkers {
        bunker.people.sort_by_key(|id| id.0);
    }
    let bunkers: Vec<CastBunker<ActorId>> = bunkers.into_iter().map(|(_, b)| b).collect();
    let parts = def.parts.clone();
    // How each person feels toward the player, from their own memory (the
    // parts cast from memory).
    let me = player.single().ok().map(|a| a.0);
    let feels = |id: ActorId| {
        let memory = memories.iter().find(|(a, _)| a.0 == id).map(|(_, m)| m)?;
        modforge::relationship::feeling(&memory.0, me?, |line| episodes.felt(line))
    };
    let before = episode.cast_before.clone();
    episode.cast = cast(&parts, &bunkers, &before, feels, seed.0);
    episode.cast_done = true;
    info!("episode {:?} cast: {:?}", episode.name, episode.cast);
}

/// How far from the way a site can stand and still be on it, metres.
pub(crate) const ON_THE_WAY: f32 = 80.0;

/// How far `at` stands from the way `from` to `to`, and how far along it
/// (0 at `from`, 1 at `to`).
pub(crate) fn off_the_way(from: Vec2, to: Vec2, at: Vec2) -> (f32, f32) {
    let way = to - from;
    let along = ((at - from).dot(way) / way.length_squared().max(1.0)).clamp(0.0, 1.0);
    (at.distance(from + way * along), along)
}

/// Once the episode is cast, each thing its def brings into the world
/// (modforge `ThingDef`, Skyrim's quest aliases) comes in through
/// `interact::spawn_item`, once: inside a building of the site nearest the
/// middle between its two places, drawing from its part's store, known by
/// the part who found it as a place they have been, with every site near
/// the way there from the first place. It is the episode's thing under its
/// name, and its site under "<name> site" (the lines' and pivot points'
/// names for them).
#[allow(clippy::too_many_arguments)]
pub fn place_things(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    mut episode: ResMut<Episode>,
    episodes: Res<EpisodeRegistryRes>,
    items: Res<ItemRegistryRes>,
    mut world: ResMut<crate::terrain::WorldRes>,
    registries: (Res<crate::content::BiomeRegistryRes>, Res<crate::content::MonumentRegistryRes>),
    people: Query<(&Actor, &Home, &Store)>,
    mut memories: Query<(&Actor, &mut Memory)>,
    mut ids: ResMut<crate::interact::ThingIssuer>,
    stores: Query<&crate::interact::Container>,
) {
    if !episode.cast_done {
        return;
    }
    let Some(def) = episode.name.as_deref().and_then(|n| episodes.0.def(n)) else {
        return;
    };
    let placed = |episode: &Episode, name: &str| episode.things.iter().any(|(n, ..)| n == name || *n == format!("no {name}"));
    let to_place: Vec<&modforge::storyteller::ThingDef> = def.things.iter().filter(|t| !placed(&episode, &t.name)).collect();
    for thing in to_place {
        let Some(item_def) = items.0.def(&thing.item) else {
            warn!("{:?}: no item {} for its {}", episode.name, thing.item, thing.name);
            continue;
        };
        // A place on the ground: the player's bunker door, or a part's home
        // (sites stand on the ground, so the way between is read there).
        let home_of = |part: &str| episode.cast_as(part).and_then(|id| people.iter().find(|(a, ..)| a.0 == id)).map(|(_, home, _)| home.0.ground());
        let place = |p: &modforge::storyteller::Place| match p {
            modforge::storyteller::Place::PlayerBunkerDoor => Some(crate::structure::bunker_door(world.bunker).0),
            modforge::storyteller::Place::HomeOf(part) => home_of(part),
        };
        let (Some(from), Some(to)) = (place(&thing.between.0), place(&thing.between.1)) else {
            continue;
        };
        let (biomes, monuments) = &registries;
        let r = modforge::world2d::Registries {
            biomes: &biomes.0,
            monuments: &monuments.monuments,
            buildings: &monuments.buildings,
        };
        // Anywhere between the two: as far out as half the way is long.
        let margin = Vec2::splat((from.distance(to) / 2.0).max(ON_THE_WAY));
        let sites: Vec<modforge::places::Place> = world
            .world
            .sites_in(r, from.min(to) - margin, from.max(to) + margin)
            .into_iter()
            .filter(|p| !p.rolled.members.is_empty())
            .collect();
        let middle = (from + to) / 2.0;
        let Some(site) = sites.iter().min_by(|a, b| a.position.distance(middle).total_cmp(&b.position.distance(middle))) else {
            warn!("{:?}: no building between its places to put the {} in", episode.name, thing.name);
            episode.things.push((format!("no {}", thing.name), crate::on_ground(middle, 0.0), 0));
            continue;
        };
        let member = &site.rolled.members[0];
        let at = site.position + Vec2::new(member.offset.x, member.offset.z);
        // On the building's ground floor.
        let thing_at = crate::structure::on_floor(at, 0.0);
        let (made, id) = spawn_item(&mut commands, &mut ids, item_def, item::create(item_def, 1, &[], 0.0, 0), thing_at);
        let store = thing.draws_from.as_deref().and_then(|part| episode.cast_as(part)).and_then(|id| people.iter().find(|(a, ..)| a.0 == id)).map(|(.., s)| s.0);
        if let Some(store) = store {
            commands.entity(made).insert(DrawsFrom(store));
            // What the store holds as it starts being drawn from.
            let held = stores.get(store).map_or(0, |held| crate::interact::water_in(&held.0, &items));
            commands.entity(store).insert(crate::interact::Drawn { held });
        }
        let key = id.0;
        let ground = |p: Vec2| crate::on_ground(p, 0.0);
        if let Some(finder) = thing.found_by.as_deref().and_then(|part| episode.cast_as(part))
            && let Some((_, mut memory)) = memories.iter_mut().find(|(a, _)| a.0 == finder)
        {
            // Found, so they have been there: never a thing they have yet
            // to look at (the brain's look walked Dell out to the tap before
            // the opening, the operator's playtest 2026-10-03).
            memory.0.see(key, &thing.item, thing_at, tick.0);
            memory.0.visited(key, tick.0);
            if thing.kept_quiet {
                memory.0.keep_quiet(key);
            }
            // Every site near the way there, passed: been there too.
            for passed in sites.iter().filter(|p| off_the_way(from, at, p.position).0 <= ON_THE_WAY) {
                memory.0.see(site_key(passed.position), &passed.monument, ground(passed.position), tick.0);
                memory.0.visited(site_key(passed.position), tick.0);
            }
        }
        episode.things.push((thing.name.clone(), thing_at, key));
        episode.things.push((format!("{} site", thing.name), ground(site.position), site_key(site.position)));
        info!("{:?}: the {} stands in the {} at {at}", episode.name, thing.name, site.monument);
    }
}

/// Every step: each cue of the episode running whose deed the player has
/// done (modforge `cues_due`; The Tap: the note read) has the person cast
/// in its part do its order, through the one way an order reaches a person
/// (command::order), and is reached, once.
pub fn cues(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    mut episode: ResMut<Episode>,
    episodes: Res<EpisodeRegistryRes>,
    players: Query<(&Actor, &Memory), With<crate::player::Player>>,
    mut people: Query<(Entity, &Actor, Option<&mut crate::command::Commanded>)>,
) {
    let Some(def) = episode.name.as_ref().and_then(|name| episodes.0.def(name)) else {
        return;
    };
    let Ok((player, memory)) = players.single() else {
        return;
    };
    let reached: Vec<&str> = episode.reached.iter().map(|(name, ..)| name.as_str()).collect();
    let due: Vec<modforge::storyteller::CueDef> = modforge::storyteller::cues_due(def, &memory.0.done, episode.started, &reached).into_iter().cloned().collect();
    let mut new = std::collections::HashMap::new();
    for cue in due {
        let Some(who) = episode.cast_as(&cue.part) else {
            continue;
        };
        // Whom the order names and where, worked out now (the one map,
        // modforge `Command::resolve`): the player or a part, an episode
        // thing where it stands.
        let whom = |w: modforge::storyteller::Who| match w {
            modforge::storyteller::Who::ThePlayer => Some(player.0.0),
            modforge::storyteller::Who::Part(part) => episode.cast_as(&part).map(|id| id.0),
        };
        let at = |thing: String| episode.thing(&thing).map(|(at, _)| modforge::command::Point::from(at));
        match cue.does.clone().resolve(whom, at) {
            Some(order) => crate::command::order(&mut people, &mut new, who.0, order),
            None => warn!("cue {}: its order {:?} names someone or something not there; not given", cue.name, cue.does),
        }
        info!("cue {}: {} ({:?}) {:?}", cue.name, cue.part, who, cue.does);
        episode.reached.push((cue.name.clone(), who, tick.0));
    }
    for (entity, left) in new {
        commands.entity(entity).insert(crate::command::Commanded { left, started: false });
    }
}


/// Pivot points reached by what was said (episodes.md "One way for every
/// conversation"): once a game second, every line of the episode said by
/// anyone since it started, read from memory (the one record), is reached
/// under its name ("the opening", "believes Dell", "threatens Mara").
/// Pivot points reached by what was done are reached where it is seen
/// (`errands`' arrivals, `deeds`).
pub fn pivots_said(
    tick: Res<crate::SimTick>,
    mut episode: ResMut<Episode>,
    episodes: Res<EpisodeRegistryRes>,
    people: Query<(&Actor, &Memory)>,
) {
    if tick.0 % 60 != 0 {
        return;
    }
    let Some(def) = episode.name.as_deref().and_then(|n| episodes.0.def(n)) else {
        return;
    };
    let since = episode.started;
    let mut said: Vec<(String, ActorId, u64)> = Vec::new();
    for (actor, memory) in &people {
        for (t, did) in &memory.0.done {
            if let Did::Talked(_, line) = did
                && *t >= since
                && def.lines.iter().any(|l| l.name == line.name)
            {
                said.push((line.name.clone(), actor.0, *t));
            }
        }
    }
    said.sort_by_key(|(.., t)| *t);
    for (pivot, who, t) in said {
        episode.reach(&pivot, who, t);
    }
}

/// How near the place an errand sends someone they have arrived, for the
/// pivot point reached on arrival, in metres.
const ARRIVED: f32 = 4.0;

/// How far from a thing an errand's spot beside it is, in metres.
const BESIDE: f32 = 2.0;

/// Once a game second, every part of the episode running is asked what its
/// first errand that holds asks (modforge `errand_for`, Skyrim's quest
/// stages with conditions), as `Asked` the brain weighs like any choice:
/// to stand beside a thing of the episode (two metres off at the errand's
/// side, on a spot they can stand on), back home, or to stay home; or to
/// lead someone beside a thing (ai::lead, the one led following until they
/// move themselves). Going somewhere or leading wins over a talk; staying
/// home never takes them out of one. With no errand holding, the episode
/// lets go only of its own asking. Arrived where an errand sends them, its
/// pivot point on arrival is reached.
pub fn errands(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    mut episode: ResMut<Episode>,
    episodes: Res<EpisodeRegistryRes>,
    nav: Res<crate::navmesh::Nav>,
    archipelagos: Query<&bevy_landmass::prelude::Archipelago3d>,
    player: Query<(Entity, &Actor), (With<crate::player::Player>, Without<Dead>)>,
    people: Query<(Entity, &Actor, &Transform, &Home, Has<crate::ai::Leading>, Has<crate::talk::HeldToTalk>), Without<Dead>>,
) {
    use modforge::storyteller::{ErrandAct, Who};
    if tick.0 % 60 != 0 {
        return;
    }
    let Some(def) = episode.name.as_deref().and_then(|n| episodes.0.def(n)) else {
        return;
    };
    let archipelago = archipelagos.get(nav.archipelago).ok();
    // The spot beside a thing of the episode at a side of it.
    let beside = |thing: &str, side: f32| {
        let (at, _) = episode.thing(thing)?;
        Some(archipelago.and_then(|a| crate::navmesh::stand_by(a, at, BESIDE, side)).unwrap_or(at))
    };
    let player_id = player.single().ok().map(|(e, a)| (a.0, e));
    let someone = |who: &Who| match who {
        Who::ThePlayer => player_id,
        Who::Part(part) => episode.cast_as(part).and_then(|id| people.iter().find(|p| p.1.0 == id)).map(|p| (p.1.0, p.0)),
    };
    let reached = episode.reached_names();
    let reached: Vec<&str> = reached.iter().map(String::as_str).collect();
    let mut arrived: Vec<(String, ActorId)> = Vec::new();
    for (part, id) in episode.cast.clone() {
        let Some((entity, _, at, home, leading, held_to_talk)) = people.iter().find(|p| p.1.0 == id) else {
            continue;
        };
        let errand = modforge::storyteller::errand_for(def, &part, &reached);
        let (to, wins_over_a_talk) = match errand.map(|e| &e.does) {
            Some(ErrandAct::GoTo { thing, side }) => (beside(thing, *side), true),
            Some(ErrandAct::GoHome) => (Some(home.0), true),
            Some(ErrandAct::StayHome) => (Some(home.0), false),
            Some(ErrandAct::Lead { who, thing, side }) => {
                if !leading
                    && let (Some(to), Some((who, follower))) = (beside(thing, *side), someone(who))
                {
                    commands.entity(entity).insert(crate::ai::Leading { who, to, waiting_at: None });
                    commands.entity(follower).insert(crate::ai::Following(id));
                }
                continue;
            }
            None => (None, false),
        };
        if let (Some(e), Some(to)) = (errand, to)
            && let Some(pivot) = &e.reached_on_arrival
            && crate::interact::feet_of(at).distance(to) <= ARRIVED
        {
            arrived.push((pivot.clone(), id));
        }
        match to {
            // Going somewhere wins over a talk: it is no longer the
            // conversation's to let go.
            Some(to) if wins_over_a_talk => {
                commands.entity(entity).insert(crate::actor::Asked(to)).remove::<crate::talk::HeldToTalk>();
            }
            Some(to) if !held_to_talk => {
                commands.entity(entity).insert(crate::actor::Asked(to));
            }
            Some(_) => {}
            // Nothing asked: the episode lets go only of its own asking,
            // never of the talk holding them (talk::hold).
            None if !held_to_talk => {
                commands.entity(entity).remove::<crate::actor::Asked>();
            }
            None => {}
        }
    }
    for (pivot, who) in arrived {
        episode.reach(&pivot, who, tick.0);
    }
}

/// How far from the bunker point, and how far below the ground, the player
/// is home.
const HOME_WITHIN: f32 = 20.0;
const BELOW_GROUND: f32 = -0.5;

/// Once a game second, every deed pivot point of the episode running
/// (modforge `DeedDef`, todo 11ai) is read from memory and the world and
/// reached once it holds: of a group, the first in order that holds is
/// reached and none of the group after it. Anyone's eating from a thing is
/// reached by whoever did it first, at their tick; every other deed is the
/// player's, now.
#[allow(clippy::type_complexity)]
pub fn deeds(
    tick: Res<crate::SimTick>,
    mut episode: ResMut<Episode>,
    episodes: Res<EpisodeRegistryRes>,
    world: Res<crate::terrain::WorldRes>,
    player: Query<(&Actor, &Transform, &Memory, &crate::interact::OpenContainer), With<crate::player::Player>>,
    people: Query<(&Actor, Option<&Npc>, Option<&crate::ai::Following>, Option<&crate::ai::Leading>, Option<&Store>), Without<Dead>>,
    memories: Query<(&Actor, &Memory)>,
    stores: Query<(&crate::interact::Container, Option<&crate::interact::Drawn>)>,
    items: Res<ItemRegistryRes>,
    day: Res<crate::survival::Day>,
    (storms, things): (Res<crate::storm::StormRes>, Res<crate::interact::ThingIds>),
) {
    use modforge::storyteller::{Company, Deed, Told};
    if tick.0 % 60 != 0 {
        return;
    }
    let Some(def) = episode.name.as_deref().and_then(|n| episodes.0.def(n)) else {
        return;
    };
    let Ok((me, at, memory, open)) = player.single() else {
        return;
    };
    let feet = crate::interact::feet_of(at);
    let started = episode.started;
    let of_bunker = |id: ActorId| people.iter().any(|(a, npc, ..)| a.0 == id && npc.and_then(|n| n.faction.as_deref()) == Some(PLAYER_FACTION));
    // Whoever follows the player, or leads them.
    let with: Vec<ActorId> = people
        .iter()
        .filter(|(_, _, following, leading, _)| following.is_some_and(|f| f.0 == me.0) || leading.is_some_and(|l| l.who == me.0))
        .map(|(a, ..)| a.0)
        .collect();
    // Someone ate or drank from a thing of the episode since it started:
    // its item, as memory records eating it.
    let item_of = |thing: &str| def.things.iter().find(|t| t.name == thing).map_or(thing.to_string(), |t| t.item.clone());
    let ate = |memory: &modforge::memory::Memory, item: &str| memory.done.iter().find(|(t, d)| *t >= started && matches!(d, Did::Ate(i) if i == item)).map(|(t, _)| *t);
    // The store the player has open, what it holds, and whether something
    // draws from it.
    let checked = open.0.and_then(|e| stores.get(e).ok().map(|(held, drawn)| (e, held, drawn)));
    let storm = crate::storm::phase(&tick, *day, &storms);
    let holds = |deed: &Deed, episode: &Episode| -> Option<(ActorId, u64)> {
        let now = Some((me.0, tick.0));
        match deed {
            Deed::Ate { thing } => {
                let item = item_of(thing);
                memories.iter().filter_map(|(a, m)| ate(&m.0, &item).map(|t| (a.0, t))).min_by_key(|(_, t)| *t)
            }
            Deed::CameNear { thing, within, with: company } => {
                let (there, _) = episode.thing(thing)?;
                if feet.distance(there) > *within {
                    return None;
                }
                let holds = match company {
                    Company::Part(part) => episode.cast_as(part).is_some_and(|id| with.contains(&id)),
                    Company::BunkerMember => with.iter().any(|id| of_bunker(*id)),
                    Company::NoOne => true,
                };
                holds.then_some(now?)
            }
            Deed::HomeAfterAte { thing, told } => {
                let (_, key) = episode.thing(thing)?;
                let home = feet.y < BELOW_GROUND && feet.ground().distance(world.bunker) <= HOME_WITHIN;
                ate(&memory.0, &item_of(thing))?;
                if !home {
                    return None;
                }
                // Whom they told of it (the talk path's Tell), by the kind
                // they know it as.
                let kind = memory.0.known.iter().find(|k| k.key == key).map_or(item_of(thing), |k| k.kind.clone());
                let told_of = format!("told of the {kind}");
                let was_told = |id: ActorId| memory.0.done.iter().any(|(t, d)| *t >= started && matches!(d, Did::Talked(to, line) if *to == id && line.words == told_of));
                let holds = match told {
                    Told::AllOfTheirBunker => {
                        let bunker: Vec<ActorId> = people.iter().filter(|(a, ..)| of_bunker(a.0)).map(|(a, ..)| a.0).collect();
                        !bunker.is_empty() && bunker.iter().all(|id| was_told(*id))
                    }
                    Told::Part(part) => episode.cast_as(part).is_some_and(was_told),
                    Told::NoOne => true,
                };
                holds.then_some(now?)
            }
            Deed::GoneInTheStorm { thing } => {
                let (_, key) = episode.thing(thing)?;
                let storming = matches!(storm, modforge::storm::StormPhase::Warning | modforge::storm::StormPhase::Storm);
                (storming && things.get(key).is_none()).then_some(now?)
            }
            Deed::OpenedDrawnStoreDry => {
                let (_, held, drawn) = checked?;
                drawn.is_some_and(|d| d.dry(crate::interact::water_in(&held.0, &items))).then_some(now?)
            }
            Deed::OpenedOwnStoreOver { need, days } => {
                let (store, held, _) = checked?;
                let ours = people.iter().any(|(a, _, _, _, s)| of_bunker(a.0) && s.is_some_and(|s| s.0 == store));
                // The player's bunker's living people: the player and its own.
                let living = 1 + people.iter().filter(|(a, ..)| of_bunker(a.0)).count() as u32;
                (ours && crate::interact::days_of(&held.0, *need, &items, *day, living) > *days).then_some(now?)
            }
        }
    };
    let mut reached: Vec<(String, ActorId, u64)> = Vec::new();
    let mut groups_done: Vec<&str> = Vec::new();
    for d in &def.deeds {
        if episode.reached(&d.pivot) || reached.iter().any(|(p, ..)| *p == d.pivot) {
            continue;
        }
        // A group is reached once, by its first deed that holds.
        if let Some(group) = d.group.as_deref() {
            let group_reached = def.deeds.iter().any(|o| o.group.as_deref() == Some(group) && (episode.reached(&o.pivot) || reached.iter().any(|(p, ..)| *p == o.pivot)));
            if group_reached || groups_done.contains(&group) {
                continue;
            }
        }
        if let Some((who, at)) = holds(&d.deed, &episode) {
            reached.push((d.pivot.clone(), who, at));
            if let Some(group) = d.group.as_deref() {
                groups_done.push(group);
            }
        }
    }
    for (pivot, who, at) in reached {
        episode.reach(&pivot, who, at);
    }
}

/// When the storm comes, the episode's ending is shown, once, as a text
/// screen before the world shifts (the-tap.md "Which ending"; modforge
/// `storyteller::ending`: the first whose pivot points are reached and
/// whose parts are alive).
pub fn the_ending(
    tick: Res<crate::SimTick>,
    day: Res<crate::survival::Day>,
    storms: Res<crate::storm::StormRes>,
    episode: Res<Episode>,
    episodes: Res<EpisodeRegistryRes>,
    mut players: Query<&mut crate::interact::Hud, With<crate::player::Player>>,
    people: Query<(&Actor, Has<Dead>)>,
) {
    if crate::storm::phase(&tick, *day, &storms) != modforge::storm::StormPhase::Storm {
        return;
    }
    let (Ok(mut hud), Some(def)) = (players.single_mut(), episode.name.as_deref().and_then(|n| episodes.0.def(n))) else {
        return;
    };
    if hud.0.ending.is_some() {
        return;
    }
    let reached = episode.reached_names();
    let reached: Vec<&str> = reached.iter().map(String::as_str).collect();
    let alive = |part: &str| episode.cast_as(part).is_some_and(|id| people.iter().any(|(a, dead)| a.0 == id && !dead));
    if let Some(end) = modforge::storyteller::ending(def, &reached, alive) {
        info!("{:?}: the ending is {}", episode.name, end.name);
        hud.0.ending = Some(modforge::hud::Ending { name: end.name.clone(), words: end.words.clone() });
    }
}
