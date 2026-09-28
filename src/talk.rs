//! The one talk path (operator, 2026-09-27): every person's `Talk` this
//! tick, the player's and the brain's alike, reaches one person in reach;
//! modforge `talk` decides what comes of it and writes both memories.
//!
//! Conversations (episodes.md "Conversations"): the player looks at a
//! person and presses E, or someone speaks to them; the moment's choices;
//! the other person's reply; then it ends and the panel closes itself.

use avian3d::prelude::*;
use bevy::prelude::*;
use modforge::actions::Said;
use modforge::actor::ActorId;
use modforge::hud::{OpenPanel, Prompt};
use modforge::memory::Did;

use crate::Ground;
use crate::actor::{Actor, ActorMapRes, Dead, Inventory, Memory};
use crate::input::{Action, ThisTick};
use crate::interact::Hud;
use crate::player::{Eyes, Player};

/// How far away a person can be talked to, in metres.
pub const TALK_REACH: f32 = 3.0;

/// Real seconds a conversation stays on screen after its last line, to be
/// read, before it ends. Tuning.
const READ_SECS: f64 = 4.0;

/// The person the player is looking at within talking distance: the
/// entity and their ActorId, from a ray along the player's view.
#[derive(Component, Default, Debug, Clone, Copy)]
pub struct LookingAt(pub Option<(Entity, u64)>);

/// The player's conversation: with whom, the tick it started (only what
/// is said from then counts), and the real time it ends at once its last
/// line has been said.
#[derive(Component, Debug, Clone, Copy)]
pub struct Conversation {
    pub with: u64,
    pub started: u64,
    pub ends_at: Option<f64>,
}

/// A person held where they stand while the player talks to them (they
/// stop to talk, as anyone would); let go when the conversation ends.
#[derive(Component)]
pub struct HeldToTalk;

