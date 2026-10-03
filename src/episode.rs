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
    /// Each errand of the def (by its place in the def's list) and the tick
    /// its condition first held: when an errand starting the first morning
    /// after starts.
    #[serde(default)]
    pub held_since: Vec<(usize, u64)>,
    /// Each named errand's window: its name, when it started, when it ended
    /// (what `Since::During` and `Cond::Ended` read).
    #[serde(default)]
    pub windows: Vec<(String, u64, Option<u64>)>,
    /// Each errand's order already given to a person (an order is given
    /// once, through the one command path).
    #[serde(default)]
    pub ordered: Vec<(usize, ActorId)>,
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
            held_since: Vec::new(),
            windows: Vec::new(),
            ordered: Vec::new(),
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

/// One person as an episode's conditions read them.
struct Someone<'a> {
    id: ActorId,
    entity: Entity,
    alive: bool,
    feet: Vec3,
    memory: Option<&'a modforge::memory::Memory>,
    faction: Option<String>,
    player: bool,
    came_for: Option<u64>,
    /// Whom they follow or lead.
    with: Vec<ActorId>,
    holding: Option<Vec3>,
    home: Option<Vec3>,
    open: Option<modforge::storyteller::OpenStore>,
}

/// Everything an episode's conditions read, gathered once from the world
/// (modforge `storyteller::Facts`): the one place the world is read for
/// them.
struct EpisodeFacts<'a> {
    now: u64,
    player: ActorId,
    episode: &'a Episode,
    def: &'a modforge::storyteller::EpisodeDef,
    people: Vec<Someone<'a>>,
    storm: modforge::storm::StormPhase,
    bunker: Vec2,
    /// The episode's things gone from the world.
    gone: Vec<String>,
}

/// How near the place an errand sends someone they have arrived, and how
/// near a thing someone holding a place holds it, in metres.
const ARRIVED: f32 = 4.0;

/// How far from the bunker point, and how far below the ground, the player
/// is home.
const HOME_WITHIN: f32 = 20.0;
const BELOW_GROUND: f32 = -0.5;

impl EpisodeFacts<'_> {
    fn someone(&self, id: ActorId) -> Option<&Someone<'_>> {
        self.people.iter().find(|p| p.id == id)
    }

    /// The faction a group is: a part's, or the player's bunker's.
    fn faction_of(&self, group: &modforge::storyteller::Group) -> Option<String> {
        use modforge::storyteller::Group;
        match group {
            Group::BunkerOf(part) => self.episode.cast_as(part).and_then(|id| self.someone(id)).and_then(|p| p.faction.clone()),
            Group::ThePlayersBunker => Some(PLAYER_FACTION.to_string()),
            Group::CameFor(_) => None,
        }
    }
}

