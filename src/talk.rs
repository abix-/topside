//! The one talk path (operator, 2026-09-27): every person's `Talk` this
//! tick, the player's and the brain's alike, reaches one person in reach;
//! modforge `talk` decides what comes of it and writes both memories.
//!
//! One way for every conversation (episodes.md "One way for every
//! conversation"): what anyone says, an episode's line or plain talk, and
//! the player's choices are the lines whose rules match best (modforge
//! `talk::pick`, `talk::choices`), read against the speaker's memory; the
//! words are worked out when the line is said. This file only gathers
//! what modforge matches against and carries out what it picks.

use avian3d::prelude::*;
use bevy::prelude::*;
use modforge::actions::Said;
use modforge::actor::ActorId;
use modforge::hud::Prompt;
use modforge::memory::Memory as Mind;
use modforge::storyteller::{LineDef, When};

use crate::Ground;
use crate::actor::{Actor, ActorMapRes, Dead, Inventory, Memory, Survival};
use crate::content::EpisodeRegistryRes;
use crate::episode::Episode;
use crate::input::{Action, ThisTick};
use crate::interact::Hud;
use crate::player::{Eyes, Player};

/// How far away a person can be talked to, in metres.
pub const TALK_REACH: f32 = 3.0;

/// Whether two people at `a` and `b` are within `reach` of each other to
/// talk: that far on the ground, and on the same level (authority.md
/// "Presentation": a floor between them stops it).
pub fn within(a: Vec3, b: Vec3, reach: f32) -> bool {
    a.ground().distance(b.ground()) <= reach && (a.y - b.y).abs() < 1.5
}

/// How far from a thing a known place can be and still be where the thing
/// is ("the well at the campsite"), in metres. Tuning.
const PLACE_OF_A_THING: f32 = 60.0;

/// The person the player is looking at within talking distance: the
/// entity and their ActorId, from a ray along the player's view.
#[derive(Component, Default, Debug, Clone, Copy)]
pub struct LookingAt(pub Option<(Entity, u64)>);

/// The player's conversation: with whom, the tick it started (only what
/// is said from then counts), whether the player started it (their E),
/// the tick their current choices showed (for silence's wait), the last
/// line said (its tick) and the tick it has been read by (modforge
/// `talk::reading_secs`), and the tick it ends at once its last line has
/// been said and read. All in ticks, never real time, so a talk plays the
/// same however fast the game runs (authority.md rule 5). The HUD shows
/// it from `HudState::talking`.
#[derive(Component, Debug, Clone, Copy, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Conversation {
    pub with: u64,
    pub started: u64,
    pub opened: bool,
    pub choices_since: Option<u64>,
    pub read: Option<(u64, u64)>,
    pub ends_at: Option<u64>,
}

impl Conversation {
    pub fn new(with: u64, started: u64, opened: bool) -> Self {
        Self { with, started, opened, choices_since: None, read: None, ends_at: None }
    }
}

/// A time in game seconds as ticks (60 a second), rounded up.
/// Whether the last line `memory` heard from `by` is still being read now
/// (modforge `talk::reading_secs`): nothing more is said to them until it
/// is, in a talk or before one.
fn still_reading(memory: &modforge::memory::Memory, by: ActorId, now: u64) -> bool {
    memory.done.iter().rev().find_map(|(t, d)| match d {
        modforge::memory::Did::Heard(from, l) if *from == by => Some(*t + ticks_of(modforge::talk::reading_secs(&l.words))),
        _ => None,
    }).is_some_and(|until| now < until)
}

pub fn ticks_of(secs: f64) -> u64 {
    (secs * 60.0).ceil() as u64
}

/// The HUD's picture of a new conversation with this person.
pub fn talking_with(with: u64, name: &str) -> modforge::hud::Talking {
    modforge::hud::Talking { with, name: name.to_string(), ..Default::default() }
}

/// What a called line is called with: a knock on a shut door, or a holder's
/// warning to one who comes for the place they hold (todo 11z).
#[derive(Clone, Copy, PartialEq)]
pub enum Call {
    Knocking,
    Warning,
}