/// Every person's `Talk` this tick: the listener, by ActorId, alive and
/// within `TALK_REACH`; the offered item from the speaker's inventory; a
/// line's words from the episode that wrote them. Spoken to, the player is
/// in a conversation with the speaker.
pub fn talk(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    map: Res<ActorMapRes>,
    episode: Res<crate::Episode>,
    speakers: Query<(Entity, &Actor, &ThisTick), Without<Dead>>,
    mut people: Query<(&Transform, &mut Memory, &Inventory, &mut Hud, Has<Player>), Without<Dead>>,
) {
    for (speaker, id, this_tick) in &speakers {
        for said in this_tick.0.iter().filter_map(|s| match s.action {
            Action::Talk { to, said } => Some((to, said)),
            _ => None,
        }) {
            let (to, said) = said;
            let Some(listener) = map.0.handle_of(ActorId(to)) else {
                continue;
            };
            let Ok([(from, mut speaker_memory, inventory, _, _), (at, mut listener_memory, _, mut hud, is_player)]) =
                people.get_many_mut([speaker, listener])
            else {
                continue;
            };
            if from.translation.ground().distance(at.translation.ground()) > TALK_REACH {
                continue;
            }
            let offered = match said {
                Said::Offer { slot } => inventory.0.slots.get(slot).and_then(Option::as_ref).map(|s| s.item.clone()),
                _ => None,
            };
            // A line's words, held by the episode that wrote them.
            let words = match said {
                Said::Line { line, .. } => episode.spoken.get(line as usize).map(String::as_str),
                _ => None,
            };
            let answer = modforge::talk::talk(
                said,
                (id.0, &mut speaker_memory.0),
                (ActorId(to), &mut listener_memory.0),
                (offered.as_deref(), words),
                tick.0,
            );
            debug!("{:?} to {to}: {said:?}, {answer:?}", id.0);
            if is_player && !(hud.0.open_panel == OpenPanel::Talk && hud.0.talking_to == Some(id.0.0)) {
                hud.0.open_panel = OpenPanel::Talk;
                hud.0.talking_to = Some(id.0.0);
                commands.entity(listener).insert(Conversation { with: id.0.0, started: tick.0, ends_at: None });
            }
        }
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

/// The player's conversation, once a game second: with nothing to talk
/// about, the other person says a short line of theirs; once they have
/// spoken after the player's last word and no choice to them is open, it
/// ends `READ_SECS` later.
#[allow(clippy::too_many_arguments)]
pub fn conversations(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    real: Res<Time<Real>>,
    seed: Res<crate::WorldSeed>,
    mut episode: ResMut<crate::Episode>,
    mut players: Query<(&Actor, &Transform, &Memory, &mut Conversation), With<Player>>,
    mut people: Query<(Entity, &Actor, &Transform, &mut crate::input::Actions, Has<crate::actor::Asked>), Without<Player>>,
) {
    if tick.0 % 15 != 0 {
        return;
    }
    let Ok((player, here, memory, mut talking)) = players.single_mut() else {
        return;
    };
    if talking.ends_at.is_some() {
        return;
    }
    let with = ActorId(talking.with);
    // Walked away (or they did): it ends now; any choice with them stays
    // open for the next time.
    let them = people.iter().find(|(_, a, ..)| a.0 == with).map(|(e, _, there, _, asked)| (e, there.translation.ground(), asked));
    let apart = them.is_none_or(|(_, there, _)| there.distance(here.translation.ground()) > TALK_REACH * 2.0);
    if apart {
        talking.ends_at = Some(real.elapsed_secs_f64());
        return;
    }
    // They stop to talk, unless something already has them going somewhere.
    if let Some((them, there, false)) = them {
        commands.entity(them).insert((crate::actor::Asked(there), HeldToTalk));
    }
    // They face whoever they talk to, the same Aim the brain pushes.
    let face = here.translation.ground();
    if let Some((_, _, _, mut actions, _)) = people.iter_mut().find(|(_, a, ..)| a.0 == with) {
        actions.0.push(tick.0, Action::Aim { x: face.x, y: face.y });
    }
    let choices_open = episode.choices.iter().any(|(_, _, to)| *to == with);
    let since = |t: &u64| *t >= talking.started;
    let last_heard = memory.0.done.iter().rev().find_map(|(t, d)| (since(t) && matches!(d, Did::Heard(by, _) if *by == with)).then_some(*t));
    let last_said = memory.0.done.iter().rev().find_map(|(t, d)| (since(t) && matches!(d, Did::Talked(to, _) if *to == with)).then_some(*t));
    if choices_open {
        return;
    }
    match last_heard {
        // They have had the last word: read it, then it ends.
        Some(heard) if last_said.is_none_or(|said| heard >= said) => {
            talking.ends_at = Some(real.elapsed_secs_f64() + READ_SECS);
        }
        // Nothing to talk about: a short line of theirs.
        None => {
            let ways: Vec<String> = crate::content::NOTHING_TO_SAY.iter().map(|w| w.to_string()).collect();
            if let Some(words) = modforge::storyteller::say(&ways, &[], seed.0 ^ with.0, tick.0)
                && let Some((_, _, _, mut actions, _)) = people.iter_mut().find(|(_, a, ..)| a.0 == with)
            {
                let line = episode.speak(words);
                actions.0.push(tick.0, Action::Talk { to: player.0.0, said: Said::Line { line, key: 0 } });
            }
        }
        _ => {}
    }
}

/// A conversation past its end closes, panel and all, and whoever stopped
/// to talk goes back to what they were doing. The player closing the
/// panel (Escape) or dying ends it too.
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
    let closed = hud.0.open_panel != OpenPanel::Talk;
    if closed || dead || talking.ends_at.is_some_and(|at| real.elapsed_secs_f64() >= at) {
        if hud.0.open_panel == OpenPanel::Talk {
            hud.0.open_panel = OpenPanel::None;
        }
        hud.0.talking_to = None;
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
    let at: Vec<(Entity, ActorId, String, Vec2)> = people
        .iter()
        .filter_map(|(e, a, npc, t, _)| npc.faction.clone().map(|f| (e, a.0, f, t.translation.ground())))
        .collect();
    for (i, (a, a_id, a_faction, a_at)) in at.iter().enumerate() {
        for (b, b_id, b_faction, b_at) in &at[i + 1..] {
            if a_faction != b_faction || a_at.distance(*b_at) > TALK_REACH * 2.0 {
                continue;
            }
            let Ok([(_, _, _, _, mut a_memory), (_, _, _, _, mut b_memory)]) = people.get_many_mut([*a, *b]) else {
                continue;
            };
            let heard = |w: &str| episodes.0.felt(w);
            modforge::talk::share((*a_id, &a_memory.0), (*b_id, &mut b_memory.0), heard, tick.0);
            modforge::talk::share((*b_id, &b_memory.0), (*a_id, &mut a_memory.0), heard, tick.0);
        }
    }
}
