//! Events inside an episode, sent by the alien director (topside-town
//! "Storms and events are different"; topside-show "The alien director",
//! operator, 2026-11-24). Every choice is modforge's (`event::choose`,
//! the layered roll and the director); topside feeds the director what it
//! sees, foretells, and carries out each `Does`. Never while the sky warns
//! or the storm rages. Every number here is Claude's, for the operator to
//! change.

use bevy::prelude::*;
use modforge::event::{Director, Does, Event, Lasting, Leaves, Place, Warning};
use modforge::storm::StormPhase;

use crate::actor::{Actor, ActorMapRes, SpawnActor, spawn_actor};
use crate::combat::Health;
use crate::content::{BUNKER_AT, BUNKER_LEVEL, ItemRegistryRes, LayerRegistryRes};
use crate::interact::{Container, ThingIssuer, spawn_item};
use crate::player::Player;
use crate::storm::StormRes;
use crate::structure::{Shelter, sheltered};
use crate::survival::Day;
use crate::{Ground, SimTick, WorldSeed, on_ground};

/// How far out the edge is from the player's bunker, and how near its
/// door an event at the door lands.
const EDGE: f32 = 220.0;
const DOOR: (f32, f32) = (5.0, 10.0);

/// The director, the next event's time, the one foretold and coming,
/// when the last trouble came, and what lasts in the air (each with the
/// second it ends). Saved, so a load goes on the same.
#[derive(Resource, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct EventsRes {
    pub at: Option<f32>,
    pub n: u32,
    /// The storm number (`StormRes::shifts`) the count is for.
    pub episode: u32,
    pub director: Director,
    pub coming: Option<(Event, f32)>,
    pub last_trouble: f32,
    pub lasting: Vec<(Lasting, f32)>,
}

impl EventsRes {
    /// Whether `what` lasts in the air now.
    pub fn lasts(&self, what: Lasting, secs: f32) -> bool {
        self.lasting.iter().any(|(w, until)| *w == what && secs < *until)
    }

    /// The weather an event set, the last one still lasting at `secs`:
    /// it is the weather everywhere (src/weather.rs), so it does what that
    /// weather does to those outside and to sight.
    pub fn weather(&self, secs: f32) -> Option<modforge::weather::Weather> {
        self.lasting.iter().rev().find_map(|(w, until)| match w {
            Lasting::Weather(weather) if secs < *until => Some(*weather),
            _ => None,
        })
    }

    /// What the player is told of the event foretold and coming (todo
    /// "Events 3").
    pub fn foretold(&self) -> Option<String> {
        self.coming.as_ref().and_then(|(event, _)| event.foretold())
    }

    /// Whether the coming event is foretold by a light in the sky.
    pub fn foretold_by_sky(&self) -> bool {
        self.coming.as_ref().is_some_and(|(event, _)| event.warning == Warning::Sky)
    }
}

/// Every fixed step: the director feels the show, and when the next
/// event's time has come in the calm, chooses one; a foretold one is told
/// first and comes a while after.
#[allow(clippy::too_many_arguments)]
pub fn happen(
    mut commands: Commands,
    (tick, day, seed, reality): (Res<SimTick>, Res<Day>, Res<WorldSeed>, Res<crate::Reality>),
    storms: Res<StormRes>,
    mut state: ResMut<EventsRes>,
    player: Query<&Health, With<Player>>,
    show: Res<crate::show::ShowRes>,
) {
    let secs = Day::secs(&tick);
    let def = &*crate::content::EVENTS;
    if state.episode != storms.shifts {
        state.episode = storms.shifts;
        state.n = 0;
        state.at = None;
        state.coming = None;
        state.lasting.clear();
        state.last_trouble = secs;
    }
    let hurt = player.single().map_or(0.0, |h| 1.0 - h.0.current / h.0.max.max(1.0));
    let quiet = (secs - state.last_trouble) / day.0;
    state.director.feel(quiet, hurt);
    // What the viewers just saw stirs them (todo Show 3 and 6, modforge
    // `show`): a killing lifts the ratings, a dull stretch lets them fall.
    state.director.viewers = show.show.ratings(state.director.viewers);
    state.director.boredom = 1.0 - state.director.viewers;
    let salt = seed.0 ^ u64::from(storms.shifts) << 40 ^ u64::from(state.n) << 20;
    let Some(at) = state.at else {
        state.at = Some(modforge::event::next_at(&def.pace, secs, day.0, salt));
        return;
    };
    if crate::storm::phase(&tick, *day, &storms) != StormPhase::Calm {
        return;
    }
    if let Some((event, when)) = state.coming.clone() {
        if secs >= when {
            state.coming = None;
            send(&mut commands, &mut state, event, secs, day.0);
        }
        return;
    }
    if secs < at {
        return;
    }
    let night = crate::survival::night(&tick, *day, &reality);
    state.n += 1;
    state.at = Some(modforge::event::next_at(&def.pace, secs, day.0, salt ^ 0x5EED));
    let Some(event) = modforge::event::choose(def, &state.director, storms.shifts, night, salt) else {
        return;
    };
    if event.warning == Warning::None {
        send(&mut commands, &mut state, event, secs, day.0);
    } else {
        info!("event foretold ({:?}): {} {:?}", event.warning, event.name, event.twists);
        state.coming = Some((event, secs + def.pace.lead_hours * day.0 / 24.0));
    }
}