/// The line `caller` calls to `to` with their knock or warning, if one fits
/// (authority.md "Talking": one called line, heard like the knock, no
/// conversation opened; a line rule with `When::Knocking` or
/// `When::Warning`, picked as any line is): its name and words, as they
/// say it now.
pub fn called_line(
    (episode, lines, bunker, seed, items): (&Episode, &Lines, Vec2, u64, &modforge::item::ItemRegistry),
    (caller, memory, at, survival): (ActorId, &Memory, Vec3, Option<&Survival>),
    to: (ActorId, bool),
    call: Call,
    tick: u64,
) -> Option<modforge::memory::Line> {
    let moment = Moment::of(episode, (caller, false), to, at, survival);
    let mut speaking = moment.speaking(to.0, &memory.0, tick, false);
    speaking.knocking = call == Call::Knocking;
    speaking.warning = call == Call::Warning;
    // Only a line written for this call is called (When::Knocking or
    // When::Warning): any other line waits for the talk (the opening once
    // "you awake" was said had gone through the door).
    let written_for = |l: &LineDef| l.when.iter().any(|w| matches!((w, call), (When::Knocking, Call::Knocking) | (When::Warning, Call::Warning)));
    let called: Vec<usize> = (0..lines.0.len()).filter(|&i| written_for(&lines.0[i])).collect();
    let candidates: Vec<LineDef> = called.iter().map(|&i| lines.0[i].clone()).collect();
    let line = called[modforge::talk::pick(&candidates, &speaking, seed ^ tick)?];
    // A call is made where they stand: what they know is reckoned from there.
    let (words, _) = words_of(&lines.0[line], line, (caller, &memory.0, at), tick, (episode, items), bunker, seed)?;
    Some(modforge::memory::Line { name: lines.0[line].name.clone(), words })
}

/// A person held where they stand while the player talks or trades with
/// them (they stop to talk, as anyone would); let go when it ends.
#[derive(Component, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct HeldToTalk;

/// Hold `them` where their feet are (`at`), to talk or trade: asked to be
/// there, the same `Asked` the brain weighs.
pub fn hold(commands: &mut Commands, them: Entity, at: Vec3) {
    commands.entity(them).insert((crate::actor::Asked(at), HeldToTalk));
}

/// Let `them` go back to what they were doing, if they were held.
pub fn let_go(commands: &mut Commands, them: Entity) {
    commands.entity(them).remove::<(crate::actor::Asked, HeldToTalk)>();
}

/// Every line anyone can say now: the running episode's, then plain talk
/// (content). A `Said::Line` names its line by its id (modforge
/// `LineDef::id`), never its place here.
#[derive(Resource, Default)]
pub struct Lines(pub Vec<LineDef>);

impl Lines {
    /// The line with this id, and its place here.
    pub fn by_id(&self, id: u64) -> Option<(usize, &LineDef)> {
        self.0.iter().enumerate().find(|(_, l)| l.id() == id)
    }
}

/// The lines, rebuilt when another episode starts.
pub fn lines(episode: Res<Episode>, episodes: Res<EpisodeRegistryRes>, mut lines: ResMut<Lines>, mut for_episode: Local<Option<Option<String>>>) {
    if for_episode.as_ref() == Some(&episode.name) {
        return;
    }
    let mut all: Vec<LineDef> = episode.name.as_deref().and_then(|n| episodes.0.def(n)).map(|d| d.lines.clone()).unwrap_or_default();
    all.extend(crate::content::plain_lines());
    lines.0 = all;
    *for_episode = Some(episode.name.clone());
}

/// What is true of one person about to speak to another: their parts,
/// what the episode has reached, how far they stand from its things, and
/// their needs. Owned, so it lives while modforge reads it.
struct Moment {
    part: String,
    to_part: String,
    reached: Vec<String>,
    near: Vec<(String, f32)>,
    needs: Vec<(String, f32)>,
    feels: f32,
}

impl Moment {
    fn of(episode: &Episode, (who, is_player): (ActorId, bool), (to, to_is_player): (ActorId, bool), at: Vec3, survival: Option<&Survival>) -> Self {
        let part = |id: ActorId, player: bool| if player { "player".to_string() } else { episode.part_of(id).unwrap_or("").to_string() };
        Self {
            part: part(who, is_player),
            to_part: part(to, to_is_player),
            reached: episode.reached_names(),
            near: episode.things.iter().map(|(name, there, _)| (name.clone(), there.distance(at))).collect(),
            needs: survival.map_or_else(Vec::new, |s| vec![("hunger".to_string(), s.0.hunger), ("thirst".to_string(), s.0.thirst)]),
            feels: 0.0,
        }
    }

    /// The one spoken to belongs to no group and plays no part: a
    /// stranger, the part a line names as "stranger" (todo 11af).
    fn to_a_stranger(mut self, stranger: bool) -> Self {
        if stranger && self.to_part.is_empty() {
            self.to_part = "stranger".to_string();
        }
        self
    }

