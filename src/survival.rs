//! The binding for modforge::survival: the one tick that moves every
//! actor's needs (the player and every NPC, one path).

use bevy::prelude::*;
use modforge::survival::{Condition, SurvivalRates};

use modforge::combat::Hit;

use crate::SimTick;
use crate::actor::{ActorCondition, Dead, Survival};
use crate::combat::{Equipment, Health, Protection, hit_actor};
use crate::content::{DamageRegistryRes, GAME_DAY_SECS, ItemRegistryRes};

/// How fast everyone's needs move: a full stomach lasts one game day, a
/// full canteen half of one, rest a day awake.
#[derive(Resource, Clone, Copy)]
pub struct Rates(pub SurvivalRates);

impl Default for Rates {
    fn default() -> Self {
        Self(SurvivalRates::lasting(GAME_DAY_SECS, GAME_DAY_SECS / 2.0, GAME_DAY_SECS))
    }
}

/// Once a second, everyone with an empty stomach or canteen takes
/// damage through the one hit path (todo 10c): starvation and
/// dehydration, no armor helps, player and NPC alike.
pub fn waste(
    tick: Res<SimTick>,
    items: Res<ItemRegistryRes>,
    damage: Res<DamageRegistryRes>,
    mut actors: Query<(&Survival, &Protection, &Equipment, &mut Health), Without<Dead>>,
) {
    if tick.0 % 60 != 0 {
        return;
    }
    for (needs, own, worn, mut health) in &mut actors {
        for (empty, name) in [(needs.0.starving(), "starvation"), (needs.0.dehydrated(), "dehydration")] {
            if !empty {
                continue;
            }
            let Some(def) = damage.0.def(name) else {
                continue;
            };
            let hit = Hit {
                def,
                self_inflicted: false,
                distance: 0.0,
                location_scale: 1.0,
            };
            hit_actor(&hit, own, worn, &items.0, &mut health);
        }
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
