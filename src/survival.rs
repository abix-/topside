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

/// How clean one person is (saved; modforge hygiene).
#[derive(Component, Default, Clone, Copy, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Clean(pub modforge::hygiene::Hygiene);

/// Every step everyone living gets a little dirtier (topside-town
/// "Everything works"); washing is `interact::interact_on_use`'s.
pub fn get_dirty(
    mut commands: Commands,
    time: Res<Time<Fixed>>,
    day: Res<Day>,
    mut people: Query<(Entity, Option<&mut Clean>), (With<crate::actor::Actor>, Without<Dead>, Without<crate::vehicle::Vehicle>)>,
) {
    for (person, clean) in &mut people {
        match clean {
            Some(mut clean) => clean.0.live(time.delta_secs() / day.0),
            None => {
                commands.entity(person).insert(Clean::default());
            }
        }
    }
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
    mut wearers: Query<(&mut Equipment, &Clean)>,
) {
    if tick.0 % 60 != 0 {
        return;
    }
    let today = today(&tick, *day);
    // A powered fridge or freezer keeps what is in it cold: it ages only
    // part of this second (modforge `item::keep_cold`). A powered washer
    // washes dirty clothes, a dryer dries wet ones (modforge `launder`).
    let second = 1.0 / day.0;
    for (mut held, item, powered) in &mut boxes {
        let on = powered.is_some_and(|p| p.0);
        let name = item.map(|i| i.stack.item.as_str());
        let cold = name.and_then(|n| crate::content::COLD.iter().find(|(c, _)| *c == n)).map(|(_, slows)| *slows);
        if let (Some(slows), true) = (cold, on) {
            for stack in held.0.slots.iter_mut().flatten() {
                modforge::item::keep_cold(stack, second, slows);
            }
        }
        let dry = match name {
            Some(crate::content::WASHER) => Some(false),
            Some(crate::content::DRYER) => Some(true),
            _ => None,
        };
        if let (Some(dry), true) = (dry, on) {
            for stack in held.0.slots.iter_mut().flatten() {
                if let Some(done) = modforge::hygiene::launder(&stack.item, dry) {
                    stack.item = done;
                }
            }
        }
    }
    // Whoever is filthy dirties the clothes they wear.
    for (mut worn, clean) in &mut wearers {
        if !clean.0.filthy() {
            continue;
        }
        for slot in 0..modforge::item::EQUIPMENT_SLOTS {
            let Some(dirty) = worn.0.get(slot).filter(|s| crate::content::WORN.contains(&s.item)).and_then(|s| modforge::hygiene::soiled(&s.item)) else {
                continue;
            };
            let mut stack = worn.0.take_off(slot);
            if let Some(stack) = stack.as_mut() {
                stack.item = dirty;
            }
            worn.0.swap_with(slot, &mut stack);
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
    mut actors: Query<(&mut Persona, Option<&ActorCondition>, &Transform, Option<&crate::actor::Inventory>), (Without<Dead>, Without<crate::vehicle::Vehicle>)>,
    places: Query<(&crate::interact::WorldItem, &Transform), Without<Persona>>,
    index: Res<crate::interact::ThingIndex>,
) {
    use crate::content::{COMFORT, GROUND_COMFORT, SLEEPING_BAG, SLEEPS_ON_REACH};
    let night = night(&tick, *day, &reality);
    let rates = day.rates();
    let comfort_of = |name: &str| COMFORT.iter().find(|(n, _)| *n == name).map(|(_, c)| *c);
    for (mut person, condition, at, carried) in &mut actors {
        let mut condition = condition.map_or_else(Condition::default, |c| c.0);
        condition.night = night;
        person.0.needs.tick(rates, condition, time.delta_secs());
        if !condition.asleep {
            continue;
        }
        // Asleep on the best bed or couch in reach, else on a carried
        // sleeping bag, else on the ground (modforge `slept_on`).
        // Only the things filed round them (`interact::ThingIndex`, cells
        // wider than the reach).
        let on_bed = index
            .near(at.translation)
            .filter_map(|e| places.get(e).ok())
            .filter(|(_, p)| p.translation.distance(at.translation) <= SLEEPS_ON_REACH)
            .filter_map(|(item, _)| comfort_of(&item.stack.item))
            .fold(None, |best: Option<f32>, c| Some(best.map_or(c, |b| b.max(c))));
        let bag = carried.filter(|i| i.0.count_of(SLEEPING_BAG) > 0).and_then(|_| comfort_of(SLEEPING_BAG));
        let comfort = on_bed.or(bag).unwrap_or(GROUND_COMFORT);
        person.0.needs.slept_on(rates, comfort, time.delta_secs());
    }
}