    /// The speaker belongs to no group and plays no part: a stranger,
    /// speaking the lines written for "stranger" (todo 11af).
    fn from_a_stranger(mut self, stranger: bool) -> Self {
        if stranger && self.part.is_empty() {
            self.part = "stranger".to_string();
        }
        self
    }

    /// How the speaker feels toward the one spoken to (`When::Feels`).
    fn feeling(mut self, feels: f32) -> Self {
        self.feels = feels;
        self
    }

    fn speaking<'a>(&'a self, to: ActorId, memory: &'a Mind, since: u64, opened: bool) -> modforge::talk::Speaking<'a> {
        modforge::talk::Speaking {
            part: &self.part,
            to_part: &self.to_part,
            to,
            memory,
            since,
            opened,
            reached: &self.reached,
            near: &self.near,
            needs: &self.needs,
            knocking: false,
            warning: false,
            feels: self.feels,
        }
    }
}

/// The way to a thing, from what the speaker knows: the slots of "the way"
/// (episodes.md "The lines"). {way} out of the bunker door, the {place} the
/// thing stands in (its site), and the site they know nearest the way there
/// as the {landmark}, with the {side} it stands on.
pub fn the_way(memory: &modforge::memory::Memory, door: (Vec2, Vec2), thing: u64, site: u64) -> Vec<(&'static str, String)> {
    let (door, out) = door;
    let ground = |p: Vec3| Vec2::new(p.x, p.z);
    let Some(thing_at) = memory.known.iter().find(|k| k.key == thing).map(|k| ground(k.position)) else {
        return Vec::new();
    };
    let to = thing_at - door;
    let mut slots = vec![("way", modforge::talk::way_out(out, to).to_string())];
    if let Some(place) = memory.known.iter().find(|k| k.key == site) {
        slots.push(("place", place.kind.clone()));
    }
    let landmark = memory
        .known
        .iter()
        .filter(|k| k.key != site && k.key & crate::actor::SITE != 0)
        .map(|k| (k, crate::episode::off_the_way(door, thing_at, ground(k.position))))
        .filter(|(_, (off, along))| *off <= crate::episode::ON_THE_WAY && *along > 0.0 && *along < 1.0)
        .min_by(|a, b| a.1.0.total_cmp(&b.1.0));
    if let Some((k, _)) = landmark {
        slots.push(("landmark", k.kind.clone()));
        slots.push(("side", modforge::talk::side_of(to, ground(k.position) - door).to_string()));
    }
    slots
}

/// A line's words and what it tells of, as this speaker says it now: the
/// thing its rule tells of (an episode thing by name, else a known thing of
/// that kind they do not keep quiet), its {slots} filled from their memory
/// (the way to an episode thing from the bunker door, the place a known
/// thing is at), one way picked from the seed, the speaker, the line, and
/// the conversation, so the panel shows what is then said.
/// Where a speaker reckons what they know from (`words_of`'s
/// {source of <need>}): their home; the player's is their bunker; one with
/// no home, where they stand.
fn home_of(home: Option<&crate::actor::Home>, is_player: bool, feet: Vec3, bunker: Vec2) -> Vec3 {
    match home {
        Some(home) => home.0,
        None if is_player => Vec3::new(bunker.x, 0.0, bunker.y),
        None => feet,
    }
}

