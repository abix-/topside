//! The episode running now (episodes.md): started by the storm after the
//! reality, its parts cast from the bunkers' people once they exist.

use bevy::prelude::*;
use modforge::actor::ActorId;
use modforge::storyteller::{CastBunker, cast};

use modforge::item;
use modforge::memory::Did;

use crate::Ground;
use crate::actor::{Actor, Dead, Home, Memory, Npc, Store};
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
const ON_THE_WAY: f32 = 80.0;

/// The mark a site's key carries, apart from things' keys.
pub const SITE: u64 = 1 << 62;

/// The key memory knows a site by: its position, marked a place (the
/// brain visits places) and a site, apart from things. The same site has
/// the same key in every world rolled from the same seed, whatever order
/// its chunks were made in.
pub fn site_key(at: Vec2) -> u64 {
    crate::actor::PLACE_KEY | SITE | ((at.x.round() as i32 as u32 as u64) << 24) ^ (at.y.round() as i32 as u32 as u64)
}

/// Where the player's bunker door stands on the ground (the starting
/// bunker's one door, `structure::doors_of`), and which way it faces out.
pub fn bunker_door(bunker: Vec2) -> (Vec2, Vec2) {
    // The front door: of the bunker's door blocks on the ground (modforge
    // `door_blocks`, what its doors are built from), the one with a side
    // outside every room on the ground, a step through its wall; a door
    // between two rooms has a room on both sides. It faces that way out.
    let plan = crate::content::starting_bunker();
    let in_a_room = |p: Vec2| {
        plan.rooms.iter().any(|r| {
            r.origin.y <= 0.01 && r.origin.y + r.interior.y > 0.01 && (p.x - r.origin.x).abs() < r.interior.x / 2.0 && (p.y - r.origin.z).abs() < r.interior.z / 2.0
        })
    };
    let front = modforge::structure::door_blocks(&plan).into_iter().filter(|b| b.floor.abs() < 0.01).find_map(|b| {
        let through = b.along.perp();
        [through, -through].into_iter().find(|out| !in_a_room(b.centre + *out)).map(|out| (b.centre, out))
    });
    let (door, out) = front.unwrap_or((Vec2::ZERO, Vec2::Y));
    (bunker + door, out)
}

/// How far `at` stands from the way `from` to `to`, and how far along it
/// (0 at `from`, 1 at `to`).
fn off_the_way(from: Vec2, to: Vec2, at: Vec2) -> (f32, f32) {
    let way = to - from;
    let along = ((at - from).dot(way) / way.length_squared().max(1.0)).clamp(0.0, 1.0);
    (at.distance(from + way * along), along)
}

