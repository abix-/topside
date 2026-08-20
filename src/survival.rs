//! Topside's binding for modforge::survival: the `Survival`
//! component every actor carries (the player and every NPC, one
//! mechanic path) and the one tick that drains it.

use bevy::prelude::*;
pub use modforge::survival::{SurvivalRates, SurvivalStats};

use crate::bunker::GAME_DAY_SECS;

/// Hunger and thirst on one actor.
#[derive(Component, Default, Debug, PartialEq)]
pub struct Survival(pub SurvivalStats);

/// How fast everyone drains. A full stomach lasts one game day, a
/// full canteen half of one. Tunable.
#[derive(Resource, Clone, Copy)]
pub struct Rates(pub SurvivalRates);

impl Default for Rates {
    fn default() -> Self {
        Self(SurvivalRates::lasting(GAME_DAY_SECS, GAME_DAY_SECS / 2.0))
    }
}

pub fn survival_tick(
    time: Res<Time<Fixed>>,
    rates: Res<Rates>,
    mut actors: Query<&mut Survival>,
) {
    for mut survival in &mut actors {
        survival.0.drain(rates.0, time.delta_secs());
    }
}