pub fn words_of(
    line: &LineDef,
    index: usize,
    (speaker, memory, home): (ActorId, &Mind, Vec3),
    since: u64,
    (episode, items): (&Episode, &modforge::item::ItemRegistry),
    bunker: Vec2,
    seed: u64,
) -> Option<(String, u64)> {
    let key = if line.tells.is_empty() {
        0
    } else if let Some((_, key)) = episode.thing(&line.tells) {
        key
    } else {
        memory.known.iter().find(|k| k.kind == line.tells && !memory.quiet.contains(&k.key)).map_or(0, |k| k.key)
    };
    let mut slots: Vec<(&str, String)> = Vec::new();
    if key != 0 {
        if let Some((_, site)) = episode.thing(&format!("{} site", line.tells)) {
            slots = the_way(memory, crate::structure::bunker_door(bunker), key, site);
        } else if let Some(thing) = memory.known.iter().find(|k| k.key == key) {
            let ground = |p: Vec3| Vec2::new(p.x, p.z);
            let place = memory
                .known
                .iter()
                .filter(|k| k.key & crate::actor::SITE != 0)
                .map(|k| (k, ground(k.position).distance(ground(thing.position))))
                .filter(|(_, d)| *d <= PLACE_OF_A_THING)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((place, _)) = place {
                slots.push(("place", place.kind.clone()));
            }
        }
    }
    // {source of <need>} ("source of thirst"): the speaker's known source
    // of that need (eaten or drunk from where it stands: a river's water, a
    // well) nearest their home, by what it is; never the episode's own
    // things, which the line is about.
    for way in &line.ways {
        for (at, _) in way.match_indices("{source of ") {
            let Some(name) = way[at + 1..].split('}').next() else {
                continue;
            };
            let need = match name.trim_start_matches("source of ") {
                "hunger" => modforge::survival::Need::Hunger,
                "thirst" => modforge::survival::Need::Thirst,
                _ => continue,
            };
            let source = memory
                .known
                .iter()
                .filter(|k| !episode.things.iter().any(|(.., key)| *key == k.key))
                .filter(|k| items.def(&k.kind).is_some_and(|d| d.kind == modforge::item::ItemKind::Source))
                .filter(|k| modforge::memory::food_worth(k, need, items) > 0.0)
                .min_by(|a, b| a.position.distance(home).total_cmp(&b.position.distance(home)));
            if let Some(source) = source
                && !slots.iter().any(|(n, _)| *n == name)
            {
                slots.push((name, source.kind.clone()));
            }
        }
    }
    // Silence has no ways: it is remembered as said with no words.
    if line.ways.is_empty() {
        return Some(("...".to_string(), key));
    }
    let slots: Vec<(&str, &str)> = slots.iter().map(|(n, v)| (*n, v.as_str())).collect();
    let words = modforge::storyteller::say(&line.ways, &slots, seed ^ speaker.0 ^ since, index as u64)?;
    Some((words, key))
}

/// Every person's `Talk` this tick: the listener, by ActorId, alive and
/// within `TALK_REACH`; the offered item from the speaker's inventory; a
/// line's words worked out now (`words_of`). Spoken to, the player is in a
/// conversation with the speaker.
#[allow(clippy::too_many_arguments)]
pub fn talk(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    map: Res<ActorMapRes>,
    episode: Res<Episode>,
    lines: Res<Lines>,
    world: Res<crate::terrain::WorldRes>,
    seed: Res<crate::WorldSeed>,
    speakers: Query<(Entity, &Actor, &ThisTick, &Name, Has<crate::actor::Asked>, Option<&crate::actor::Home>), Without<Dead>>,
    mut people: Query<(&Transform, &mut Memory, &mut Inventory, &mut Hud, Has<Player>, Option<&Conversation>, Option<&crate::actor::Npc>), Without<Dead>>,
    mut joins: ResMut<crate::joining::Joins>,
    mut to_edge: ResMut<crate::edge::WordToEdge>,
    items: Res<crate::content::ItemRegistryRes>,
) {
    // In ActorId order (authority.md rule 5): of two speaking to the player
    // in one tick, whom the player ends up talking to is the same after a
    // load.
    let order = crate::actor::in_actor_order(speakers.iter().map(|(entity, actor, ..)| (Some(actor.0), entity)));
    for (speaker, id, this_tick, name, asked, home) in order.into_iter().filter_map(|e| speakers.get(e).ok()) {
        for said in this_tick.0.iter().filter_map(|s| match s.action {
            Action::Talk { to, said } => Some((to, said)),
            _ => None,
        }) {
            let (to, said) = said;
            let Some(listener) = map.0.handle_of(ActorId(to)) else {
                continue;
            };
            let Ok([(from, mut speaker_memory, mut inventory, _, speaker_is_player, speaker_talking, speaker_npc), (at, mut listener_memory, mut listener_inventory, mut hud, is_player, listener_talking, listener_npc)]) =
                people.get_many_mut([speaker, listener])
            else {
                continue;
            };
            if !within(from.translation, at.translation, TALK_REACH) {
                continue;
            }
            let offered = match said {
                Said::Offer { slot } => inventory.0.slots.get(slot).and_then(Option::as_ref).map(|s| s.item.clone()),
                _ => None,
            };
            // A line's words, worked out now, as the panel showed them.
            let since = speaker_talking.or(listener_talking).map_or(tick.0, |c| c.started);
            let (said, words, gives) = match said {
                Said::Line { line, key } => {
                    let Some((index, def)) = lines.by_id(line) else {
                        continue;
                    };
                    let home = home_of(home, speaker_is_player, crate::interact::feet_of(from), world.bunker);
                    let Some((words, told)) = words_of(def, index, (id.0, &speaker_memory.0, home), since, (&episode, &items.0), world.bunker, seed.0) else {
                        continue;
                    };
                    let key = if key != 0 { key } else { told };
                    (Said::Line { line, key }, Some((def.name.clone(), words)), def.gives.clone())
                }
                other => (other, None, String::new()),
            };
            let answer = modforge::talk::talk(
                said,
                (id.0, &mut speaker_memory.0),
                (ActorId(to), &mut listener_memory.0),
                (offered.as_deref(), words.as_ref().map(|(n, w)| (n.as_str(), w.as_str()))),
                tick.0,
            );
            debug!("{:?} to {to}: {said:?}, {answer:?}", id.0);
            // Each one's group: the player's bunker for the player, else
            // their faction.
            let group = |player: bool, npc: Option<&crate::actor::Npc>| if player { Some(crate::content::PLAYER_FACTION.to_string()) } else { npc.and_then(|n| n.faction.clone()) };
            // Agreeing to stay, said: they join the group of the one who
            // asked, whoever that is (todo 11af).
            if words.as_ref().is_some_and(|(name, _)| name == crate::content::AGREES_TO_STAY)
                && let Some(theirs) = group(is_player, listener_npc)
            {
                joins.0.push((speaker, theirs));
            }
            // A line that gives (its `gives`): one of that item goes from
            // the speaker to the one spoken to, if they carry one.
            if !gives.is_empty()
                && let Some(mut one) = inventory.0.slots.iter().flatten().find(|s| s.item == gives).cloned()
                && inventory.0.take_one(&one.item)
            {
                one.count = 1;
                let max = items.0.def(&one.item).map_or(1, |d| d.max_stack);
                let _ = listener_inventory.0.add(one, max);
            }
            // A place told to someone of another group: word of it sets out
            // for the edge (todo 11i).
            if let modforge::talk::Answer::Learned(key) = answer
                && group(speaker_is_player, speaker_npc) != group(is_player, listener_npc)
                && let Some((kind, place)) = speaker_memory.0.known.iter().find(|k| k.key == key).map(|k| (k.kind.clone(), k.position))
            {
                to_edge.0.push((key, kind, place, from.translation.ground()));
            }
            if is_player {
                spoken_to(&mut commands, listener, &mut hud, (id.0.0, name.as_str()), tick.0);
                // Speaking to the player, they stop to talk now, unless
                // something already has them going somewhere: the brain,
                // which may think before the talk next holds them, never
                // sends them off mid-sentence (Roxanne left the opening).
                if !asked {
                    hold(&mut commands, speaker, crate::interact::feet_of(from));
                }
            }
        }
    }
}