/// The event happens: each of its doings at each of its places (a game
/// day being `day_secs`).
fn send(commands: &mut Commands, state: &mut EventsRes, event: Event, secs: f32, day_secs: f32) {
    info!(
        "event: {} {:?} at {:?}, episode {}, the viewers at {:.2} voted down {:?}",
        event.name, event.twists, event.places, state.episode, state.director.viewers, event.voted_down
    );
    state.director.sent(&crate::content::EVENTS, &event);
    if event.mood == modforge::event::Mood::Trouble {
        state.last_trouble = secs;
    }
    for (k, place) in event.places.iter().enumerate() {
        for (n, does) in event.does.iter().enumerate() {
            let seed = event.seed ^ (k as u64) << 48 ^ (n as u64) << 32;
            match does {
                Does::Arrive { who, count, leaves } => {
                    commands.run_system_cached_with(arrive, (who.first().cloned().unwrap_or_default(), count.0, *leaves, *place, seed));
                }
                Does::Drop { item, count, danger } => {
                    commands.run_system_cached_with(drop, (item.clone(), count.0, *danger, *place, seed));
                }
                // What lasts in the air is everywhere, once.
                Does::Last { what, hours } if k == 0 => state.lasting.push((*what, secs + hours.0 * day_secs / 24.0)),
                Does::Last { .. } => {}
                Does::Spoil if k == 0 => commands.run_system_cached(spoil),
                Does::Spoil => {}
                Does::Storm(effect) if k == 0 => crate::storm_effect::run(commands, *effect),
                Does::Storm(_) => {}
                Does::Home(home) if k == 0 => commands.run_system_cached_with(crate::home::trouble, (*home, seed)),
                Does::Home(_) => {}
                Does::Aliens(what) if k == 0 => commands.run_system_cached_with(crate::show::aliens, (*what, seed)),
                Does::Aliens(_) => {}
            }
        }
    }
}

/// The `n`th spot at `place`, rolled from `seed`: on open ground for the
/// edge, the town and the door; inside and under the player's bunker by
/// its levels.
pub(crate) fn spot(place: Place, seed: u64, n: u32, shelters: &Query<&Shelter>) -> Option<Vec3> {
    let roll = |k: u64| modforge::roll::salted_index(seed ^ u64::from(n) << 16, 0xE7_5070 + k, 10_001) as f32 / 10_000.0;
    let around = |far: (f32, f32)| {
        (0..16u64).find_map(|t| {
            let angle = roll(t * 2) * std::f32::consts::TAU;
            let at = BUNKER_AT + Vec2::from_angle(angle) * (far.0 + (far.1 - far.0) * roll(t * 2 + 1));
            (!sheltered(shelters, at)).then(|| on_ground(at, 0.0))
        })
    };
    match place {
        Place::Edge => around((EDGE, EDGE + 20.0)),
        Place::Town => crate::storm_effect::drop_point(seed, 0xE7_5100, n, shelters).map(|at| on_ground(at, 0.0)),
        Place::Door => around(DOOR),
        Place::Inside => Some(on_ground(BUNKER_AT + Vec2::new(roll(40) - 0.5, roll(41) - 0.5) * 4.0, -BUNKER_LEVEL)),
        Place::Under => Some(on_ground(BUNKER_AT + Vec2::new(roll(42) - 0.5, roll(43) - 0.5) * 4.0, -2.0 * BUNKER_LEVEL)),
    }
}

