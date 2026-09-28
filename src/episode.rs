//! The episode running now (episodes.md): started by the storm after the
//! reality, its parts cast from the bunkers' people once they exist.

use bevy::prelude::*;
use modforge::actor::ActorId;
use modforge::storyteller::{CastBunker, cast};

use modforge::actions::Said;
use modforge::item;
use modforge::memory::Did;

use crate::Ground;
use crate::actor::{Actor, Dead, Home, Memory, Npc, Store};
use crate::content::{BUNKER_AT, EpisodeRegistryRes, ItemRegistryRes, PLAYER_FACTION};
use crate::interact::{DrawsFrom, spawn_item};

/// The episode running now: one that fits the reality, started by the
/// storm that handed the reality over; its cast and the pivot points
/// reached so far.
#[derive(Resource)]
pub struct Episode {
    /// None when no episode fits the reality.
    pub name: Option<String>,
    /// Each part and the person cast for it.
    pub cast: Vec<(String, ActorId)>,
    /// Whether the cast has been filled for this episode.
    pub cast_done: bool,
    /// Each pivot point reached: its name, who, and when.
    pub reached: Vec<(String, ActorId, u64)>,
    /// What the episode brought into the world, where, and the key
    /// memory knows it by.
    pub things: Vec<(String, Vec2, u64)>,
    /// Every line said in this episode, its words as said (one way of
    /// each, slots filled): a `Said::Line` names its line by its place
    /// here.
    pub spoken: Vec<String>,
    /// The player's choices open now: the line of its words, what
    /// choosing it is, and who it is said to.
    pub choices: Vec<(u32, String, ActorId)>,
}

impl Episode {
    /// A new episode starts: nobody cast, nothing reached.
    pub fn start(&mut self, name: Option<String>) {
        *self = Self::new(name);
    }

    fn new(name: Option<String>) -> Self {
        Self {
            name,
            cast: Vec::new(),
            cast_done: false,
            reached: Vec::new(),
            things: Vec::new(),
            spoken: Vec::new(),
            choices: Vec::new(),
        }
    }