/// The player spoken to by this person, or knocked for (interact::knock):
/// in a conversation with them, unless they already are; whose turn it is
/// then the conversation decides.
pub fn spoken_to(commands: &mut Commands, player: Entity, hud: &mut Hud, (by, name): (u64, &str), tick: u64) {
    if hud.0.talking.as_ref().is_none_or(|t| t.with != by) {
        hud.0.talking = Some(talking_with(by, name));
        commands.entity(player).insert(Conversation::new(by, tick, false));
    }
}

/// Who the player looks at: a ray from their eyes along their view,
/// through Avian, as far as `TALK_REACH`; a living person it hits first is
/// the one. The prompt says "[E] Talk to" and their name.
pub fn look_at(
    spatial: SpatialQuery,
    eyes: Query<(&Transform, &Eyes), Without<Player>>,
    mut players: Query<(Entity, &Transform, &crate::actor::Facing, &mut LookingAt, &mut Hud), With<Player>>,
    people: Query<(&Actor, &Name), Without<Dead>>,
    things: Query<(Entity, Option<&crate::interact::Door>, &Transform), (With<crate::interact::Interactable>, Without<Player>)>,
    (stores, containers): (Query<&crate::actor::Store>, Query<&crate::interact::Container>),
) {
    let (Ok((eye, eyes)), Ok((me, body, facing, mut looking, mut hud))) = (eyes.single(), players.single_mut()) else {
        return;
    };
    // Where they face and how far the eyes tilt, now: not the camera's
    // world transform (worked out once a frame, while several fixed steps
    // can run in one) nor the body's turn (it follows `Facing` later in
    // the step).
    let from = body.translation + eye.translation;
    let ground = facing.direction();
    let Ok(view) = Dir3::new(Vec3::new(ground.x * eyes.pitch.cos(), eyes.pitch.sin(), ground.y * eyes.pitch.cos())) else {
        return;
    };
    let hit = spatial.cast_ray(from, view, TALK_REACH, true, &SpatialQueryFilter::from_excluded_entities([me]));
    let person = hit.and_then(|h| people.get(h.entity).ok().map(|(actor, name)| (h.entity, actor.0.0, name.as_str())));
    looking.0 = person.map(|(entity, id, _)| (entity, id));
    // The thing E would use (interact::pointed_thing, a step ahead of where
    // they face): a store (a bunker's, some person's `Store`) says what it
    // holds, each kind with how many (todo 11ad; any store, not only one
    // another place draws from).
    let feet = crate::interact::feet_of(body);
    let thing = crate::interact::pointed_thing(feet, feet.ground() + ground, things.iter());
    let store = thing
        .filter(|e| stores.iter().any(|s| s.0 == *e))
        .and_then(|e| containers.get(e).ok())
        .map(|held| (crate::actor::held_in(&held.0), modforge::fluid::litres_in(&held.0)));
    hud.0.prompt = match (person, store) {
        (Some((_, _, name)), _) => modforge::hud::talk_prompt(name),
        (None, Some((held, _))) if held.is_empty() => Prompt { text: "Store: empty".to_string(), can_interact: true },
        // Its water first, in litres (modforge fluid), then each kind.
        (None, Some((held, litres))) => Prompt {
            text: format!("Store: {}", std::iter::once(format!("{litres:.1} L water")).chain(held.iter().map(|(item, n)| format!("{n} {item}"))).collect::<Vec<_>>().join(", ")),
            can_interact: true,
        },
        (None, None) => Prompt::default(),
    };
}

