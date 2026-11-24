//! The aliens' show (todo Show 1 to 8; topside-show), on modforge's
//! rules (modforge `show`): everyone's deeds are watched, the viewers
//! react and the chat talks, the channel runs round its cycle day and
//! night, a commercial's thing turns up in loot, and a television placed
//! and used shows it all: the star live over their shoulder (an
//! invisible camera rendering into the screen), clips, "last time on"
//! recaps edited to lie, "next time on", the viewer count, the vote, the
//! chat and their popularity. Paid for, it shows another bunker live and
//! takes a vote against them. Everyone is equal: anyone can use a
//! television; only the player's is drawn.

use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use modforge::actions::Action;
use modforge::show::{Segment, Show};

use crate::Ground;
use crate::actor::{Actor, Dead, Facing, Hotbar, Inventory, Npc, Persona, Store};
use crate::content::{BUNKER_AT, ItemRegistryRes, PAY_PER_VIEW, PAY_PER_VIEW_TOPIC, SHOW, TELEVISION, VOTE_AGAINST};
use crate::input::ThisTick;
use crate::interact::{Container, ThingId, WorldItem};
use crate::player::Player;
use crate::survival::Day;
use crate::SimTick;

/// How near a television must be to use it, metres.
const REACH: f32 = crate::interact::REACH + 1.0;
/// How far from the player's bunker another bunker's people live.
const OURS: f32 = 30.0;
/// The screen's picture, pixels.
const PICTURE: (u32, u32) = (480, 270);

/// The show's state, and whether the last commercial's thing is still
/// owed to the loot (saved).
#[derive(Resource, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct ShowRes {
    pub show: Show,
    pub owed: bool,
    /// A challenge set (todo Events 6): the tick it ends and the
    /// player's kills when it was set.
    #[serde(default)]
    pub challenge: Option<(u64, usize)>,
}

/// The challenge's reward: a loot box at the door, at this danger; how
/// far the lure is heard, metres; how near the cliffhanger brings the
/// storm, game hours (Claude's).
const REWARD_DANGER: u32 = 5;
const LURE_HEARD: f32 = 250.0;
const CLIFFHANGER_HOURS: f32 = 1.0;

/// The kills in a memory.
fn kills(persona: &Persona) -> usize {
    persona.0.memory.done.iter().filter(|(_, d)| matches!(d, modforge::memory::Did::Killed(_))).count()
}

/// The aliens' own doings (todo Events 6; modforge `event::Aliens`), each
/// through the system that owns the thing: a challenge kept here and paid
/// by `on_air`; a guest moved to the door, gone in the morning; the storm
/// brought near; the enemies about given a move to the door (modforge
/// `command`); a secret told to the player as a thing they know of.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn aliens(
    In((what, seed)): In<(modforge::event::Aliens, u64)>,
    mut commands: Commands,
    (tick, day): (Res<SimTick>, Res<Day>),
    mut state: ResMut<ShowRes>,
    mut storms: ResMut<crate::storm::StormRes>,
    shelters: Res<crate::structure::Roofs>,
    mut player: Query<(&Transform, &mut Persona), With<Player>>,
    mut people: Query<(Entity, &Actor, &mut Transform, &crate::actor::Home, Option<&Store>), (With<Npc>, Without<Dead>, Without<Player>)>,
    secrets: Query<(&ThingId, &Transform), (With<crate::secrets::Secret>, Without<Actor>, Without<Player>)>,
) {
    use modforge::event::Aliens;
    let secs = Day::secs(&tick);
    match what {
        Aliens::Challenge => {
            let before = player.single().map_or(0, |(_, p)| kills(p));
            state.challenge = Some((tick.0 + (day.0 * 60.0) as u64, before));
            state.show.say(&SHOW, "CHALLENGE: a kill before the day is out".to_string());
        }
        Aliens::Guest => {
            let mut guests: Vec<(Entity, modforge::actor::ActorId)> = people
                .iter()
                .filter(|(.., home, store)| store.is_some() && home.0.ground().distance(BUNKER_AT) >= OURS)
                .map(|(e, a, ..)| (e, a.0))
                .collect();
            guests.sort_by_key(|(_, id)| *id);
            let pick = modforge::roll::salted_index(seed, 0x6E57, guests.len().max(1) as u64) as usize;
            if let (Some(&(guest, _)), Some(at)) = (guests.get(pick), crate::event::spot(modforge::event::Place::Door, seed, 0, &shelters))
                && let Ok((_, _, mut where_, ..)) = people.get_mut(guest)
            {
                where_.translation = at;
                commands.entity(guest).insert(crate::event::TillMorning(crate::survival::today(&tick, *day) as u32));
                state.show.say(&SHOW, "a SPECIAL GUEST from another channel!".to_string());
            }
        }
        Aliens::Cliffhanger => {
            let near = day.storm().now(secs) + CLIFFHANGER_HOURS * day.0 / 24.0;
            storms.at = Some(storms.at.map_or(near, |at| at.min(near)));
            state.show.say(&SHOW, "TO BE CONTINUED...".to_string());
        }
        Aliens::Lure => {
            let door = crate::on_ground(BUNKER_AT, 0.0);
            for (entity, _, at, _, store) in &people {
                if store.is_none() && at.translation.ground().distance(BUNKER_AT) <= LURE_HEARD {
                    let to = modforge::command::Point { x: door.x, y: door.y, z: door.z };
                    commands.entity(entity).insert(crate::command::Commanded { left: [modforge::command::Command::MoveTo { to }].into(), started: false });
                }
            }
        }
        Aliens::Rumor => {
            let Ok((at, mut persona)) = player.single_mut() else {
                return;
            };
            let unknown = secrets
                .iter()
                .filter(|(id, _)| !persona.0.memory.knows(id.0))
                .min_by(|a, b| a.1.translation.distance(at.translation).total_cmp(&b.1.translation.distance(at.translation)));
            if let Some((id, there)) = unknown {
                persona.0.memory.see(id.0, "storage box", there.translation, tick.0);
                state.show.say(&SHOW, "psst... there's something out there".to_string());
            }
        }
    }
}

