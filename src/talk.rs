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
use modforge::storyteller::LineDef;

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

/// Real seconds a conversation stays on screen after its last line, to be
/// read, before it ends. Tuning.
const READ_SECS: f64 = 4.0;

/// How far from a thing a known place can be and still be where the thing
/// is ("the well at the campsite"), in metres. Tuning.
const PLACE_OF_A_THING: f32 = 60.0;

/// The person the player is looking at within talking distance: the
/// entity and their ActorId, from a ray along the player's view.
#[derive(Component, Default, Debug, Clone, Copy)]
pub struct LookingAt(pub Option<(Entity, u64)>);

/// The player's conversation: with whom, the tick it started (only what
/// is said from then counts), whether the player started it (their E),
/// the real time their current choices showed (for silence's wait), and
/// the real time it ends at once its last line has been said. The HUD
/// shows it from `HudState::talking`.
#[derive(Component, Debug, Clone, Copy)]
pub struct Conversation {
    pub with: u64,
    pub started: u64,
    pub opened: bool,
    pub choices_since: Option<f64>,
    pub ends_at: Option<f64>,
}

impl Conversation {
    pub fn new(with: u64, started: u64, opened: bool) -> Self {
        Self { with, started, opened, choices_since: None, ends_at: None }
    }
}

/// The HUD's picture of a new conversation with this person.
pub fn talking_with(with: u64, name: &str) -> modforge::hud::Talking {
    modforge::hud::Talking { with, name: name.to_string(), ..Default::default() }
}

/// A person held where they stand while the player talks to them (they
/// stop to talk, as anyone would); let go when the conversation ends.
#[derive(Component)]
pub struct HeldToTalk;

/// Every line anyone can say now: the running episode's, then plain talk
/// (content). A `Said::Line` names its line by its place here.
#[derive(Resource, Default)]
pub struct Lines(pub Vec<LineDef>);

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
        }
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
        }
    }
}