/// People or animals arrive: `count` of the actor kind `who` at `place`.
fn arrive(
    In((who, count, leaves, place, seed)): In<(String, u32, Leaves, Place, u64)>,
    mut spawn: SpawnActor,
    actors: Res<crate::content::ActorRegistryRes>,
    (tick, day): (Res<SimTick>, Res<Day>),
    shelters: Query<&Shelter>,
) {
    let Some(def) = actors.0.def(&who).cloned() else {
        warn!("event: no actor kind {who}");
        return;
    };
    let came = crate::survival::today(&tick, *day) as u32;
    for n in 0..count {
        if let Some(at) = spot(place, seed, n, &shelters)
            && let Some(one) = spawn_actor(&mut spawn, &def, at, None)
            && leaves == Leaves::Morning
        {
            spawn.commands.entity(one).insert(TillMorning(came));
        }
    }
}

/// Things drop at `place`: `count` loot boxes filled at `danger`, or one
/// stack of `count` of `item`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn drop(
    In((item, count, danger, place, seed)): In<(Option<String>, u32, u32, Place, u64)>,
    mut commands: Commands,
    mut ids: ResMut<ThingIssuer>,
    items: Res<ItemRegistryRes>,
    layers: Res<LayerRegistryRes>,
    (tick, day): (Res<SimTick>, Res<Day>),
    shelters: Query<&Shelter>,
) {
    if let Some(name) = item {
        let Some(def) = items.0.def(&name) else {
            warn!("event: no item {name}");
            return;
        };
        if let Some(at) = spot(place, seed, 0, &shelters) {
            let stack = modforge::item::create(def, count, &[], 0.0, seed);
            spawn_item(&mut commands, &mut ids, def, stack, at);
        }
        return;
    }
    let Some(box_def) = items.0.def("storage box") else {
        return;
    };
    let table = crate::content::loot_table();
    let today = crate::survival::today(&tick, *day);
    for n in 0..count {
        let Some(at) = spot(place, seed, n, &shelters) else {
            continue;
        };
        let box_seed = seed ^ u64::from(n) << 40;
        let contents = crate::structure::fill_box(&items, &layers, &table, box_def, danger, box_seed, today);
        let stack = modforge::item::create(box_def, 1, &[], 0.0, box_seed);
        let (entity, _) = spawn_item(&mut commands, &mut ids, box_def, stack, at);
        commands.entity(entity).insert(Container(contents));
    }
}

/// The food in the player's store goes bad: every box under a roof by
/// the player's bunker.
fn spoil(shelters: Query<&Shelter>, mut boxes: Query<(&Transform, &mut Container)>) {
    for (at, mut held) in &mut boxes {
        let ground = at.translation.ground();
        if ground.distance(BUNKER_AT) > 30.0 || !sheltered(&shelters, ground) {
            continue;
        }
        for stack in held.0.slots.iter_mut().flatten() {
            if crate::content::KEEPS_DAYS.iter().any(|(item, _)| *item == stack.item) {
                stack.item = crate::content::ROTTEN_FOOD.to_string();
            }
        }
    }
}

/// Every game second: what lasted in the air and has ended goes. A
/// weather is the weather while it lasts (src/weather.rs, which hurts
/// those outside through exposure.rs and closes the fog); the dark is
/// drawn (hud `render_sky`).
pub fn lasting(tick: Res<SimTick>, mut state: ResMut<EventsRes>) {
    if tick.0 % 60 != 0 || state.lasting.is_empty() {
        return;
    }
    let secs = Day::secs(&tick);
    state.lasting.retain(|(_, until)| secs < *until);
}

/// What an event brought that leaves in the morning, and the day it
/// came (saved).
#[derive(Component, Clone, Copy, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct TillMorning(pub u32);

/// Every step: what came till morning leaves once a morning after the
/// day it came has come.
pub fn leave_in_the_morning(
    mut commands: Commands,
    (tick, day, reality): (Res<SimTick>, Res<Day>, Res<crate::Reality>),
    mut map: ResMut<ActorMapRes>,
    guests: Query<(Entity, &TillMorning, Option<&Actor>)>,
) {
    if crate::survival::night(&tick, *day, &reality) {
        return;
    }
    let today = crate::survival::today(&tick, *day) as u32;
    for (entity, came, actor) in &guests {
        if today > came.0 {
            crate::combat::leave(&mut commands, &mut map, entity, actor);
        }
    }
}
