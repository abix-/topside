//! The binding for modforge::survival: the one tick that moves every
//! actor's needs (the player and every NPC, one path).

use bevy::prelude::*;
use modforge::survival::{Condition, SurvivalRates};

use crate::actor::{ActorCondition, Dead, Survival};
use crate::content::GAME_DAY_SECS;

/// How fast everyone's needs move: a full stomach lasts one game day, a
/// full canteen half of one, rest a day awake.
#[derive(Resource, Clone, Copy)]
pub struct Rates(pub SurvivalRates);

impl Default for Rates {
    fn default() -> Self {
        Self(SurvivalRates::lasting(GAME_DAY_SECS, GAME_DAY_SECS / 2.0, GAME_DAY_SECS))
    }
}

/// Every living actor's needs move one fixed step.
pub fn survival_tick(
    time: Res<Time<Fixed>>,
    rates: Res<Rates>,
    mut actors: Query<(&mut Survival, Option<&ActorCondition>), Without<Dead>>,
) {
    for (mut survival, condition) in &mut actors {
        let condition = condition.map_or_else(Condition::default, |c| c.0);
        survival.0.tick(rates.0, condition, time.delta_secs());
    }
}