/// A line's words and what it tells of, as this speaker says it now: the
/// thing its rule tells of (an episode thing by name, else a known thing of
/// that kind they do not keep quiet), its {slots} filled from their memory
/// (the way to an episode thing from the bunker door, the place a known
/// thing is at), one way picked from the seed, the speaker, the line, and
/// the conversation, so the panel shows what is then said.
pub fn words_of(line: &LineDef, index: usize, (speaker, memory): (ActorId, &Mind), since: u64, episode: &Episode, bunker: Vec2, seed: u64) -> Option<(String, u64)> {
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
            slots = crate::episode::the_way(memory, crate::episode::bunker_door(bunker), key, site);
        } else if let Some(thing) = memory.known.iter().find(|k| k.key == key) {
            let ground = |p: Vec3| Vec2::new(p.x, p.z);
            let place = memory
                .known
                .iter()
                .filter(|k| k.key & crate::episode::SITE != 0)
                .map(|k| (k, ground(k.position).distance(ground(thing.position))))
                .filter(|(_, d)| *d <= PLACE_OF_A_THING)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((place, _)) = place {
                slots.push(("place", place.kind.clone()));
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
    speakers: Query<(Entity, &Actor, &ThisTick, &Name), Without<Dead>>,
    mut people: Query<(&Transform, &mut Memory, &Inventory, &mut Hud, Has<Player>, Option<&Conversation>), Without<Dead>>,
) {
    for (speaker, id, this_tick, name) in &speakers {
        for said in this_tick.0.iter().filter_map(|s| match s.action {
            Action::Talk { to, said } => Some((to, said)),
            _ => None,
        }) {
            let (to, said) = said;
            let Some(listener) = map.0.handle_of(ActorId(to)) else {
                continue;
            };
            let Ok([(from, mut speaker_memory, inventory, _, _, speaker_talking), (at, mut listener_memory, _, mut hud, is_player, listener_talking)]) =
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
            let (said, words) = match said {
                Said::Line { line, key } => {
                    let Some(def) = lines.0.get(line as usize) else {
                        continue;
                    };
                    let Some((words, told)) = words_of(def, line as usize, (id.0, &speaker_memory.0), since, &episode, world.bunker, seed.0) else {
                        continue;
                    };
                    let key = if key != 0 { key } else { told };
                    (Said::Line { line, key }, Some((def.name.clone(), words)))
                }
                other => (other, None),
            };
            let answer = modforge::talk::talk(
                said,
                (id.0, &mut speaker_memory.0),
                (ActorId(to), &mut listener_memory.0),
                (offered.as_deref(), words.as_ref().map(|(n, w)| (n.as_str(), w.as_str()))),
                tick.0,
            );
            debug!("{:?} to {to}: {said:?}, {answer:?}", id.0);
            if is_player {
                spoken_to(&mut commands, listener, &mut hud, (id.0.0, name.as_str()), tick.0);
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
    hud.0.prompt = person.map_or_else(Prompt::default, |(_, _, name)| modforge::hud::talk_prompt(name));
}

/// The player's conversation, four times a game second, into the HUD
/// (`HudState::talking`): the last line said, as the subtitle; whoever's
/// turn it is speaks the line modforge picks for them (`talk::pick`), else
/// the player's choices are what matches for them (`talk::choices`), and
/// silence said by
/// itself when its wait runs out; else it has run out and ends
/// `READ_SECS` after the last word.
#[allow(clippy::too_many_arguments)]
pub fn conversations(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    real: Res<Time<Real>>,
    seed: Res<crate::WorldSeed>,
    episode: Res<Episode>,
    lines: Res<Lines>,
    world: Res<crate::terrain::WorldRes>,
    mut players: Query<(&Actor, &Transform, &Memory, &Survival, &mut Conversation, &mut Hud, &mut crate::input::Actions), With<Player>>,
    mut people: Query<(Entity, &Actor, &Transform, &Memory, Option<&Survival>, &mut crate::input::Actions, Has<crate::actor::Asked>), Without<Player>>,
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
    let now = real.elapsed_secs_f64();
    // The last line said between them, the subtitle.
    let their_name = shown.name.clone();
    shown.line = memory.0.done.iter().rev().find_map(|(t, d)| match d {
        modforge::memory::Did::Talked(to, l) if *t >= talking.started && *to == with => Some(("You".to_string(), l.words.clone())),
        modforge::memory::Did::Heard(by, l) if *t >= talking.started && *by == with => Some((their_name.clone(), l.words.clone())),
        _ => None,
    });
    if talking.ends_at.is_some() {
        shown.choices.clear();
        return;
    }
    let Some((them, _, there, their_memory, their_survival, mut actions, asked)) = people.iter_mut().find(|(_, a, ..)| a.0 == with) else {
        // They are gone: it ends now.
        talking.ends_at = Some(now);
        return;
    };
    // Walked away (or they did), or a floor between them: it ends now.
    if !within(here.translation, there.translation, TALK_REACH * 2.0) {
        talking.ends_at = Some(now);
        return;
    }
    let here = crate::interact::feet_of(here);
    let their_at = crate::interact::feet_of(there);
    // They stop to talk, unless something already has them going somewhere.
    if !asked {
        commands.entity(them).insert((crate::actor::Asked(their_at), HeldToTalk));
    }
    // They face whoever they talk to, the same Aim the brain pushes.
    actions.0.push(tick.0, Action::Aim { x: here.x, y: here.z });
    // The player's choices: theirs is the first word in a conversation they
    // started, if they have anything to say (modforge `talk::choices`).
    let mine = Moment::of(&episode, (player.0, true), (with, false), here, Some(survival));
    let open = modforge::talk::choices(&lines.0, &mine.speaking(with, &memory.0, talking.started, !talking.opened));
    let nothing_said = !memory.0.done.iter().any(|(t, d)| {
        *t >= talking.started
            && matches!(d, modforge::memory::Did::Talked(to, _) | modforge::memory::Did::Heard(to, _) if *to == with)
    });
    let player_first = talking.opened && nothing_said && !open.is_empty();
    // Their turn: the line modforge picks for them.
    let theirs = Moment::of(&episode, (with, false), (player.0, true), their_at, their_survival);
    if !player_first
        && let Some(line) = modforge::talk::pick(&lines.0, &theirs.speaking(player.0, &their_memory.0, talking.started, talking.opened), seed.0 ^ tick.0)
    {
        actions.0.push(tick.0, Action::Talk { to: player.0.0, said: Said::Line { line: line as u32, key: 0 } });
        shown.choices.clear();
        talking.choices_since = None;
        return;
    }
    // The player's turn: what they can say, and silence, which is never
    // shown (modforge `talk::shown_and_silence`).
    let (rows, silence) = modforge::talk::shown_and_silence(&lines.0, &open);
    // A threat is a line heard below hostile (life.md "How a person feels
    // about others"), shown <Threaten>.
    let choices: Vec<modforge::hud::Choice> = rows
        .into_iter()
        .filter_map(|i| {
            let (words, _) = words_of(&lines.0[i], i, (player.0, &memory.0), talking.started, &episode, world.bunker, seed.0)?;
            Some(modforge::hud::Choice {
                said: Said::Line { line: i as u32, key: 0 },
                words,
                threat: lines.0[i].felt < modforge::relationship::HOSTILE,
            })
        })
        .collect();
    if choices != shown.choices {
        shown.choices = choices;
    }
    if open.is_empty() {
        talking.choices_since = None;
        // Nobody has anything left to say: read it, then it ends.
        talking.ends_at = Some(now + READ_SECS);
        return;
    }
    let since = *talking.choices_since.get_or_insert(now);
    // Chosen nothing in time: silence is said (doing nothing is a choice).
    if let Some(quiet) = silence
        && now - since >= f64::from(lines.0[quiet].wait)
    {
        mine_to_push.0.push(tick.0, Action::Talk { to: with.0, said: Said::Line { line: quiet as u32, key: 0 } });
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
    players: Query<(&Actor, &Transform), (With<Player>, Without<Conversation>, Without<Dead>)>,
    mut people: Query<(&Actor, &Transform, &Memory, Option<&Survival>, &mut crate::input::Actions), (Without<Player>, Without<Dead>)>,
) {
    if tick.0 % 60 != 0 {
        return;
    }
    let Ok((player, here)) = players.single() else {
        return;
    };
    let here = here.translation;
    for (actor, there, memory, survival, mut actions) in &mut people {
        let at = crate::interact::feet_of(there);
        if !within(there.translation, here, TALK_REACH) {
            continue;
        }
        let moment = Moment::of(&episode, (actor.0, false), (player.0, true), at, survival);
        // Nothing of this conversation said yet: it starts now.
        if let Some(line) = modforge::talk::pick(&lines.0, &moment.speaking(player.0, &memory.0, tick.0, false), seed.0 ^ tick.0) {
            actions.0.push(tick.0, Action::Talk { to: player.0.0, said: Said::Line { line: line as u32, key: 0 } });
            return;
        }
    }
}

/// A conversation past its end closes, and whoever stopped to talk goes
/// back to what they were doing. Escape (it clears `HudState::talking`)
/// or dying ends it too.
pub fn end_conversations(
    mut commands: Commands,
    real: Res<Time<Real>>,
    map: Res<ActorMapRes>,
    mut players: Query<(Entity, &mut Hud, &Conversation, Has<Dead>), With<Player>>,
    held: Query<(), With<HeldToTalk>>,
) {
    let Ok((player, mut hud, talking, dead)) = players.single_mut() else {
        return;
    };
    let closed = hud.0.talking.is_none();
    if closed || dead || talking.ends_at.is_some_and(|at| real.elapsed_secs_f64() >= at) {
        hud.0.talking = None;
        commands.entity(player).remove::<Conversation>();
        if let Some(them) = map.0.handle_of(ActorId(talking.with))
            && held.contains(them)
        {
            commands.entity(them).remove::<(crate::actor::Asked, HeldToTalk)>();
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
            let heard = |line: &str| episodes.0.felt(line);
            modforge::talk::share((*a_id, &a_memory.0), (*b_id, &mut b_memory.0), heard, tick.0);
            modforge::talk::share((*b_id, &b_memory.0), (*a_id, &mut a_memory.0), heard, tick.0);
        }
    }
}