/// The player's conversation, four times a game second, into the HUD
/// (`HudState::talking`): the last line said, as the subtitle, left up as
/// long as it takes to read (modforge `talk::reading_secs`); once their
/// line is read, whoever's turn it is speaks the line modforge picks for
/// them (`talk::pick`), else the player's choices are what matches for
/// them (`talk::choices`), and silence said by itself when its wait runs
/// out; else it has run out and ends once its last line is read.
#[allow(clippy::too_many_arguments)]
pub fn conversations(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    seed: Res<crate::WorldSeed>,
    episode: Res<Episode>,
    lines: Res<Lines>,
    world: Res<crate::terrain::WorldRes>,
    mut players: Query<(&Actor, &Transform, &Memory, &Survival, &mut Conversation, &mut Hud, &mut crate::input::Actions), With<Player>>,
    mut people: Query<(Entity, &Actor, &Transform, &Memory, Option<&Survival>, &mut crate::input::Actions, Has<crate::actor::Asked>, Option<&crate::actor::Npc>), Without<Player>>,
    episodes: Res<EpisodeRegistryRes>,
    items: Res<crate::content::ItemRegistryRes>,
) {
    if tick.0 % 15 != 0 {
        return;
    }
    let Ok((player, here, memory, survival, mut talking, mut hud, mut mine_to_push)) = players.single_mut() else {
        return;
    };
    let Some(shown) = hud.0.talking.as_mut() else {
        return;
    };
    let with = ActorId(talking.with);
    let now = tick.0;
    // The last line said between them, the subtitle: when, whether it is
    // theirs, by whom, its words; and when it has been read (modforge
    // `talk::reading_secs`, the one rule for every wait here).
    let their_name = shown.name.clone();
    let last = memory.0.done.iter().rev().find_map(|(t, d)| match d {
        modforge::memory::Did::Talked(to, l) if *t >= talking.started && *to == with => Some((*t, false, "You".to_string(), l.words.clone())),
        modforge::memory::Did::Heard(by, l) if *t >= talking.started && *by == with => Some((*t, true, their_name.clone(), l.words.clone())),
        _ => None,
    });
    if let Some((t, _, _, words)) = &last
        && talking.read.is_none_or(|(seen, _)| seen != *t)
    {
        talking.read = Some((*t, now + ticks_of(modforge::talk::reading_secs(words))));
    }
    let read_by = talking.read.map_or(now, |(_, at)| at);
    // A line of theirs still being read: nothing comes after it yet (the
    // player read their own choice as they chose it).
    // So is one heard from them just before it began (called through the
    // shut door with a knock), while it is still being read.
    let called_still_read = last.is_none() && still_reading(&memory.0, with, now);
    let reading_theirs = (last.as_ref().is_some_and(|(_, theirs, ..)| *theirs) && now < read_by) || called_still_read;
    shown.line = last.map(|(_, _, who, words)| (who, words));
    // Ending: nothing more to say, but anyone can still be asked to trade
    // until the box closes.
    if talking.ends_at.is_some() {
        shown.choices.retain(|c| matches!(c.said, Said::Trade));
        return;
    }
    let Some((them, _, there, their_memory, their_survival, mut actions, asked, their_npc)) = people.iter_mut().find(|(_, a, ..)| a.0 == with) else {
        // They are gone: it ends now.
        talking.ends_at = Some(now);
        return;
    };
    // Farther apart than a line is heard (one reach for speaking and for
    // staying in a talk, authority.md "Talking"), or a floor between them:
    // it ends now, never open with every line dropped.
    if !within(here.translation, there.translation, TALK_REACH) {
        talking.ends_at = Some(now);
        return;
    }
    let here = crate::interact::feet_of(here);
    let their_at = crate::interact::feet_of(there);
    // They stop to talk, unless something already has them going somewhere.
    if !asked {
        hold(&mut commands, them, their_at);
    }
    // They face whoever they talk to, the same Aim the brain pushes.
    actions.0.push(tick.0, Action::Aim { x: here.x, y: here.z });
    // Their line read first: then their next line, or the player's choices.
    if reading_theirs {
        shown.choices.clear();
        talking.choices_since = None;
        return;
    }
    // The player's choices: theirs is the first word in a conversation they
    // started, if they have anything to say (modforge `talk::choices`).
    let stranger = their_npc.is_some_and(|n| n.faction.is_none());
    let mine = Moment::of(&episode, (player.0, true), (with, false), here, Some(survival)).to_a_stranger(stranger);
    let open = modforge::talk::choices(&lines.0, &mine.speaking(with, &memory.0, talking.started, !talking.opened));
    let nothing_said = !memory.0.done.iter().any(|(t, d)| {
        *t >= talking.started
            && matches!(d, modforge::memory::Did::Talked(to, _) | modforge::memory::Did::Heard(to, _) if *to == with)
    });
    let player_first = talking.opened && nothing_said && !open.is_empty();
    // Their turn: the line modforge picks for them.
    let feels = modforge::relationship::feeling(&their_memory.0, player.0, |line| episodes.felt(line)).unwrap_or(0.0);
    let theirs = Moment::of(&episode, (with, false), (player.0, true), their_at, their_survival).feeling(feels).from_a_stranger(stranger);
    if !player_first
        && let Some(line) = modforge::talk::pick(&lines.0, &theirs.speaking(player.0, &their_memory.0, talking.started, talking.opened), seed.0 ^ tick.0)
    {
        actions.0.push(tick.0, Action::Talk { to: player.0.0, said: Said::Line { line: lines.0[line].id(), key: 0 } });
        shown.choices.clear();
        talking.choices_since = None;
        return;
    }
    // The player's turn: what they can say, and silence, which is never
    // shown (modforge `talk::shown_and_silence`).
    let (rows, silence) = modforge::talk::shown_and_silence(&lines.0, &open);
    // A threat is a line heard below hostile (life.md "How a person feels
    // about others"), shown <Threaten>.
    let mut choices: Vec<modforge::hud::Choice> = rows
        .into_iter()
        .filter_map(|i| {
            let home = home_of(None, true, here, world.bunker);
            let (words, _) = words_of(&lines.0[i], i, (player.0, &memory.0, home), talking.started, (&episode, &items.0), world.bunker, seed.0)?;
            Some(modforge::hud::Choice {
                said: Said::Line { line: lines.0[i].id(), key: 0 },
                words,
                threat: lines.0[i].felt < modforge::relationship::HOSTILE,
            })
        })
        .collect();
    // Anyone can be asked to trade (design.md "Trading"); whether they will
    // is how they stand toward the player (trade::trade).
    choices.push(modforge::hud::Choice { said: Said::Trade, words: "Let's trade.".to_string(), threat: false });
    if choices != shown.choices {
        shown.choices = choices;
    }
    if open.is_empty() {
        talking.choices_since = None;
        // Nobody has anything left to say: it ends once the last line and
        // the choices still shown (asking to trade) have been read.
        let shown_words: String = shown.choices.iter().map(|c| c.words.as_str()).collect::<Vec<_>>().join(" ");
        talking.ends_at = Some(read_by.max(now + ticks_of(modforge::talk::reading_secs(&shown_words))));
        return;
    }
    let since = *talking.choices_since.get_or_insert(now);
    // Chosen nothing in time: silence is said (doing nothing is a choice).
    if let Some(quiet) = silence
        && now.saturating_sub(since) >= ticks_of(f64::from(lines.0[quiet].wait))
    {
        mine_to_push.0.push(tick.0, Action::Talk { to: with.0, said: Said::Line { line: lines.0[quiet].id(), key: 0 } });
        shown.choices.clear();
        talking.choices_since = None;
    }
}

