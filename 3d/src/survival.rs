//! Topside's binding for modforge::survival: the `Survival`
//! component every actor carries (the player and every NPC, one
//! mechanic path), the `Condition` the brain and perception set on
//! it, and the one tick that moves the needs.

use bevy::prelude::*;
pub use modforge::survival::{Condition, Need, SurvivalRates, SurvivalStats};

use crate::bunker::GAME_DAY_SECS;

/// The needs on one actor: hunger, thirst, rest, safety.
#[derive(Component, Default, Debug, PartialEq)]
pub struct Survival(pub SurvivalStats);

/// What is true of this actor right now for the needs: asleep (the
/// brain sets it), at home and a threat in sight (perception sets
/// them). Cleared to the default by nothing; whoever knows, writes.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq)]
pub struct ActorCondition(pub Condition);

/// How fast everyone's needs move. A full stomach lasts one game day,
/// a full canteen half of one, rest a day awake. Tunable.
#[derive(Resource, Clone, Copy)]
pub struct Rates(pub SurvivalRates);

impl Default for Rates {
    fn default() -> Self {
        Self(SurvivalRates::lasting(
            GAME_DAY_SECS,
            GAME_DAY_SECS / 2.0,
            GAME_DAY_SECS,
        ))
    }
}

pub fn survival_tick(
    time: Res<Time<Fixed>>,
    rates: Res<Rates>,
    mut actors: Query<(&mut Survival, Option<&ActorCondition>)>,
) {
    for (mut survival, condition) in &mut actors {
        let condition = condition.map_or_else(Condition::default, |c| c.0);
        survival.0.tick(rates.0, condition, time.delta_secs());
    }
}
