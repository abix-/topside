//! The room's radio clicks (todo 10e4; design.md "The first hour": "a
//! radio that clicks twice an hour"): twice a game hour it clicks twice,
//! heard by the player when near it, louder the nearer. The click is a
//! short tone Bevy makes itself (`Pitch`), no sound file.

use std::time::Duration;

use bevy::audio::{Pitch, PlaybackSettings, Volume};
use bevy::prelude::*;

use crate::content::RADIO_ITEM;

/// How far the clicks are heard, metres; how high and how long one click
/// is; and how far apart the two clicks are, in ticks (Claude's numbers,
/// for the operator to change).
const HEARD_WITHIN: f32 = 20.0;
const CLICK_HZ: f32 = 1800.0;
const CLICK_MS: u64 = 25;
const BETWEEN_TICKS: u64 = 18;

/// The ticks in half a game hour, on the day's clock.
fn half_hour(day: crate::survival::Day) -> u64 {
    ((day.0 / 24.0 / 2.0) * 60.0).round().max(1.0) as u64
}

/// Every fixed step: on the half hour and a moment after it, the radio
/// clicks, wherever it lies or whoever carries it, if the player is near.
pub fn clicks(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    day: Res<crate::survival::Day>,
    pitches: Option<ResMut<Assets<Pitch>>>,
    player: Query<&Transform, With<crate::player::Player>>,
    lying: Query<(&crate::interact::WorldItem, &Transform)>,
    carried: Query<(&crate::actor::Inventory, &crate::actor::Hotbar, &Transform)>,
) {
    let Some(mut pitches) = pitches else {
        return;
    };
    let at = tick.0 % half_hour(*day);
    if at != 0 && at != BETWEEN_TICKS {
        return;
    }
    let Ok(ear) = player.single() else {
        return;
    };
    let holds = |inv: &crate::actor::Inventory, bar: &crate::actor::Hotbar| {
        inv.0.slots.iter().chain(bar.0.slots.slots.iter()).flatten().any(|s| s.item == RADIO_ITEM)
    };
    let radios = lying
        .iter()
        .filter(|(item, _)| item.stack.item == RADIO_ITEM)
        .map(|(_, t)| t.translation)
        .chain(carried.iter().filter(|(inv, bar, _)| holds(inv, bar)).map(|(_, _, t)| t.translation));
    let Some(nearest) = radios.map(|r| r.distance(ear.translation)).min_by(f32::total_cmp) else {
        return;
    };
    if nearest > HEARD_WITHIN {
        return;
    }
    let loud = 1.0 - nearest / HEARD_WITHIN;
    commands.spawn((
        AudioPlayer(pitches.add(Pitch::new(CLICK_HZ, Duration::from_millis(CLICK_MS)))),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(loud)),
    ));
    if at == 0 {
        info!("radio: two clicks, heard {nearest:.1} m away");
    }
}