/// On whoever watches a television: our own channel (None), or the other
/// bunker's they paid for, by its store. Not saved: a load turns it off.
#[derive(Component, Clone, Copy, PartialEq)]
pub struct Watching(pub Option<Entity>);

/// Every step, before things are used: Use at a television turns it on
/// or off for the one using it; with the pay-per-view's price in hand
/// once it is researched, it tunes to the next other bunker's channel and
/// casts a vote against its people (Claude's). The Use is spent.
#[allow(clippy::type_complexity)]
pub fn use_television(
    mut commands: Commands,
    tick: Res<SimTick>,
    mut state: ResMut<ShowRes>,
    researched: Res<crate::research::Researched>,
    mut people: Query<(Entity, &mut ThisTick, &Transform, &Facing, &Hotbar, &mut Inventory, Option<&Watching>, Option<&Npc>, Has<Player>), Without<Dead>>,
    tvs: Query<(&Transform, &WorldItem), Without<ThisTick>>,
    bunkers: Query<(&Store, &Actor, &crate::actor::Home), (With<Npc>, Without<Dead>, Without<ThisTick>)>,
    index: Res<crate::interact::ThingIndex>,
) {
    for (me, mut this_tick, at, facing, hotbar, mut inventory, watching, npc, is_player) in &mut people {
        if !this_tick.0.iter().any(|s| s.action == Action::Use) {
            continue;
        }
        let feet = at.translation.ground();
        let ahead = feet + facing.direction();
        // Only the things filed round them (`interact::ThingIndex`).
        let near = index
            .near(at.translation)
            .filter_map(|e| tvs.get(e).ok())
            .filter(|(t, item)| item.stack.item == TELEVISION && t.translation.ground().distance(feet) <= REACH)
            .any(|(t, _)| t.translation.ground().distance(ahead) <= REACH);
        if !near {
            continue;
        }
        this_tick.0.retain(|s| s.action != Action::Use);
        let paying = hotbar.0.held_stack().is_some_and(|s| s.item == PAY_PER_VIEW.0);
        let group = crate::research::group_of(is_player, npc);
        let can_pay = researched.0.get(&group).is_some_and(|k| k.level(PAY_PER_VIEW_TOPIC) > 0) && inventory.0.count_of(PAY_PER_VIEW.0) >= PAY_PER_VIEW.1;
        if paying && can_pay {
            // The other bunkers' channels, each by its store, in the order
            // of their people's least ActorId.
            // Each store's people by ActorId, the first of each kept: its
            // least. Each person's ActorId is their own, so the order is whole.
            let mut channels: Vec<(Entity, modforge::actor::ActorId)> =
                bunkers.iter().filter(|(.., home)| home.0.ground().distance(BUNKER_AT) >= OURS).map(|(store, actor, _)| (store.0, actor.0)).collect();
            channels.sort_unstable();
            channels.dedup_by_key(|(store, _)| *store);
            channels.sort_by_key(|(_, least)| *least);
            let now = watching.and_then(|w| w.0).and_then(|s| channels.iter().position(|(c, _)| *c == s));
            let Some(&(next, _)) = channels.get(now.map_or(0, |n| n + 1) % channels.len().max(1)) else {
                continue;
            };
            for _ in 0..PAY_PER_VIEW.1 {
                inventory.0.take_one(PAY_PER_VIEW.0);
            }
            let theirs: Vec<modforge::actor::ActorId> = bunkers.iter().filter(|(s, ..)| s.0 == next).map(|(_, a, _)| a.0).collect();
            state.show.vote_against(&theirs, VOTE_AGAINST);
            commands.entity(me).insert(Watching(Some(next)));
            info!("tuned to another bunker's channel, and voted against its {} people", theirs.len());
        } else if watching.is_some() {
            commands.entity(me).remove::<Watching>();
        } else {
            commands.entity(me).insert(Watching(None));
            state.show.seen_watching(&SHOW, tick.0);
        }
    }
}

