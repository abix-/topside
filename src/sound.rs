//! Far sounds heard everywhere, tones Bevy makes itself (`Pitch`), no
//! sound file, as the radio's clicks (src/radio.rs): the rumble of an
//! event foretold by a sound (todo Events 3: "Something far off.
//! Listen.") and the thunder of a storm that brings weather (todo Storm 3:
//! the normal storm's "rain and thunder"). Every number is Claude's, for
//! the operator to change.

use std::time::Duration;

use bevy::audio::{Pitch, PlaybackSettings, Volume};
use bevy::prelude::*;
use modforge::event::Warning;
use modforge::storm::StormPhase;

/// The rumble of a foretold event: how low, how long, how loud, and how
/// many ticks apart while it is coming.
const RUMBLE_HZ: f32 = 70.0;
const RUMBLE_MS: u64 = 1200;
const RUMBLE_LOUD: f32 = 0.5;
const RUMBLE_EVERY: u64 = 600;

/// Thunder: how low, how long, how loud; one chance in `THUNDER_ONE_IN`
/// each `THUNDER_EVERY` ticks while the storm rages.
const THUNDER_HZ: f32 = 45.0;
const THUNDER_MS: u64 = 900;
const THUNDER_LOUD: f32 = 0.8;
const THUNDER_EVERY: u64 = 240;
const THUNDER_ONE_IN: u64 = 3;

/// Every fixed step: the rumble while a sound-foretold event is coming,
/// the thunder while a storm that brings weather rages.
pub fn far_sounds(
    mut commands: Commands,
    (tick, day, seed): (Res<crate::SimTick>, Res<crate::survival::Day>, Res<crate::WorldSeed>),
    (events, storms): (Res<crate::event::EventsRes>, Res<crate::storm::StormRes>),
    pitches: Option<ResMut<Assets<Pitch>>>,
) {
    let Some(mut pitches) = pitches else {
        return;
    };
    let mut tone = |hz: f32, ms: u64, loud: f32| {
        commands.spawn((
            AudioPlayer(pitches.add(Pitch::new(hz, Duration::from_millis(ms)))),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(loud)),
        ));
    };
    if tick.0 % RUMBLE_EVERY == 0 && events.coming.as_ref().is_some_and(|(event, _)| event.warning == Warning::Sound) {
        tone(RUMBLE_HZ, RUMBLE_MS, RUMBLE_LOUD);
    }
    if tick.0 % THUNDER_EVERY == 0
        && crate::storm::phase(&tick, *day, &storms) == StormPhase::Storm
        && crate::storm_type::coming(&storms, &seed).is_some_and(|k| k.weather.is_some())
        && modforge::roll::salted_index(seed.0 ^ tick.0, 0x7409_DE00, THUNDER_ONE_IN) == 0
    {
        tone(THUNDER_HZ, THUNDER_MS, THUNDER_LOUD);
    }
}