/// The Tap, once cast: the tap comes into the world through
/// `interact::spawn_item` inside a building of the site nearest the middle
/// of the way from the player's bunker door to Mara's bunker, its water
/// drawn from Mara's bunker's store. Dell found it: she knows the site and
/// every site on the way (docs/episodes/the-tap.md "How it opens").
#[allow(clippy::too_many_arguments)]
pub fn place_the_tap(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    mut episode: ResMut<Episode>,
    items: Res<ItemRegistryRes>,
    mut world: ResMut<crate::terrain::WorldRes>,
    registries: (Res<crate::content::BiomeRegistryRes>, Res<crate::content::MonumentRegistryRes>),
    people: Query<(&Actor, &Home, &Store)>,
    mut memories: Query<(&Actor, &mut Memory)>,
    mut ids: ResMut<crate::interact::ThingIssuer>,
    stores: Query<&crate::interact::Container>,
) {
    if episode.name.as_deref() != Some("The Tap") || !episode.cast_done || !episode.things.is_empty() {
        return;
    }
    let (Some(mara), Some(def)) = (episode.cast_as("Mara"), items.0.def("tap")) else {
        return;
    };
    let Some((_, home, store)) = people.iter().find(|(a, ..)| a.0 == mara) else {
        return;
    };
    let (door, _) = bunker_door(world.bunker);
    // Sites stand on the ground, so the way between bunkers is read there.
    let mara_home = home.0.ground();
    let (biomes, monuments) = registries;
    let r = modforge::world2d::Registries {
        biomes: &biomes.0,
        monuments: &monuments.monuments,
        buildings: &monuments.buildings,
    };
    // Anywhere between the bunkers: as far out as half the way is long.
    let margin = Vec2::splat((door.distance(mara_home) / 2.0).max(ON_THE_WAY));
    let sites: Vec<modforge::places::Place> = world
        .world
        .sites_in(r, door.min(mara_home) - margin, door.max(mara_home) + margin)
        .into_iter()
        .filter(|p| !p.rolled.members.is_empty())
        .collect();
    let middle = (door + mara_home) / 2.0;
    let Some(site) = sites.iter().min_by(|a, b| a.position.distance(middle).total_cmp(&b.position.distance(middle))) else {
        warn!("The Tap: no building between the bunkers to put the tap in");
        episode.things.push(("no tap".to_string(), crate::on_ground(middle, 0.0), 0));
        return;
    };
    let member = &site.rolled.members[0];
    let at = site.position + Vec2::new(member.offset.x, member.offset.z);
    // On the building's ground floor.
    let tap_at = crate::on_ground(at, 0.0);
    let (tap, id) = spawn_item(&mut commands, &mut ids, def, item::create(def, 1, &[], 0.0, 0), tap_at);
    commands.entity(tap).insert(DrawsFrom(store.0));
    // What Mara's store holds as the tap starts drawing from it.
    let held = stores.get(store.0).map_or(0, |held| crate::interact::water_in(&held.0, &items));
    commands.entity(store.0).insert(crate::interact::Drawn { held });
    let key = id.0;
    let ground = |p: Vec2| crate::on_ground(p, 0.0);
    if let Some(dell) = episode.cast_as("Dell")
        && let Some((_, mut memory)) = memories.iter_mut().find(|(a, _)| a.0 == dell)
    {
        memory.0.see(key, "tap", tap_at, tick.0);
        // Dell wants it kept secret (docs/episodes/the-tap.md "Pushed all
        // the way"): she never passes it on.
        memory.0.keep_quiet(key);
        for passed in sites.iter().filter(|p| off_the_way(door, at, p.position).0 <= ON_THE_WAY) {
            memory.0.see(site_key(passed.position), &passed.monument, ground(passed.position), tick.0);
        }
    }
    episode.things.push(("tap".to_string(), tap_at, key));
    episode.things.push(("tap site".to_string(), ground(site.position), site_key(site.position)));
    info!("the tap stands in the {} at {at}, drawing from Mara's bunker's store", site.monument);
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
        let order = match cue.does {
            modforge::storyteller::CueAct::KnockOnThePlayer => modforge::command::Command::Knock { who: player.0.0 },
        };
        crate::command::order(&mut people, &mut new, who.0, order);
        info!("cue {}: {} ({:?}) {:?}", cue.name, cue.part, who, cue.does);
        episode.reached.push((cue.name.clone(), who, tick.0));
    }
    for (entity, left) in new {
        commands.entity(entity).insert(crate::command::Commanded { left, started: false });
    }
}

/// Dell's way to the tap, from what Dell knows: the slots of "the way"
/// (episodes.md "The lines"). {way} out of the bunker door, the {place}
/// the tap stands in, and the site Dell knows nearest the way there as the
/// {landmark}, with the {side} it stands on.
pub fn the_way(memory: &modforge::memory::Memory, door: (Vec2, Vec2), tap: u64, tap_site: u64) -> Vec<(&'static str, String)> {
    let (door, out) = door;
    let ground = |p: Vec3| Vec2::new(p.x, p.z);
    let Some(tap_at) = memory.known.iter().find(|k| k.key == tap).map(|k| ground(k.position)) else {
        return Vec::new();
    };
    let to = tap_at - door;
    let mut slots = vec![("way", modforge::talk::way_out(out, to).to_string())];
    if let Some(place) = memory.known.iter().find(|k| k.key == tap_site) {
        slots.push(("place", place.kind.clone()));
    }
    let landmark = memory
        .known
        .iter()
        .filter(|k| k.key != tap_site && k.key & SITE != 0)
        .map(|k| (k, off_the_way(door, tap_at, ground(k.position))))
        .filter(|(_, (off, along))| *off <= ON_THE_WAY && *along > 0.0 && *along < 1.0)
        .min_by(|a, b| a.1.0.total_cmp(&b.1.0));
    if let Some((k, _)) = landmark {
        slots.push(("landmark", k.kind.clone()));
        slots.push(("side", modforge::talk::side_of(to, ground(k.position) - door).to_string()));
    }
    slots
}