/// Every step: what everyone did this tick is watched; once a game
/// second the stir fades; when a commercial comes on its thing is sold,
/// and turns up in a loot box away from the player's bunker.
#[allow(clippy::type_complexity)]
pub fn on_air(
    mut commands: Commands,
    tick: Res<SimTick>,
    mut state: ResMut<ShowRes>,
    items: Res<ItemRegistryRes>,
    people: Query<(&Actor, &Persona, Has<Player>), (Without<Dead>, Changed<Persona>)>,
    mut boxes: Query<(&ThingId, &Transform, &mut Container), Without<Actor>>,
) {
    let state = &mut *state;
    // Only those whose memory was written since the last step: whoever
    // did something this tick is among them.
    for (actor, persona, is_player) in &people {
        for (_, did) in persona.0.memory.done.iter().rev().take_while(|(t, _)| *t == tick.0) {
            state.show.saw(&SHOW, actor.0, did, tick.0);
        }
        // The challenge (todo Events 6): a kill before it ends pays a
        // loot box at the door.
        if is_player && let Some((_, before)) = state.challenge
            && kills(persona) > before
        {
            state.challenge = None;
            state.show.say(&SHOW, "CHALLENGE DONE! a gift at the door".to_string());
            commands.run_system_cached_with(crate::event::drop, (None, 1, REWARD_DANGER, modforge::event::Place::Door, tick.0));
        }
    }
    // Past its end, the challenge is failed.
    if let Some((until, _)) = state.challenge
        && tick.0 > until
    {
        state.challenge = None;
        state.show.say(&SHOW, "challenge failed, boooo".to_string());
    }
    if tick.0 % 60 == 0 {
        state.show.fade(&SHOW);
    }
    let secs = Day::secs(&tick);
    let came_on = modforge::show::segment(&SHOW, secs) == Segment::Commercial && modforge::show::segment(&SHOW, secs - 1.0 / 60.0) != Segment::Commercial;
    if came_on && state.show.commercial(&SHOW, tick.0).is_some() {
        state.owed = true;
    }
    if !state.owed {
        return;
    }
    let Some(def) = state.show.sponsored.as_deref().and_then(|name| items.0.def(name)) else {
        state.owed = false;
        return;
    };
    let mut far: Vec<(u64, Mut<Container>)> =
        boxes.iter_mut().filter(|(_, at, _)| at.translation.ground().distance(BUNKER_AT) > OURS).map(|(id, _, held)| (id.0, held)).collect();
    far.sort_by_key(|(id, _)| *id);
    if far.is_empty() {
        return;
    }
    let pick = modforge::roll::salted_index(tick.0, 0x5_7050, far.len() as u64) as usize;
    let stack = modforge::item::create(def, 1, &[], 0.0, tick.0);
    if far[pick].1.0.add(stack, def.max_stack).is_none() {
        info!("the sponsor's {} is in a box now", def.name);
    }
    state.owed = false;
}

/// The screen drawn while the player watches.
#[derive(Component)]
pub struct Screen;
#[derive(Component)]
pub struct ScreenText;
#[derive(Component)]
pub struct ScreenPicture;
/// The invisible camera filming the show, and the channel it films.
#[derive(Component)]
pub struct ShowCamera(pub Option<Entity>);

