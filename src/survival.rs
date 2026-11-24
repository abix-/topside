//! The binding for modforge::survival: the one tick that moves every
//! actor's needs (the player and every NPC, one path).

use bevy::prelude::*;
use modforge::survival::{Condition, SurvivalRates, day_fraction, is_night};

use modforge::combat::Hit;
use modforge::storm::StormDef;

use crate::SimTick;
use crate::actor::{ActorCondition, Dead, Persona};
use crate::combat::{Equipment, Health, Protection, hit_actor};
use crate::content::{DamageRegistryRes, GAME_DAY_SECS, ItemRegistryRes, STORM};

/// How long a game day is, in seconds: the one clock hunger, thirst,
/// rest, night, and the storm all read. The game's is `GAME_DAY_SECS`;
/// a test that must see days pass sets a short one, and the same rules
/// run on a faster clock.
#[derive(Resource, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct Day(pub f32);

impl Default for Day {
    fn default() -> Self {
        Self(GAME_DAY_SECS)
    }
}

impl Day {
    /// How fast everyone's needs move: a full stomach lasts one day, a
    /// full canteen half of one, rest a day awake.
    pub fn rates(self) -> SurvivalRates {
        SurvivalRates::lasting(self.0, self.0 / 2.0, self.0)
    }

    /// One person's hunger and thirst points a day, for counting days of
    /// food and water (modforge memory::days_of).
    pub fn per_day(self) -> (f32, f32) {
        let rates = self.rates();
        (rates.hunger_per_sec * self.0, rates.thirst_per_sec * self.0)
    }

    /// The storm on this day's clock: its warning and its length the same
    /// share of the day as the game's.
    pub fn storm(self) -> StormDef {
        let scale = self.0 / GAME_DAY_SECS;
        StormDef {
            warning_secs: STORM.warning_secs * scale,
            storm_secs: STORM.storm_secs * scale,
        }
    }

    /// Seconds since the world began at `tick`.
    pub fn secs(tick: &SimTick) -> f32 {
        tick.0 as f32 / 60.0
    }
}

/// Once a second, everyone with an empty stomach or canteen takes
/// damage through the one hit path (todo 10c): starvation and
/// dehydration, no armor helps, player and NPC alike.
pub fn waste(
    tick: Res<SimTick>,
    items: Res<ItemRegistryRes>,
    layers: Res<crate::content::LayerRegistryRes>,
    damage: Res<DamageRegistryRes>,
    mut actors: Query<(&Protection, &Equipment, &mut Health, &mut Persona), Without<Dead>>,
) {
    if tick.0 % 60 != 0 {
        return;
    }
    for (own, worn, mut health, mut person) in &mut actors {
        let needs = person.0.needs;
        for (empty, name) in [(needs.starving(), "starvation"), (needs.dehydrated(), "dehydration")] {
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
            hit_actor(&hit, own, worn, &items.0, &layers.0, Default::default(), &mut health, Some(&mut person), None, tick.0);
        }
    }
}

/// The game day it is now, counted from the world's start: what a stack's
/// made day is read against.
pub fn today(tick: &SimTick, day: Day) -> f32 {
    Day::secs(tick) / day.0
}

/// Once a second, food kept past its days turns rotten wherever it is
/// held: an inventory, a hotbar or a box (design.md "Survival
/// pressures": food spoils over time; modforge `item::spoiled`).
pub fn rot(
    tick: Res<SimTick>,
    day: Res<Day>,
    mut inventories: Query<&mut crate::actor::Inventory>,
    mut hotbars: Query<&mut crate::actor::Hotbar>,
    mut boxes: Query<(&mut crate::interact::Container, Option<&crate::interact::WorldItem>, Option<&crate::power::Powered>)>,
) {
    if tick.0 % 60 != 0 {
        return;
    }
    let today = today(&tick, *day);
    // A powered fridge or freezer keeps what is in it cold: it ages only
    // part of this second (modforge `item::keep_cold`).
    let second = 1.0 / day.0;
    for (mut held, item, powered) in &mut boxes {
        let cold = item.and_then(|i| crate::content::COLD.iter().find(|(name, _)| *name == i.stack.item)).map(|(_, slows)| *slows);
        if let (Some(slows), Some(true)) = (cold, powered.map(|p| p.0)) {
            for stack in held.0.slots.iter_mut().flatten() {
                modforge::item::keep_cold(stack, second, slows);
            }
        }
    }
    let turn = |inventory: &mut modforge::item::Inventory| {
        for stack in inventory.slots.iter_mut().flatten() {
            let Some(&(_, keeps)) = crate::content::KEEPS_DAYS.iter().find(|(item, _)| *item == stack.item) else {
                continue;
            };
            if modforge::item::spoiled(stack, today, keeps) {
                stack.item = crate::content::ROTTEN_FOOD.to_string();
            }
        }
    };
    for mut held in &mut inventories {
        turn(&mut held.0);
    }
    for mut held in &mut hotbars {
        turn(&mut held.0.slots);
    }
    for (mut held, ..) in &mut boxes {
        turn(&mut held.0);
    }
}

/// Whether the one day clock is in its night hours (todo 11e4).
pub fn night_hours(tick: &SimTick, day: Day) -> bool {
    is_night(day_fraction(Day::secs(tick), day.0))
}

/// Whether it is night now: the night hours, never in a reality without
/// night (realities.md).
pub fn night(tick: &SimTick, day: Day, reality: &crate::Reality) -> bool {
    reality.0.night && night_hours(tick, day)
}

/// Every living actor's needs move one fixed step; at night the awake
/// tire faster, so people go home and sleep.
pub fn survival_tick(
    time: Res<Time<Fixed>>,
    tick: Res<SimTick>,
    day: Res<Day>,
    reality: Res<crate::Reality>,
    // A vehicle has fuel in place of needs (todo 57).
    mut actors: Query<(&mut Persona, Option<&ActorCondition>), (Without<Dead>, Without<crate::vehicle::Vehicle>)>,
) {
    let night = night(&tick, *day, &reality);
    let rates = day.rates();
    for (mut person, condition) in &mut actors {
        let mut condition = condition.map_or_else(Condition::default, |c| c.0);
        condition.night = night;
        person.0.needs.tick(rates, condition, time.delta_secs());
    }
}