/// Someone near the player with something to say to them says it,
/// unasked, once a game second (episodes.md "Conversations": a
/// conversation starts when someone speaks): the line modforge picks for
/// them with nothing said yet, which only a line with a condition can be.
#[allow(clippy::too_many_arguments)]
pub fn speak_up(
    tick: Res<crate::SimTick>,
    seed: Res<crate::WorldSeed>,
    episode: Res<Episode>,
    lines: Res<Lines>,
    players: Query<(&Actor, &Transform, &Memory), (With<Player>, Without<Conversation>, Without<Dead>)>,
    mut people: Query<(&Actor, &Transform, &Memory, Option<&Survival>, &mut crate::input::Actions), (Without<Player>, Without<Dead>)>,
) {
    if tick.0 % 60 != 0 {
        return;
    }
    let Ok((player, here, player_memory)) = players.single() else {
        return;
    };
    let here = here.translation;
    for (actor, there, memory, survival, mut actions) in &mut people {
        let at = crate::interact::feet_of(there);
        // Not while a line of theirs the player heard (called through a
        // shut door) is still being read.
        if !within(there.translation, here, TALK_REACH) || still_reading(&player_memory.0, actor.0, tick.0) {
            continue;
        }
        let moment = Moment::of(&episode, (actor.0, false), (player.0, true), at, survival);
        // Nothing of this conversation said yet: it starts now.
        if let Some(line) = modforge::talk::pick(&lines.0, &moment.speaking(player.0, &memory.0, tick.0, false), seed.0 ^ tick.0) {            actions.0.push(tick.0, Action::Talk { to: player.0.0, said: Said::Line { line: lines.0[line].id(), key: 0 } });
            return;
        }
    }
}