/// Every frame: while the player watches, the screen shows the channel;
/// otherwise it is gone. Live, the picture is the show camera's: over the
/// player's shoulder on our channel, over the other bunker's first person
/// on theirs.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn screen(
    mut commands: Commands,
    tick: Res<SimTick>,
    state: Res<ShowRes>,
    events: Res<crate::event::EventsRes>,
    images: Option<ResMut<Assets<Image>>>,
    watcher: Query<(&Watching, &Actor), With<Player>>,
    eyes: Query<Entity, With<crate::player::Eyes>>,
    followed: Query<(&Actor, &Transform, &Store), (With<Npc>, Without<Dead>, Without<ShowCamera>)>,
    roots: Query<Entity, With<Screen>>,
    mut text: Query<&mut Text, With<ScreenText>>,
    mut picture: Query<&mut Visibility, With<ScreenPicture>>,
    mut cameras: Query<(Entity, &ShowCamera, &mut Transform), Without<Store>>,
) {
    let Some(mut images) = images else {
        return;
    };
    let Ok((watching, me)) = watcher.single() else {
        roots.iter().chain(cameras.iter().map(|(e, ..)| e)).for_each(|e| commands.entity(e).despawn());
        return;
    };
    // A new channel: the old screen and camera go, built again below.
    if cameras.iter().any(|(_, cam, _)| cam.0 != watching.0) {
        roots.iter().chain(cameras.iter().map(|(e, ..)| e)).for_each(|e| commands.entity(e).despawn());
        return;
    }
    if roots.is_empty() {
        let image = images.add(Image::new_target_texture(PICTURE.0, PICTURE.1, TextureFormat::Rgba8Unorm, Some(TextureFormat::Rgba8UnormSrgb)));
        let camera = (
            Camera3d::default(),
            Camera { order: -1, ..default() },
            RenderTarget::Image(image.clone().into()),
            ShowCamera(watching.0),
        );
        match (watching.0, eyes.single()) {
            // Over the shoulder: behind and to the right of the eyes.
            (None, Ok(eyes)) => {
                commands.entity(eyes).with_children(|c| {
                    c.spawn((camera, Transform::from_xyz(0.35, 0.25, 0.9)));
                });
            }
            _ => {
                commands.spawn((camera, Transform::default()));
            }
        }
        commands
            .spawn((
                Screen,
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(16.0),
                    top: Val::Px(16.0),
                    width: Val::Px(PICTURE.0 as f32),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
            ))
            .with_children(|c| {
                c.spawn((
                    ScreenPicture,
                    ImageNode::new(image),
                    Node {
                        width: Val::Px(PICTURE.0 as f32),
                        height: Val::Px(PICTURE.1 as f32),
                        ..default()
                    },
                ));
                c.spawn((ScreenText, Text::new(""), TextFont::from_font_size(14.0)));
            });
        return;
    }
    // Another bunker's channel: the camera over their first person.
    if let Some(store) = watching.0 {
        let star = followed.iter().filter(|(_, _, s)| s.0 == store).min_by_key(|(a, ..)| a.0);
        if let (Some((_, at, _)), Ok((_, _, mut cam))) = (star, cameras.single_mut()) {
            let head = at.translation + Vec3::Y * 1.5;
            *cam = Transform::from_translation(head + Vec3::new(1.5, 1.0, 1.5)).looking_at(head, Vec3::Y);
        }
    }
    let secs = Day::secs(&tick);
    let segment = modforge::show::segment(&SHOW, secs);
    for mut seen in &mut picture {
        *seen = if segment == Segment::Live { Visibility::Inherited } else { Visibility::Hidden };
    }
    let show = &state.show;
    let on = match segment {
        Segment::Live if watching.0.is_some() => "PAY-PER-VIEW: another bunker, live".to_string(),
        Segment::Live => format!("LIVE on {}", SHOW.name),
        Segment::Clip => show.clips.last().map_or("CLIP".to_string(), |c| format!("CLIP: {}", modforge::show::edit(&SHOW, &c.words))),
        Segment::Commercial => format!("A WORD FROM OUR SPONSORS: {}", show.sponsored.as_deref().unwrap_or("stay tuned")),
        Segment::LastTime => {
            let (title, lines) = show.recap(&SHOW, u64::from(events.episode));
            format!("LAST TIME ON {}: \"{title}\"\n{}", SHOW.name.to_uppercase(), lines.join("\n"))
        }
        Segment::NextTime => modforge::show::next_time(&SHOW, events.foretold().as_deref()),
    };
    // The vote: what the viewers chose over what they voted down
    // (modforge `event::choose`).
    let vote = match &events.coming {
        Some((event, _)) => format!("THE VOTE: {} over {}", event.name, event.voted_down.as_deref().unwrap_or("nothing")),
        None => "THE VOTE: open".to_string(),
    };
    let viewers = (SHOW.audience as f32 * events.director.viewers) as u32;
    if let Ok(mut text) = text.single_mut() {
        text.0 = format!(
            "{on}\nVIEWERS {viewers}   YOUR POPULARITY {:+.2}\n{vote}\n{}",
            show.popularity_of(me.0),
            show.chat.iter().map(|l| format!("> {l}")).collect::<Vec<_>>().join("\n")
        );
    }
}