/// Pivot points reached by what was said (episodes.md "One way for every
/// conversation"): once a game second, every line of the episode said by
/// anyone since it started, read from memory (the one record), is reached
/// under its name ("the opening", "believes Dell", "threatens Mara").
/// Pivot points reached by what was done are reached where it is seen
/// (`the_taps_errands`, `the_first_drink`).
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

/// Who The Tap asks to be where (docs/episodes/the-tap.md), once a game
/// second, as `Asked` the brain weighs like any choice: Mara to the tap
/// from the start (her bunker's water is going); Dell, once the player has
/// chosen, to the tap ("Show me.", or alone when not believed); sent to
/// look again, Dell goes to the tap and comes back home, and tells the way
/// again by her rule ("Dell told the way again", content).
pub fn the_taps_errands(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    mut episode: ResMut<Episode>,
    nav: Res<crate::navmesh::Nav>,
    archipelagos: Query<&bevy_landmass::prelude::Archipelago3d>,
    player: Query<(Entity, &Actor), (With<crate::player::Player>, Without<Dead>)>,
    people: Query<(Entity, &Actor, &Transform, &Home, Has<crate::ai::Leading>), Without<crate::player::Player>>,
) {
    if tick.0 % 60 != 0 || episode.name.as_deref() != Some("The Tap") {
        return;
    }
    let Some((tap, _)) = episode.thing("tap") else {
        return;
    };
    // Where each stands by the tap: two metres off, on a spot they can
    // stand on (the tap's building has walls), Mara on one side and Dell
    // on the other.
    let archipelago = archipelagos.get(nav.archipelago).ok();
    let by_tap = |from: f32| archipelago.and_then(|a| crate::navmesh::stand_by(a, tap, 2.0, from)).unwrap_or(tap);
    let mara_at = by_tap(0.0);
    let dell_at = by_tap(std::f32::consts::PI);
    // The episode's errand wins over stopping to talk: it is no longer
    // the conversation's to let go.
    let ask = |commands: &mut Commands, entity: Entity, at: Option<Vec3>| match at {
        Some(at) => {
            commands.entity(entity).insert(crate::actor::Asked(at)).remove::<crate::talk::HeldToTalk>();
        }
        None => {
            commands.entity(entity).remove::<crate::actor::Asked>();
        }
    };
    let mara = episode.cast_as("Mara");
    let dell = episode.cast_as("Dell");
    let sent = episode.reached("sends Dell");
    let goes = episode.reached("believes Dell") || episode.reached("does not believe it");
    let mut looked = episode.reached("Dell looked");
    let believed = episode.reached("believes Dell");
    let player_id = player.single().ok().map(|(e, a)| (a.0, e));
    for (entity, actor, at, home, leading) in &people {
        if Some(actor.0) == mara {
            ask(&mut commands, entity, Some(mara_at));
        } else if Some(actor.0) == dell {
            let here = crate::interact::feet_of(at);
            if sent && !looked && here.distance(tap) <= 4.0 {
                looked = true;
            }
            // Believed, she shows the player the way: she leads them to the
            // tap (ai::lead asks her each game second), and the player
            // follows her until they move themselves (ai::follow).
            if believed {
                if !leading && let Some((who, follower)) = player_id {
                    commands.entity(entity).insert(crate::ai::Leading { who, to: dell_at, waiting_at: None });
                    commands.entity(follower).insert(crate::ai::Following(actor.0));
                }
                continue;
            }
            let to = if goes || (sent && !looked) {
                Some(dell_at)
            } else if sent && looked && !episode.reached("Dell told the way again") {
                Some(home.0)
            } else {
                None
            };
            ask(&mut commands, entity, to);
        }
    }
    if looked && !episode.reached("Dell looked")
        && let Some(dell) = dell
    {
        episode.reach("Dell looked", dell, tick.0);
    }
}