/// A conversation past its end closes, and whoever stopped to talk goes
/// back to what they were doing. Escape (it clears `HudState::talking`)
/// or dying ends it too.
pub fn end_conversations(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    map: Res<ActorMapRes>,
    mut players: Query<(Entity, &mut Hud, &Conversation, Has<Dead>, &mut crate::actor::Memory), With<Player>>,
    held: Query<(), With<HeldToTalk>>,
) {
    let Ok((player, mut hud, talking, dead, mut memory)) = players.single_mut() else {
        return;
    };
    let closed = hud.0.talking.is_none();
    if closed || dead || talking.ends_at.is_some_and(|at| tick.0 >= at) {
        hud.0.talking = None;
        // Ended, as anything done is remembered (an episode's deed may
        // read it: a question left with no answer).
        memory.0.did(modforge::memory::Did::TalkEnded(ActorId(talking.with)), tick.0);
        commands.entity(player).remove::<Conversation>();
        if let Some(them) = map.0.handle_of(ActorId(talking.with))
            && held.contains(them)
        {
            let_go(&mut commands, them);
        }
    }
}

/// Word spreads (life.md "How a person feels about others"), once a game
/// second: people of one faction within talking distance of each other
/// tell each other what was done to them (modforge `talk::share`). The
/// player tells only what they choose, by talking; people with no faction
/// share nothing.
pub fn word_spreads(
    tick: Res<crate::SimTick>,
    episodes: Res<crate::content::EpisodeRegistryRes>,
    mut people: Query<(Entity, &Actor, &crate::actor::Npc, &Transform, &mut Memory), (Without<Player>, Without<Dead>)>,
) {
    if tick.0 % 60 != 0 {
        return;
    }
    let at: Vec<(Entity, ActorId, String, Vec3)> = people
        .iter()
        .filter_map(|(e, a, npc, t, _)| npc.faction.clone().map(|f| (e, a.0, f, t.translation)))
        .collect();
    for (i, (a, a_id, a_faction, a_at)) in at.iter().enumerate() {
        for (b, b_id, b_faction, b_at) in &at[i + 1..] {
            if a_faction != b_faction || !within(*a_at, *b_at, TALK_REACH * 2.0) {
                continue;
            }
            let Ok([(_, _, _, _, mut a_memory), (_, _, _, _, mut b_memory)]) = people.get_many_mut([*a, *b]) else {
                continue;
            };
            let heard = |line: &str| episodes.felt(line);
            modforge::talk::share((*a_id, &a_memory.0), (*b_id, &mut b_memory.0), heard, tick.0);
            modforge::talk::share((*b_id, &b_memory.0), (*a_id, &mut a_memory.0), heard, tick.0);
        }
    }
}