impl modforge::storyteller::Facts for EpisodeFacts<'_> {
    fn now(&self) -> u64 {
        self.now
    }
    fn player(&self) -> ActorId {
        self.player
    }
    fn episode_started(&self) -> u64 {
        self.episode.started
    }
    fn reached_at(&self, pivot: &str) -> Option<u64> {
        self.episode.reached.iter().find(|(p, ..)| p == pivot).map(|(.., t)| *t)
    }
    fn window(&self, errand: &str) -> Option<(u64, Option<u64>)> {
        self.episode.windows.iter().find(|(e, ..)| e == errand).map(|(_, from, to)| (*from, *to))
    }
    fn members(&self, who: &modforge::storyteller::Who) -> Vec<ActorId> {
        use modforge::storyteller::{Group, Who};
        match who {
            Who::ThePlayer => vec![self.player],
            Who::Part(part) => self.episode.cast_as(part).into_iter().collect(),
            Who::Anyone => self.people.iter().map(|p| p.id).collect(),
            Who::Group(Group::CameFor(thing)) => {
                let key = self.episode.thing(thing).map(|(_, k)| k);
                self.people.iter().filter(|p| key.is_some() && p.came_for == key).map(|p| p.id).collect()
            }
            // A bunker's people: its faction's, and the player in theirs.
            Who::Group(group) => {
                let faction = self.faction_of(group);
                self.people.iter().filter(|p| faction.is_some() && (p.faction == faction || (p.player && faction.as_deref() == Some(PLAYER_FACTION)))).map(|p| p.id).collect()
            }
        }
    }
    fn alive(&self, id: ActorId) -> bool {
        self.someone(id).is_some_and(|p| p.alive)
    }
    fn memory(&self, id: ActorId) -> Option<&modforge::memory::Memory> {
        self.someone(id).and_then(|p| p.memory)
    }
    fn feet(&self, id: ActorId) -> Option<Vec3> {
        self.someone(id).map(|p| p.feet)
    }
    fn thing_at(&self, thing: &str) -> Option<Vec3> {
        // In the world: placed by the episode and not gone since.
        self.episode.thing(thing).map(|(at, _)| at).filter(|_| !self.gone(thing))
    }
    fn item_of(&self, thing: &str) -> String {
        self.def.things.iter().find(|t| t.name == thing).map_or(thing.to_string(), |t| t.item.clone())
    }
    fn told_words(&self, by: ActorId, thing: &str) -> String {
        // By the kind they know it as (the talk path's Tell).
        let key = self.episode.thing(thing).map(|(_, k)| k);
        let kind = self.memory(by).and_then(|m| m.known.iter().find(|k| Some(k.key) == key)).map_or(self.item_of(thing), |k| k.kind.clone());
        format!("told of the {kind}")
    }
    fn with(&self, a: ActorId, b: ActorId) -> bool {
        self.someone(a).is_some_and(|p| p.with.contains(&b))
    }
    fn holds_at(&self, id: ActorId, thing: &str) -> bool {
        let at = self.episode.thing(thing).map(|(at, _)| at);
        self.someone(id).and_then(|p| p.holding).zip(at).is_some_and(|(held, at)| held.distance(at) <= ARRIVED)
    }
    fn at_home(&self, id: ActorId) -> bool {
        self.someone(id).is_some_and(|p| match p.home {
            Some(home) => home.distance(p.feet) <= crate::ai::HOME_REACH,
            // The player: in their bunker, below the ground.
            None => p.feet.y < BELOW_GROUND && p.feet.ground().distance(self.bunker) <= HOME_WITHIN,
        })
    }
    fn storm(&self) -> modforge::storm::StormPhase {
        self.storm
    }
    fn opened(&self, id: ActorId) -> Option<modforge::storyteller::OpenStore> {
        self.someone(id).and_then(|p| p.open)
    }
}

impl EpisodeFacts<'_> {
    /// Whether the episode's thing is gone from the world.
    fn gone(&self, thing: &str) -> bool {
        self.gone.iter().any(|g| g == thing)
    }
}

/// What the episode's systems read the world through, once a game second.
#[derive(bevy::ecs::system::SystemParam)]
pub struct EpisodeWorld<'w, 's> {
    tick: Res<'w, crate::SimTick>,
    episodes: Res<'w, EpisodeRegistryRes>,
    world: Res<'w, crate::terrain::WorldRes>,
    items: Res<'w, ItemRegistryRes>,
    day: Res<'w, crate::survival::Day>,
    storms: Res<'w, crate::storm::StormRes>,
    things: Res<'w, crate::interact::ThingIds>,
    people: Query<
        'w,
        's,
        (
            (Entity, &'static Actor, &'static Transform, &'static Memory, Option<&'static Npc>, Has<Dead>, Has<crate::player::Player>),
            (
                Option<&'static crate::ai::Following>,
                Option<&'static crate::ai::Leading>,
                Option<&'static crate::ai::Holding>,
                Option<&'static Home>,
                Option<&'static Store>,
                Option<&'static crate::interact::OpenContainer>,
                Option<&'static crate::edge::CameFor>,
            ),
        ),
    >,
    stores: Query<'w, 's, (&'static crate::interact::Container, Option<&'static crate::interact::Drawn>)>,
}