/// The Tap's first pivot point after it opens: the first drink at the
/// tap, read from the memory of whoever drank (memory is the one record).
pub fn the_first_drink(tick: Res<crate::SimTick>, mut episode: ResMut<Episode>, people: Query<(&Actor, &Memory)>) {
    const FIRST_DRINK: &str = "the first drink";
    // Once a game second is soon enough to notice it.
    if tick.0 % 60 != 0
        || episode.name.as_deref() != Some("The Tap")
        || episode.things.is_empty()
        || episode.reached.iter().any(|(p, ..)| p == FIRST_DRINK)
    {
        return;
    }
    let first = people
        .iter()
        .filter_map(|(actor, memory)| {
            memory
                .0
                .done
                .iter()
                .find(|(_, did)| matches!(did, Did::Ate(item) if item == "tap"))
                .map(|(tick, _)| (actor.0, *tick))
        })
        .min_by_key(|(_, tick)| *tick);
    if let Some((who, tick)) = first {
        episode.reached.push((FIRST_DRINK.to_string(), who, tick));
        info!("The Tap: the first drink, by {who:?} at tick {tick}");
    }
}

/// How near the tap the player first comes for who they took (the-tap.md
/// "How pivot points 2 and 4 are reached"), in metres.
const AT_THE_TAP: f32 = 4.0;

/// How far from the bunker point, and how far below the ground, the player
/// is home.
const HOME_WITHIN: f32 = 20.0;
const BELOW_GROUND: f32 = -0.5;

