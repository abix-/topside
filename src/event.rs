//! Events inside an episode (topside-town "Storms and events are
//! different", operator, 2026-11-24): between storms, now and then
//! something happens (a loot drop, strangers coming in), picked by modforge
//! `event::pick` from content's `EVENTS` and carried out by the same
//! effects a storm's are (`storm_effect::run`). Never while the sky warns
//! or the storm rages.

use bevy::prelude::*;
use modforge::storm::StormPhase;

use crate::storm::StormRes;
use crate::survival::Day;
use crate::{SimTick, WorldSeed};

/// When the next event comes (seconds on the clock; none until the first
/// is rolled) and how many have come this episode: saved, so a load rolls
/// the same.
#[derive(Resource, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct EventsRes {
    pub at: Option<f32>,
    pub n: u32,
    /// The storm number (`StormRes::shifts`) the count is for.
    pub episode: u32,
}

/// Every fixed step: when the next event's time has come, in the calm,
/// it happens and the one after is rolled.
pub fn happen(
    mut commands: Commands,
    (tick, day, seed): (Res<SimTick>, Res<Day>, Res<WorldSeed>),
    storms: Res<StormRes>,
    mut state: ResMut<EventsRes>,
) {
    let secs = Day::secs(&tick);
    let def = &*crate::content::EVENTS;
    if state.episode != storms.shifts {
        state.episode = storms.shifts;
        state.n = 0;
        state.at = None;
    }
    let Some(at) = state.at else {
        state.at = Some(modforge::event::next_at(&def.pace, secs, day.0, seed.0 ^ u64::from(storms.shifts)));
        return;
    };
    if secs < at || crate::storm::phase(&tick, *day, &storms) != StormPhase::Calm {
        return;
    }
    if let Some(event) = modforge::event::pick(&def.kinds, storms.shifts, state.n, seed.0) {
        info!("event: {} (episode {}, the {}th)", event.name, storms.shifts, state.n);
        for effect in &event.does {
            crate::storm_effect::run(&mut commands, *effect);
        }
    }
    state.n += 1;
    state.at = Some(modforge::event::next_at(&def.pace, secs, day.0, seed.0 ^ u64::from(storms.shifts) ^ u64::from(state.n) << 20));
}