impl EpisodeWorld<'_, '_> {
    /// The facts of the episode running now, if one is.
    fn facts<'a>(&'a self, episode: &'a Episode) -> Option<EpisodeFacts<'a>> {
        let def = episode.name.as_deref().and_then(|n| self.episodes.0.def(n))?;
        let all: Vec<_> = self.people.iter().collect();
        let player = all.iter().find(|((.., is_player), _)| *is_player).map(|((_, a, ..), _)| a.0)?;
        // Who leads or follows whom, both ways.
        let pairs: Vec<(ActorId, ActorId)> = all
            .iter()
            .flat_map(|((_, a, ..), (following, leading, ..))| following.map(|f| (a.0, f.0)).into_iter().chain(leading.map(|l| (a.0, l.who))))
            .collect();
        let faction_of = |npc: Option<&Npc>, is_player: bool| if is_player { Some(PLAYER_FACTION.to_string()) } else { npc.and_then(|n| n.faction.clone()) };
        let living_of = |faction: &Option<String>| all.iter().filter(|((_, _, _, _, npc, dead, is_player), _)| !dead && faction_of(*npc, *is_player) == *faction).count() as u32;
        let store_owner = |store: Entity| all.iter().find(|(_, (.., s, _, _))| s.is_some_and(|s| s.0 == store)).map(|((_, _, _, _, npc, _, is_player), _)| faction_of(*npc, *is_player));
        let people = all
            .iter()
            .map(|((entity, actor, at, memory, npc, dead, is_player), (_, _, holding, home, _, open, came_for))| {
                let faction = faction_of(*npc, *is_player);
                let open = open.and_then(|o| o.0).and_then(|e| self.stores.get(e).ok().map(|s| (e, s))).map(|(store, (held, drawn))| {
                    let owner = store_owner(store).flatten();
                    let living = living_of(&owner).max(1);
                    modforge::storyteller::OpenStore {
                        theirs: owner.is_some() && owner == faction,
                        drawn_from: drawn.is_some(),
                        dry: drawn.is_some_and(|d| d.dry(crate::interact::water_in(&held.0, &self.items))),
                        days: (
                            crate::interact::days_of(&held.0, modforge::survival::Need::Hunger, &self.items, *self.day, living),
                            crate::interact::days_of(&held.0, modforge::survival::Need::Thirst, &self.items, *self.day, living),
                        ),
                    }
                });
                Someone {
                    id: actor.0,
                    entity: *entity,
                    alive: !dead,
                    feet: crate::interact::feet_of(at),
                    memory: Some(&memory.0),
                    faction,
                    player: *is_player,
                    came_for: came_for.map(|c| c.0),
                    with: pairs.iter().filter(|(a, _)| *a == actor.0).map(|(_, b)| *b).chain(pairs.iter().filter(|(_, b)| *b == actor.0).map(|(a, _)| *a)).collect(),
                    holding: holding.map(|h| h.at),
                    home: home.map(|h| h.0),
                    open,
                }
            })
            .collect();
        // The episode's things no longer in the world (by their ThingId).
        let gone = episode.things.iter().filter(|(_, _, key)| *key != 0 && self.things.get(*key).is_none()).map(|(n, ..)| n.clone()).collect();
        Some(EpisodeFacts {
            now: self.tick.0,
            player,
            episode,
            def,
            people,
            storm: crate::storm::phase(&self.tick, *self.day, &self.storms),
            bunker: self.world.bunker,
            gone,
        })
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
            _ => None,
        };
        let at = |r: modforge::storyteller::ThingRef| episode.thing(&r.thing).map(|(at, _)| modforge::command::Point::from(at));
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

/// How far from a thing an errand's spot beside it is, in metres.
const BESIDE: f32 = 2.0;

/// Once a game second, the episode's errands (modforge `ErrandDef`, Skyrim's
/// quest stages giving packages to aliases): each errand whose condition
/// holds starts, now or the first game morning after (a named one opening
/// its window and reaching its name as a pivot point, its window closing
/// when its end holds); each person it names (a part, a group's each
/// member) does the first started errand naming them whose condition holds,
/// as `Asked` the brain weighs like any choice: to stand beside a thing of
/// the episode, back home, or to stay home; to lead someone beside a thing;
/// or an order through the one command path, given once. Going somewhere,
/// leading or an order wins over a talk; staying home never takes them out
/// of one. With no errand holding, the episode lets go only of its own
/// asking. Arrived where an errand sends them, its pivot point on arrival
/// is reached.
#[allow(clippy::too_many_arguments)]
pub fn errands(
    mut commands: Commands,
    mut episode: ResMut<Episode>,
    world: EpisodeWorld,
    nav: Res<crate::navmesh::Nav>,
    archipelagos: Query<&bevy_landmass::prelude::Archipelago3d>,
    talking: Query<(&Actor, Has<crate::ai::Leading>, Has<crate::talk::HeldToTalk>)>,
    mut ordered_people: Query<(Entity, &Actor, Option<&mut crate::command::Commanded>)>,
) {
    use modforge::storyteller::{ErrandAct, Facts, Who, holds, starts_at};
    let now = world.tick.0;
    if now % 60 != 0 {
        return;
    }
    let day = (world.day.0 * 60.0) as u64;
    let archipelago = archipelagos.get(nav.archipelago).ok();
    // What to do, worked out from the facts; done after, as the facts
    // borrow the episode.
    enum Asks {
        Go(Entity, Vec3, bool),
        LetGo(Entity),
        Lead(Entity, ActorId, Vec3, Entity, ActorId),
        Order(usize, ActorId, modforge::command::Command),
    }
    let mut asks: Vec<Asks> = Vec::new();
    let mut arrived: Vec<(String, ActorId)> = Vec::new();
    let mut held_now: Vec<(usize, u64)> = Vec::new();
    let mut opens: Vec<String> = Vec::new();
    let mut ends: Vec<String> = Vec::new();
    {
        let Some(facts) = world.facts(&episode) else {
            return;
        };
        let def = facts.def;
        // Each errand: when its condition first held, and whether it has
        // started (and not ended).
        for (i, e) in def.errands.iter().enumerate() {
            if !episode.held_since.iter().any(|(j, _)| *j == i) && holds(&e.when, &facts).is_some() {
                held_now.push((i, now));
            }
        }
        let held = |i: usize| episode.held_since.iter().chain(&held_now).find(|(j, _)| *j == i).map(|(_, t)| *t);
        let started = |e: &modforge::storyteller::ErrandDef| {
            let i = def.errands.iter().position(|o| std::ptr::eq(o, e)).unwrap_or(0);
            let ended = e.name.as_ref().is_some_and(|n| episode.windows.iter().any(|(w, _, end)| w == n && end.is_some()));
            !ended && held(i).is_some_and(|t| now >= starts_at(e.at, t, day))
        };
        for e in def.errands.iter().filter(|e| started(e) && holds(&e.when, &facts).is_some()) {
            if let Some(name) = &e.name {
                match episode.windows.iter().find(|(w, ..)| w == name) {
                    None => opens.push(name.clone()),
                    Some((_, _, None)) if e.ends.as_ref().is_some_and(|c| holds(c, &facts).is_some()) => ends.push(name.clone()),
                    _ => {}
                }
            }
        }
        // The spot beside a thing of the episode at a side of it.
        let beside = |thing: &str, side: f32| {
            let at = facts.thing_at(thing)?;
            Some(archipelago.and_then(|a| crate::navmesh::stand_by(a, at, BESIDE, side)).unwrap_or(at))
        };
        // Everyone some errand names.
        let named: Vec<ActorId> = {
            let mut ids: Vec<ActorId> = def.errands.iter().flat_map(|e| facts.members(&e.who)).collect();
            ids.sort_by_key(|id| id.0);
            ids.dedup();
            ids
        };
        for id in named {
            let Some(person) = facts.people.iter().find(|p| p.id == id && p.alive) else {
                continue;
            };
            let (leading, held_to_talk) = talking.iter().find(|(a, ..)| a.0 == id).map_or((false, false), |(_, l, h)| (l, h));
            let errand = modforge::storyteller::errand_for(def, |w| facts.members(w).contains(&id), &facts, started);
            let home = person.home.unwrap_or(person.feet);
            let (to, wins_over_a_talk) = match errand.map(|e| &e.does) {
                Some(ErrandAct::GoTo { thing, side }) => (beside(thing, *side), true),
                Some(ErrandAct::GoHome) => (Some(home), true),
                Some(ErrandAct::StayHome) => (Some(home), false),
                Some(ErrandAct::Lead { who, thing, side }) => {
                    let follower = facts.members(who).into_iter().find_map(|f| facts.people.iter().find(|p| p.id == f && p.alive));
                    if !leading
                        && let (Some(to), Some(follower)) = (beside(thing, *side), follower)
                    {
                        asks.push(Asks::Lead(person.entity, follower.id, to, follower.entity, id));
                    }
                    continue;
                }
                Some(ErrandAct::Order(order)) => {
                    let i = def.errands.iter().position(|o| errand.is_some_and(|e| std::ptr::eq(o, e))).unwrap_or(0);
                    if episode.ordered.iter().any(|(j, who)| *j == i && *who == id) {
                        continue;
                    }
                    // Whom it names, the nearest living one to them; where,
                    // the thing where it stands.
                    let whom = |w: Who| {
                        facts
                            .members(&w)
                            .into_iter()
                            .filter(|m| *m != id && facts.alive(*m))
                            .filter_map(|m| facts.feet(m).map(|f| (m, f.distance(person.feet))))
                            .min_by(|a, b| a.1.total_cmp(&b.1))
                            .map(|(m, _)| m.0)
                    };
                    let at = |r: modforge::storyteller::ThingRef| beside(&r.thing, 0.0).map(modforge::command::Point::from);
                    match order.clone().resolve(whom, at) {
                        Some(given) => asks.push(Asks::Order(i, id, given)),
                        None => warn!("errand {:?}: its order {order:?} names no one or nothing there for {id:?}", errand.and_then(|e| e.name.clone())),
                    }
                    continue;
                }
                None => (None, false),
            };
            if let (Some(e), Some(to)) = (errand, to)
                && let Some(pivot) = &e.reached_on_arrival
                && person.feet.distance(to) <= ARRIVED
            {
                arrived.push((pivot.clone(), id));
            }
            match to {
                Some(to) => asks.push(Asks::Go(person.entity, to, wins_over_a_talk || !held_to_talk)),
                // Nothing asked: the episode lets go only of its own
                // asking, never of the talk holding them (talk::hold).
                None if !held_to_talk => asks.push(Asks::LetGo(person.entity)),
                None => {}
            }
        }
    }
    episode.held_since.extend(held_now);
    for name in opens {
        let who = episode.reached.first().map_or(ActorId(0), |(_, w, _)| *w);
        episode.windows.push((name.clone(), now, None));
        episode.reach(&name, who, now);
    }
    for name in ends {
        if let Some(w) = episode.windows.iter_mut().find(|(w, ..)| *w == name) {
            w.2 = Some(now);
        }
    }
    let mut new = std::collections::HashMap::new();
    for ask in asks {
        match ask {
            // Going somewhere wins over a talk: it is no longer the
            // conversation's to let go. Staying home asks only one not held.
            Asks::Go(entity, to, wins) if wins => {
                commands.entity(entity).insert(crate::actor::Asked(to)).remove::<crate::talk::HeldToTalk>();
            }
            Asks::Go(..) => {}
            Asks::LetGo(entity) => {
                commands.entity(entity).remove::<crate::actor::Asked>();
            }
            Asks::Lead(leader, who, to, follower, leader_id) => {
                commands.entity(leader).insert(crate::ai::Leading { who, to, waiting_at: None });
                commands.entity(follower).insert(crate::ai::Following(leader_id));
            }
            Asks::Order(i, who, given) => {
                crate::command::order(&mut ordered_people, &mut new, who.0, given);
                episode.ordered.push((i, who));
            }
        }
    }
    for (entity, left) in new {
        commands.entity(entity).insert(crate::command::Commanded { left, started: false });
    }
    for (pivot, who) in arrived {
        episode.reach(&pivot, who, now);
    }
}

/// Once a game second, every deed pivot point of the episode running
/// (modforge `DeedDef`, todo 11ai) whose condition holds (modforge `holds`,
/// the one condition language, over the facts read once) is reached, in the
/// def's order, each seeing the ones reached before it.
pub fn deeds(mut episode: ResMut<Episode>, world: EpisodeWorld) {
    if world.tick.0 % 60 != 0 {
        return;
    }
    let count = match world.facts(&episode) {
        Some(facts) => facts.def.deeds.len(),
        None => return,
    };
    for i in 0..count {
        let found = {
            let Some(facts) = world.facts(&episode) else {
                return;
            };
            let deed = &facts.def.deeds[i];
            if episode.reached(&deed.pivot) {
                continue;
            }
            modforge::storyteller::holds(&deed.when, &facts).map(|(who, at)| (deed.pivot.clone(), who, at))
        };
        if let Some((pivot, who, at)) = found {
            episode.reach(&pivot, who, at);
        }
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