/// The Tap's pivot points reached by what the player does, not by a line
/// (the-tap.md "How pivot points 2 and 4 are reached"), once a game second:
/// pivot point 2, who they took, the first time they come within 4 m of the
/// tap (Dell leading or following them is "takes Dell", another of their
/// bunker following is "takes someone else", no one is "takes no one");
/// pivot point 4, what they told their people, the first time they come
/// home after drinking at the tap (every person of their bunker told of the
/// tap is "tells everyone", only Dell "tells only Dell", no one "tells no
/// one").
#[allow(clippy::type_complexity)]
pub fn the_taps_deeds(
    tick: Res<crate::SimTick>,
    mut episode: ResMut<Episode>,
    world: Res<crate::terrain::WorldRes>,
    player: Query<(&Actor, &Transform, &Memory, &crate::interact::OpenContainer), With<crate::player::Player>>,
    people: Query<(&Actor, Option<&Npc>, Option<&crate::ai::Following>, Option<&crate::ai::Leading>, Option<&Store>), Without<Dead>>,
    stores: Query<(&crate::interact::Container, Option<&crate::interact::Drawn>)>,
    items: Res<ItemRegistryRes>,
    day: Res<crate::survival::Day>,
    (storms, taps): (Res<crate::storm::StormRes>, Query<(), With<DrawsFrom>>),
) {
    if tick.0 % 60 != 0 || episode.name.as_deref() != Some("The Tap") {
        return;
    }
    let (Ok((me, at, memory, open)), Some((tap, tap_key))) = (player.single(), episode.thing("tap")) else {
        return;
    };
    let feet = crate::interact::feet_of(at);
    let dell = episode.cast_as("Dell");
    let took = ["takes Dell", "takes someone else", "takes no one"];
    if !took.iter().any(|p| episode.reached(p)) && feet.distance(tap) <= AT_THE_TAP {
        // Whoever follows them, or is leading them.
        let with: Vec<ActorId> = people
            .iter()
            .filter(|(_, _, following, leading, _)| following.is_some_and(|f| f.0 == me.0) || leading.is_some_and(|l| l.who == me.0))
            .map(|(a, ..)| a.0)
            .collect();
        let of_bunker = |id: ActorId| people.iter().any(|(a, npc, ..)| a.0 == id && npc.and_then(|n| n.faction.as_deref()) == Some(PLAYER_FACTION));
        let pivot = if with.iter().any(|id| Some(*id) == dell) {
            took[0]
        } else if with.iter().any(|id| of_bunker(*id)) {
            took[1]
        } else {
            took[2]
        };
        episode.reach(pivot, me.0, tick.0);
    }
    let told = ["tells everyone", "tells only Dell", "tells no one"];
    let drank = memory.0.done.iter().any(|(t, d)| *t >= episode.started && matches!(d, Did::Ate(item) if item == "tap"));
    let home = feet.y < BELOW_GROUND && feet.ground().distance(world.bunker) <= HOME_WITHIN;
    if drank && home && !told.iter().any(|p| episode.reached(p)) {
        // Who of their bunker they told of the tap (the talk path's Tell).
        let tap_kind = memory.0.known.iter().find(|k| k.key == tap_key).map_or("tap".to_string(), |k| k.kind.clone());
        let told_of = format!("told of the {tap_kind}");
        let was_told = |id: ActorId| memory.0.done.iter().any(|(t, d)| *t >= episode.started && matches!(d, Did::Talked(to, line) if *to == id && line.words == told_of));
        let bunker: Vec<ActorId> = people.iter().filter(|(_, npc, ..)| npc.and_then(|n| n.faction.as_deref()) == Some(PLAYER_FACTION)).map(|(a, ..)| a.0).collect();
        let pivot = if !bunker.is_empty() && bunker.iter().all(|id| was_told(*id)) {
            told[0]
        } else if dell.is_some_and(was_told) {
            told[1]
        } else {
            told[2]
        };
        episode.reach(pivot, me.0, tick.0);
    }
    // Pivot point 10: the tap broken between the sky's warning of the storm
    // and the storm (interact: Use with a melee weapon in hand) is "breaks
    // it".
    let storm = crate::storm::phase(&tick, *day, &storms);
    if taps.is_empty() && matches!(storm, modforge::storm::StormPhase::Warning | modforge::storm::StormPhase::Storm) {
        episode.reach("breaks it", me.0, tick.0);
    }
    // Checking a store (Use on it opens it, the player's open box):
    // the player's own bunker's holding more than it needs is Dell's
    // storing noticed (pivot point 5; Dell's line and the answers follow
    // it); a store another place draws from that reads dry is "finds
    // Mara's well dry" (pivot point 7).
    let Some(checked) = open.0 else {
        return;
    };
    let Ok((held, drawn)) = stores.get(checked) else {
        return;
    };
    let water = crate::interact::water_in(&held.0, &items);
    if drawn.is_some_and(|d| d.dry(water)) {
        episode.reach(FINDS_THE_WELL_DRY, me.0, tick.0);
    }
    let ours = people.iter().any(|(_, npc, _, _, store)| npc.and_then(|n| n.faction.as_deref()) == Some(PLAYER_FACTION) && store.is_some_and(|s| s.0 == checked));
    // The player's bunker's living people: the player and its own.
    let ours_living = 1 + people.iter().filter(|(_, npc, ..)| npc.and_then(|n| n.faction.as_deref()) == Some(PLAYER_FACTION)).count() as u32;
    if ours && crate::interact::days_of_water(&held.0, &items, *day, ours_living) > crate::content::BUNKER_WATER_DAYS {
        episode.reach(crate::content::STORING_NOTICED, me.0, tick.0);
    }
}

/// Pivot point 7 reached going there (the-tap.md "Pivot names").
const FINDS_THE_WELL_DRY: &str = "finds Mara's well dry";

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