    /// Keep a line's words; its line number for `Said::Line`.
    pub fn speak(&mut self, words: String) -> u32 {
        self.spoken.push(words);
        (self.spoken.len() - 1) as u32
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

    /// The thing the episode brought in by this name.
    fn thing(&self, name: &str) -> Option<(Vec2, u64)> {
        self.things.iter().find(|(n, ..)| n == name).map(|(_, at, key)| (*at, *key))
    }

    /// The person cast for a part.
    pub fn cast_as(&self, part: &str) -> Option<ActorId> {
        self.cast.iter().find(|(p, _)| p == part).map(|(_, id)| *id)
    }
}

impl FromWorld for Episode {
    fn from_world(world: &mut World) -> Self {
        let reality = world.get_resource_or_init::<crate::Reality>().0.name.clone();
        let seed = world.resource::<crate::WorldSeed>().0;
        let episodes = world.get_resource_or_init::<EpisodeRegistryRes>();
        Self::new(episodes.0.pick(&reality, seed, 0).map(|e| e.name.clone()))
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
                    distance: home.0.distance(BUNKER_AT),
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
    episode.cast = cast(&parts, &bunkers, seed.0);
    episode.cast_done = true;
    info!("episode {:?} cast: {:?}", episode.name, episode.cast);
}

/// How far from the way a site can stand and still be on it, metres.
const ON_THE_WAY: f32 = 80.0;

/// The key memory knows a site by: its position, marked apart from things.
pub fn site_key(at: Vec2) -> u64 {
    const SITE: u64 = 1 << 62;
    SITE | ((at.x.round() as i32 as u32 as u64) << 24) ^ (at.y.round() as i32 as u32 as u64)
}

/// Where the player's bunker door stands on the ground (the starting
/// bunker's one door, `structure::doors_of`), and which way it faces out.
pub fn bunker_door(bunker: Vec2) -> (Vec2, Vec2) {
    let door = modforge::structure::doors_of(&crate::content::starting_bunker())
        .first()
        .map_or(Vec2::ZERO, |d| Vec2::new(d.center.x, d.center.z));
    (bunker + door, door.normalize_or(Vec2::Y))
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
    let (biomes, monuments) = registries;
    let r = modforge::world2d::Registries {
        biomes: &biomes.0,
        monuments: &monuments.monuments,
        buildings: &monuments.buildings,
    };
    // Anywhere between the bunkers: as far out as half the way is long.
    let margin = Vec2::splat((door.distance(home.0) / 2.0).max(ON_THE_WAY));
    let sites: Vec<modforge::places::Place> = world
        .world
        .sites_in(r, door.min(home.0) - margin, door.max(home.0) + margin)
        .into_iter()
        .filter(|p| !p.rolled.members.is_empty())
        .collect();
    let middle = (door + home.0) / 2.0;
    let Some(site) = sites.iter().min_by(|a, b| a.position.distance(middle).total_cmp(&b.position.distance(middle))) else {
        warn!("The Tap: no building between the bunkers to put the tap in");
        episode.things.push(("no tap".to_string(), middle, 0));
        return;
    };
    let member = &site.rolled.members[0];
    let at = site.position + Vec2::new(member.offset.x, member.offset.z);
    let tap = spawn_item(&mut commands, def, item::create(def, 1, &[], 0.0, 0), at);
    commands.entity(tap).insert(DrawsFrom(store.0));
    let key = tap.to_bits();
    if let Some(dell) = episode.cast_as("Dell")
        && let Some((_, mut memory)) = memories.iter_mut().find(|(a, _)| a.0 == dell)
    {
        let ground = |p: Vec2| Vec3::new(p.x, 0.0, p.y);
        memory.0.see(key, "tap", ground(at), tick.0);
        // Dell wants it kept secret (docs/episodes/the-tap.md "Pushed all
        // the way"): she never passes it on.
        memory.0.keep_quiet(key);
        for passed in sites.iter().filter(|p| off_the_way(door, at, p.position).0 <= ON_THE_WAY) {
            memory.0.see(site_key(passed.position), &passed.monument, ground(passed.position), tick.0);
        }
    }
    episode.things.push(("tap".to_string(), at, key));
    episode.things.push(("tap site".to_string(), site.position, site_key(site.position)));
    info!("the tap stands in the {} at {at}, drawing from Mara's bunker's store", site.monument);
}

/// Dell's way to the tap, from what Dell knows: the slots of "the way"
/// (episodes.md "The lines"). {way} out of the bunker door, the {place}
/// the tap stands in, and the site Dell knows nearest the way there as the
/// {landmark}, with the {side} it stands on.
fn the_way(memory: &modforge::memory::Memory, door: (Vec2, Vec2), tap: u64, tap_site: u64) -> Vec<(&'static str, String)> {
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
        .filter(|k| k.key != tap_site && k.key & (1 << 62) != 0)
        .map(|k| (k, off_the_way(door, tap_at, ground(k.position))))
        .filter(|(_, (off, along))| *off <= ON_THE_WAY && *along > 0.0 && *along < 1.0)
        .min_by(|a, b| a.1.0.total_cmp(&b.1.0));
    if let Some((k, _)) = landmark {
        slots.push(("landmark", k.kind.clone()));
        slots.push(("side", modforge::talk::side_of(to, ground(k.position) - door).to_string()));
    }
    slots
}

/// How The Tap opens (docs/episodes/the-tap.md "The lines"): Dell, who
/// found the tap, says her opening lines to the player once, one way of
/// each, the way filled from her memory; then the player's choices open.
/// Every word through the same `Talk` anyone pushes.
#[allow(clippy::too_many_arguments)]
pub fn dell_opens_the_tap(
    tick: Res<crate::SimTick>,
    mut episode: ResMut<Episode>,
    episodes: Res<EpisodeRegistryRes>,
    seed: Res<crate::WorldSeed>,
    world: Res<crate::terrain::WorldRes>,
    player: Query<(&Actor, &Transform), (With<crate::player::Player>, Without<Dead>)>,
    mut people: Query<(&Actor, &Transform, &Memory, &mut crate::input::Actions), Without<Dead>>,
) {
    if episode.name.as_deref() != Some("The Tap") || episode.reached("the opening") {
        return;
    }
    let (Some(dell), Some((_, tap)), Some((_, tap_site))) = (episode.cast_as("Dell"), episode.thing("tap"), episode.thing("tap site")) else {
        return;
    };
    let (Some(def), Ok((player, near))) = (episodes.0.def("The Tap"), player.single()) else {
        return;
    };
    let Some((_, at, memory, mut actions)) = people.iter_mut().find(|(a, ..)| a.0 == dell) else {
        return;
    };
    if at.translation.ground().distance(near.translation.ground()) > crate::talk::TALK_REACH {
        return;
    }
    let slots = the_way(&memory.0, bunker_door(world.bunker), tap, tap_site);
    let slots: Vec<(&str, &str)> = slots.iter().map(|(n, v)| (*n, v.as_str())).collect();
    for (salt, line) in def.lines.iter().filter(|l| l.part == "Dell").enumerate() {
        let Some(words) = modforge::storyteller::say(&line.ways, &slots, seed.0, salt as u64) else {
            continue;
        };
        let key = if line.at == "the way" { tap } else { 0 };
        let line = episode.speak(words);
        actions.0.push(tick.0, crate::input::Action::Talk { to: player.0.0, said: Said::Line { line, key } });
    }
    for (salt, choice) in def.choices.iter().filter(|c| c.at == "the opening").enumerate() {
        if let Some(words) = modforge::storyteller::say(&choice.ways, &[], seed.0, 100 + salt as u64) {
            let line = episode.speak(words);
            episode.choices.push((line, choice.choice.clone(), dell));
        }
    }
    episode.reach("the opening", dell, tick.0);
}

/// The player's choice at the moment open: reached when the player's
/// memory holds its words said to the one it is for (memory is the one
/// record). Asked the way, Dell tells it again, another way; any other
/// choice closes the moment.
pub fn the_players_choice(
    tick: Res<crate::SimTick>,
    mut answered: Local<u64>,
    mut episode: ResMut<Episode>,
    episodes: Res<EpisodeRegistryRes>,
    seed: Res<crate::WorldSeed>,
    world: Res<crate::terrain::WorldRes>,
    player: Query<(&Actor, &Memory), (With<crate::player::Player>, Without<Dead>)>,
    mut people: Query<(&Actor, &Memory, &mut crate::input::Actions), Without<crate::player::Player>>,
) {
    if episode.choices.is_empty() {
        return;
    }
    let Ok((player, memory)) = player.single() else {
        return;
    };
    let said = memory.0.done.iter().rev().find_map(|(t, did)| match did {
        Did::Talked(to, words) if *t > *answered => episode
            .choices
            .iter()
            .find(|(line, _, whom)| whom == to && episode.spoken.get(*line as usize) == Some(words))
            .map(|(_, choice, whom)| (*t, choice.clone(), *whom)),
        _ => None,
    });
    let Some((when, choice, whom)) = said else {
        return;
    };
    *answered = when;
    if choice != "asks the way" {
        episode.reach(&choice, player.0, when);
        episode.choices.retain(|(_, _, to)| *to != whom);
        // Their reply (episodes.md "Conversations"), one way of it.
        let reply = episode
            .name
            .as_deref()
            .and_then(|n| episodes.0.def(n))
            .and_then(|def| def.choices.iter().find(|c| c.choice == choice))
            .and_then(|c| modforge::storyteller::say(&c.reply, &[], seed.0, when));
        if let Some(words) = reply
            && let Some((_, _, mut actions)) = people.iter_mut().find(|(a, ..)| a.0 == whom)
        {
            let line = episode.speak(words);
            actions.0.push(tick.0, crate::input::Action::Talk { to: player.0.0, said: Said::Line { line, key: 0 } });
        }
        return;
    }
    // The way again, another way of saying it.
    let (Some(def), Some((_, tap)), Some((_, tap_site))) = (episodes.0.def("The Tap"), episode.thing("tap"), episode.thing("tap site")) else {
        return;
    };
    let Some((_, dell_memory, mut actions)) = people.iter_mut().find(|(a, ..)| a.0 == whom) else {
        return;
    };
    let slots = the_way(&dell_memory.0, bunker_door(world.bunker), tap, tap_site);
    let slots: Vec<(&str, &str)> = slots.iter().map(|(n, v)| (*n, v.as_str())).collect();
    if let Some(way) = def.lines.iter().find(|l| l.at == "the way")
        && let Some(words) = modforge::storyteller::say(&way.ways, &slots, seed.0, when)
    {
        let line = episode.speak(words);
        actions.0.push(tick.0, crate::input::Action::Talk { to: player.0.0, said: Said::Line { line, key: tap } });
    }
}

/// Who The Tap asks to be where (docs/episodes/the-tap.md), once a game
/// second, as `Asked` the brain weighs like any choice: Mara to the tap
/// from the start (her bunker's water is going); Dell, once the player has
/// chosen, to the tap ("Show me.", or alone when not believed); sent to
/// look again, Dell goes to the tap, comes back home, and tells the way
/// again.
#[allow(clippy::too_many_arguments)]
pub fn the_taps_errands(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    mut episode: ResMut<Episode>,
    episodes: Res<EpisodeRegistryRes>,
    seed: Res<crate::WorldSeed>,
    world: Res<crate::terrain::WorldRes>,
    mut walk: ResMut<crate::nav::Walk>,
    player: Query<(Entity, &Actor, &Transform), (With<crate::player::Player>, Without<Dead>)>,
    mut people: Query<(Entity, &Actor, &Transform, &Home, &Memory, &mut crate::input::Actions, Has<crate::ai::Leading>), Without<crate::player::Player>>,
) {
    if tick.0 % 60 != 0 || episode.name.as_deref() != Some("The Tap") {
        return;
    }
    let Some((tap, tap_key)) = episode.thing("tap") else {
        return;
    };
    // Where each stands by the tap: two metres off, on a spot they can
    // stand on (the tap's building has walls), Mara on one side and Dell
    // on the other.
    let by_tap = |walk: &mut crate::nav::Walk, from: f32| walk.stand_by(tap, 2.0, from).unwrap_or(tap);
    let mara_at = by_tap(&mut walk, 0.0);
    let dell_at = by_tap(&mut walk, std::f32::consts::PI);
    // The episode's errand wins over stopping to talk: it is no longer
    // the conversation's to let go.
    let ask = |commands: &mut Commands, entity: Entity, at: Option<Vec2>| match at {
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
    let mut tell_again = None;
    let believed = episode.reached("believes Dell");
    let player_id = player.single().ok().map(|(e, a, _)| (a.0, e));
    for (entity, actor, at, home, _, _, leading) in &people {
        if Some(actor.0) == mara {
            ask(&mut commands, entity, Some(mara_at));
        } else if Some(actor.0) == dell {
            let here = at.translation.ground();
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
                if here.distance(home.0) <= 2.0 {
                    tell_again = Some(actor.0);
                }
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
    // Back from looking, with the player in reach: the way again.
    let (Some(dell), Ok((_, player, near))) = (tell_again, player.single()) else {
        return;
    };
    let (Some(def), Some((_, tap_site))) = (episodes.0.def("The Tap"), episode.thing("tap site")) else {
        return;
    };
    let Some((_, _, at, _, memory, mut actions, _)) = people.iter_mut().find(|(_, a, ..)| a.0 == dell) else {
        return;
    };
    if at.translation.ground().distance(near.translation.ground()) > crate::talk::TALK_REACH {
        return;
    }
    let slots = the_way(&memory.0, bunker_door(world.bunker), tap_key, tap_site);
    let slots: Vec<(&str, &str)> = slots.iter().map(|(n, v)| (*n, v.as_str())).collect();
    if let Some(way) = def.lines.iter().find(|l| l.at == "the way")
        && let Some(words) = modforge::storyteller::say(&way.ways, &slots, seed.0, tick.0)
    {
        let line = episode.speak(words);
        actions.0.push(tick.0, crate::input::Action::Talk { to: player.0.0, said: Said::Line { line, key: tap_key } });
        episode.reach("Dell told the way again", dell, tick.0);
    }
}

/// The Tap's pivot point 4 (docs/episodes/the-tap.md): the first meeting
/// with Mara at the tap. With the player in reach of her there, she says
/// her line once, and the player's choices for the meeting open.
#[allow(clippy::too_many_arguments)]
pub fn the_first_meeting(
    tick: Res<crate::SimTick>,
    mut episode: ResMut<Episode>,
    episodes: Res<EpisodeRegistryRes>,
    seed: Res<crate::WorldSeed>,
    player: Query<(&Actor, &Transform), (With<crate::player::Player>, Without<Dead>)>,
    mut people: Query<(&Actor, &Transform, &mut crate::input::Actions), (Without<crate::player::Player>, Without<Dead>)>,
) {
    if episode.name.as_deref() != Some("The Tap") || episode.reached("the meeting") {
        return;
    }
    let (Some(mara), Some((tap, _)), Some(def), Ok((player, near))) =
        (episode.cast_as("Mara"), episode.thing("tap"), episodes.0.def("The Tap"), player.single())
    else {
        return;
    };
    let Some((_, at, mut actions)) = people.iter_mut().find(|(a, ..)| a.0 == mara) else {
        return;
    };
    let here = at.translation.ground();
    if here.distance(tap) > 6.0 || here.distance(near.translation.ground()) > crate::talk::TALK_REACH {
        return;
    }
    if let Some(line) = def.lines.iter().find(|l| l.part == "Mara" && l.at == "the meeting")
        && let Some(words) = modforge::storyteller::say(&line.ways, &[], seed.0, 200)
    {
        let line = episode.speak(words);
        actions.0.push(tick.0, crate::input::Action::Talk { to: player.0.0, said: Said::Line { line, key: 0 } });
    }
    for (salt, choice) in def.choices.iter().filter(|c| c.at == "the meeting").enumerate() {
        if let Some(words) = modforge::storyteller::say(&choice.ways, &[], seed.0, 300 + salt as u64) {
            let line = episode.speak(words);
            episode.choices.push((line, choice.choice.clone(), mara));
        }
    }
    episode.reach("the meeting", mara, tick.0);
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
