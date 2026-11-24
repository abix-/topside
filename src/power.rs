//! Power (todo 29) on modforge's rules (modforge power.rs): a generator
//! is a box (a WorldItem with a `Container`) that burns the fuel put in
//! it; every consumer within its wiring's reach is on its grid, and the
//! grid's supply goes to them in ThingId order. A powered research
//! station researches; one without power does not (research.rs). The
//! reach, outputs and draws are content.

use bevy::prelude::*;
use modforge::power::Burning;

use crate::content::{GENERATOR, GENERATOR_DEF, RESEARCH_DRAW, WIRE_REACH};
use crate::interact::{Container, Interactable, ThingId, WorldItem};
use modforge::crafting::StationKind;
use modforge::hud::InteractKind;

/// A generator's burning (saved).
#[derive(Component, Default, Clone, Copy, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Generator(pub Burning);

/// Whether a consumer has power now: worked out every step from the
/// generators, never saved.
#[derive(Component, Default, Clone, Copy)]
pub struct Powered(pub bool);

/// What a station draws, if it needs power.
fn draw(kind: &InteractKind) -> Option<f32> {
    match kind {
        InteractKind::Station(StationKind::ResearchStation) => Some(RESEARCH_DRAW),
        _ => None,
    }
}

/// Every step: each generator burns its fuel; each consumer is powered
/// if a running generator's wiring reaches it and its supply is left.
#[allow(clippy::type_complexity)]
pub fn run(
    mut commands: Commands,
    time: Res<Time<Fixed>>,
    mut generators: Query<(Entity, &WorldItem, &Transform, &mut Container, Option<&mut Generator>, &ThingId)>,
    consumers: Query<(Entity, &Interactable, &Transform, &ThingId, Option<&Powered>)>,
    (items, fires): (Res<crate::content::ItemRegistryRes>, Query<&Powered, With<Container>>),
) {
    // The generators, in ThingId order: where each runs and its supply.
    let mut running: Vec<(u64, Vec3, f32)> = Vec::new();
    for (entity, item, at, mut fuel, generator, id) in &mut generators {
        // A fire (a fireplace, a wood stove) burns any wood put in it, on
        // the generator's burning (modforge `Burning::run`), and is lit
        // while it does (topside-town "Everything works").
        if crate::content::FIRES.contains(&item.stack.item.as_str()) {
            let Some(mut burning) = generator else {
                commands.entity(entity).insert(Generator::default());
                continue;
            };
            let wood = items.0.count_in(&fuel.0, crate::content::WOOD);
            let (used, lit) = burning.0.run(&crate::content::FIRE_DEF, wood, time.delta_secs());
            for _ in 0..used {
                let kind = fuel.0.slots.iter().flatten().find(|s| items.0.answers(&s.item, crate::content::WOOD)).map(|s| s.item.clone());
                if let Some(kind) = kind {
                    fuel.0.take_one(&kind);
                }
            }
            if fires.get(entity).map(|p| p.0).ok() != Some(lit) {
                commands.entity(entity).insert(Powered(lit));
            }
            continue;
        }
        if item.stack.item != GENERATOR {
            continue;
        }
        let Some(mut generator) = generator else {
            commands.entity(entity).insert(Generator::default());
            continue;
        };
        // A gas can or a propane tank, whichever is in it (Works 12).
        let fuels = crate::content::GENERATOR_FUELS;
        let held: u32 = fuels.iter().map(|f| fuel.0.count_of(f)).sum();
        let (used, on) = generator.0.run(&GENERATOR_DEF, held, time.delta_secs());
        for _ in 0..used {
            if let Some(f) = fuels.iter().find(|f| fuel.0.count_of(f) > 0) {
                fuel.0.take_one(f);
            }
        }
        if on {
            running.push((id.0, at.translation, GENERATOR_DEF.output));
        }
    }
    running.sort_by_key(|(id, ..)| *id);
    // The consumers, in ThingId order, each on the first grid reaching it
    // with its draw left.
    let mut wanting: Vec<(u64, Entity, Vec3, f32, bool)> = consumers
        .iter()
        .filter_map(|(entity, kind, at, id, was)| draw(&kind.kind).map(|d| (id.0, entity, at.translation, d, was.is_some_and(|p| p.0))))
        .collect();
    wanting.sort_by_key(|(id, ..)| *id);
    let mut left: Vec<f32> = running.iter().map(|(.., supply)| *supply).collect();
    for (_, entity, at, need, was) in wanting {
        let reaches: Vec<f32> = running.iter().zip(&left).map(|((_, g, _), l)| if g.distance(at) <= WIRE_REACH { *l } else { 0.0 }).collect();
        // modforge's share: powered only if the whole draw is left.
        let grid = reaches.iter().position(|l| modforge::power::share(*l, &[need])[0]);
        if let Some(grid) = grid {
            left[grid] -= need;
        }
        let on = grid.is_some();
        if on != was {
            commands.entity(entity).insert(Powered(on));
        }
    }
}

/// What a switch or a light is wired to: a light to its switch, a switch
/// to its house's breaker box (saved).
#[derive(Component, Clone, Copy, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct WiredTo(#[entities] pub Entity);

/// The town's power (topside-town "Everything works"; Project Zomboid's
/// house power): the substation, and in every house a breaker box and each
/// room's switch with its light, each through the one way a thing comes
/// into the world. Once a world, as its people; after a load they are in
/// the save.
pub fn populate_town_power(
    mut commands: Commands,
    world: Res<crate::terrain::WorldRes>,
    items: Res<crate::content::ItemRegistryRes>,
    mut ids: ResMut<crate::interact::ThingIssuer>,
    mut things_made: ResMut<crate::terrain::ThingsMade>,
    (tick, day): (Res<crate::SimTick>, Res<crate::survival::Day>),
) {
    let today = crate::survival::today(&tick, *day);
    if things_made.power {
        return;
    }
    things_made.power = true;
    let Some(town) = world.world.town.as_ref() else {
        return;
    };
    let mut make = |commands: &mut Commands, name: &str, at: Vec2, kind: Option<InteractKind>| -> Option<Entity> {
        let def = items.0.def(name)?;
        let (made, _) = crate::interact::spawn_item(commands, &mut ids, def, modforge::item::create(def, 1, &[], 0.0, 0), crate::on_ground(at, 0.0));
        match kind {
            Some(kind) => commands.entity(made).insert(Interactable { kind }),
            // Not picked up or used: it shines or it does not.
            None => commands.entity(made).remove::<Interactable>(),
        };
        Some(made)
    };
    make(&mut commands, crate::content::SUBSTATION, town.substation(), None);
    // The gas pumps, two on the first main street lot until Place 11
    // builds the gas station, each a store of fuel cans (Works 12).
    if let (Some(lot), Some(can)) = (town.lots.iter().find(|l| l.part == modforge::town::Part::MainStreet && !l.house), items.0.def(crate::content::FUEL)) {
        for side in [-2.0, 2.0] {
            let at = lot.at + lot.front * (lot.size.y / 2.0 - 2.0) + lot.front.perp() * side;
            let Some(pump) = make(&mut commands, crate::content::GAS_PUMP, at, Some(InteractKind::Container)) else {
                continue;
            };
            let mut held = modforge::item::Inventory::new(items.0.def(crate::content::GAS_PUMP).and_then(|d| d.storage).unwrap_or(0));
            // Full: a stack of cans in each slot.
            for _ in 0..held.slots.len() {
                let _ = held.add(modforge::item::create(can, can.max_stack, &[], 0.0, 0), can.max_stack);
            }
            commands.entity(pump).insert(Container(held));
        }
    }
    // The streets' lights, one each 30 m (Claude's).
    for at in town.streetlights(30.0) {
        make(&mut commands, crate::content::STREETLIGHT, at, None);
    }
    // Each house's couch in its living room, slept on as a bed is
    // (topside-town "Everything works").
    for lot in town.lots.iter().filter(|l| l.house) {
        make(&mut commands, crate::content::COUCH, lot.at + modforge::house::couch(lot.front), Some(InteractKind::Source));
    }
    // Each house's lamps, by where its people stand in the bedrooms and
    // the living room, and a candle on the dining table (Claude's).
    let lamps = town.lots.iter().filter(|l| l.house).map(|l| modforge::house::home_spots(l.front).into_iter().map(|p| l.at + p + Vec2::new(0.8, 0.0)).collect::<Vec<_>>());
    let candles = town.lots.iter().filter(|l| l.house).map(|l| l.at + modforge::house::dining_table(l.front));
    let lights: Vec<(Vec<Vec2>, Vec2)> = lamps.zip(candles).collect();
    let rooms_of = town.wiring().into_iter().zip(town.heating()).zip(lights).zip(town.kitchens()).zip(town.clothing()).zip(town.storage());
    for ((((((breaker_at, rooms), heating), (lamps, candle)), kitchen), clothing), storage) in rooms_of {
        let Some(breaker) = make(&mut commands, crate::content::BREAKER_BOX, breaker_at, Some(InteractKind::Switch { on: true })) else {
            continue;
        };
        // Its storage: boxes found holding things.
        for (kind, at) in storage {
            // The workbench is a crafting bench.
            if kind == crate::content::GARAGE_WORKBENCH {
                make(&mut commands, kind, at, Some(InteractKind::Station(StationKind::CraftingBench)));
                continue;
            }
            let Some(made) = make(&mut commands, kind, at, Some(InteractKind::Container)) else {
                continue;
            };
            let mut held = modforge::item::Inventory::new(items.0.def(kind).and_then(|d| d.storage).unwrap_or(0));
            for &(_, thing, count) in crate::content::STORAGE_HOLDS.iter().filter(|(place, ..)| *place == kind) {
                if let Some(def) = items.0.def(thing) {
                    let _ = held.add(modforge::item::create(def, count, &[], 0.0, 0), def.max_stack);
                }
            }
            commands.entity(made).insert(Container(held));
        }
        // Its clothing: closets and dressers found holding clothes; the
        // washer and the dryer on the house's breaker.
        for (kind, at) in clothing {
            let Some(made) = make(&mut commands, kind, at, Some(InteractKind::Container)) else {
                continue;
            };
            if matches!(kind, crate::content::WASHER | crate::content::DRYER) {
                commands.entity(made).insert(WiredTo(breaker));
            }
            let mut held = modforge::item::Inventory::new(items.0.def(kind).and_then(|d| d.storage).unwrap_or(0));
            for &(_, cloth, count) in crate::content::WARDROBE_HOLDS.iter().filter(|(place, ..)| *place == kind) {
                if let Some(def) = items.0.def(cloth) {
                    for _ in 0..count {
                        let _ = held.add(modforge::item::create(def, 1, &[], 0.0, 0), def.max_stack);
                    }
                }
            }
            commands.entity(made).insert(Container(held));
        }
        // Its kitchen: the stove cooks; the pantry, the fridge and the
        // freezer are boxes, found holding food; the fridge and the
        // freezer are on the house's breaker.
        for (kind, at) in kitchen {
            if kind == crate::content::STOVE {
                make(&mut commands, kind, at, Some(InteractKind::Station(StationKind::Stove)));
                continue;
            }
            let Some(made) = make(&mut commands, kind, at, Some(InteractKind::Container)) else {
                continue;
            };
            if kind != crate::content::PANTRY {
                commands.entity(made).insert(WiredTo(breaker));
            }
            let slots = items.0.def(kind).and_then(|d| d.storage).unwrap_or(0);
            let mut held = modforge::item::Inventory::new(slots);
            for &(_, food, count) in crate::content::KITCHEN_FOOD.iter().filter(|(place, ..)| *place == kind) {
                let Some(def) = items.0.def(food) else {
                    continue;
                };
                let mut stack = modforge::item::create(def, count, &[], 0.0, 0);
                stack.made_day = today;
                held.add(stack, def.max_stack);
            }
            commands.entity(made).insert(Container(held));
        }
        for at in lamps {
            if let Some(lamp) = make(&mut commands, crate::content::LAMP, at, Some(InteractKind::Switch { on: false })) {
                commands.entity(lamp).insert(WiredTo(breaker));
            }
        }
        make(&mut commands, crate::content::CANDLE, candle, Some(InteractKind::Switch { on: false }));
        // Its heating: the fires are boxes wood is put in; the furnace and
        // the air conditioner run off the thermostat, as a light off its
        // switch.
        let thermostat_at = heating.iter().find(|(kind, _)| *kind == crate::content::THERMOSTAT).map(|(_, at)| *at);
        let thermostat = thermostat_at.and_then(|at| make(&mut commands, crate::content::THERMOSTAT, at, Some(InteractKind::Switch { on: false })));
        if let Some(thermostat) = thermostat {
            commands.entity(thermostat).insert(WiredTo(breaker));
        }
        for (kind, at) in heating {
            if crate::content::FIRES.contains(&kind) {
                make(&mut commands, kind, at, Some(InteractKind::Container));
            } else if kind != crate::content::THERMOSTAT
                && let (Some(made), Some(thermostat)) = (make(&mut commands, kind, at, None), thermostat)
            {
                commands.entity(made).insert(WiredTo(thermostat));
            }
        }
        for (switch_at, light_at) in rooms {
            let switch = make(&mut commands, crate::content::LIGHT_SWITCH, switch_at, Some(InteractKind::Switch { on: false }));
            let light = make(&mut commands, crate::content::CEILING_LIGHT, light_at, None);
            if let (Some(switch), Some(light)) = (switch, light) {
                commands.entity(switch).insert(WiredTo(breaker));
                commands.entity(light).insert(WiredTo(switch));
            }
        }
    }
}

/// Every step: each light (and each furnace and air conditioner, off its
/// thermostat) is lit when modforge says (`power::lit`): its
/// switch on, its breaker on, and the breaker on a grid with power, the
/// substation's lines or a running generator's wiring.
///
/// The other lights (topside-town "Everything works": "lamps,
/// flashlights, candles, streetlights"; Project Zomboid's): a lamp is its
/// own switch on the house's power; a streetlight is lit at night while
/// the town's lines have power; a candle is lit or not, needing no power.
#[allow(clippy::type_complexity)]
pub fn lights(
    mut commands: Commands,
    (tick, day, reality): (Res<crate::SimTick>, Res<crate::survival::Day>, Res<crate::Reality>),
    things: Query<(Entity, &WorldItem, &Transform, Option<&WiredTo>, Option<&Interactable>, Option<&Powered>, Option<&Container>)>,
) {
    use crate::content::{AIR_CONDITIONER, CANDLE, CEILING_LIGHT, FURNACE, LAMP, LIGHT_LUMENS, STREETLIGHT};
    let night = crate::survival::night(&tick, *day, &reality);
    let on = |e: Entity| things.get(e).ok().and_then(|t| t.4).is_some_and(|i| matches!(i.kind, InteractKind::Switch { on: true }));
    // Where power is: the substation (its lines reach the town) and each
    // running generator (its wiring's reach).
    let sources: Vec<(Vec3, f32)> = things
        .iter()
        .filter_map(|(_, item, at, .., fuel)| match item.stack.item.as_str() {
            crate::content::SUBSTATION => Some((at.translation, crate::content::LINES_REACH)),
            // A generator runs while it has fuel (modforge `Burning::run`).
            GENERATOR if fuel.is_some_and(|f| crate::content::GENERATOR_FUELS.iter().any(|k| f.0.count_of(k) > 0)) => Some((at.translation, WIRE_REACH)),
            _ => None,
        })
        .collect();
    let has_grid = |at: Vec3| sources.iter().any(|(s, reach)| s.distance(at) <= *reach);
    // A breaker box: on, and on a grid with power.
    let breaker_live = |breaker: Entity| on(breaker) && things.get(breaker).is_ok_and(|t| has_grid(t.2.translation));
    for (entity, item, at, wired, _, was, _) in &things {
        let lit = match item.stack.item.as_str() {
            // Off its switch, the switch off its breaker.
            CEILING_LIGHT | FURNACE | AIR_CONDITIONER => wired.is_some_and(|switch| {
                let breaker = things.get(switch.0).ok().and_then(|t| t.3);
                breaker.is_some_and(|b| modforge::power::lit(true, breaker_live(b.0), on(switch.0)))
            }),
            // Its own switch, off the house's breaker.
            LAMP => wired.is_some_and(|b| modforge::power::lit(true, breaker_live(b.0), on(entity))),
            STREETLIGHT => night && has_grid(at.translation),
            // A fridge or a freezer is cold while its breaker is live.
            crate::content::FRIDGE | crate::content::FREEZER | crate::content::WASHER | crate::content::DRYER => wired.is_some_and(|b| breaker_live(b.0)),
            CANDLE => on(entity),
            _ => continue,
        };
        if was.map(|p| p.0) == Some(lit) {
            continue;
        }
        commands.entity(entity).insert(Powered(lit));
        // What shines, and how brightly; a furnace or an AC only runs.
        let Some(&(_, lumens)) = LIGHT_LUMENS.iter().find(|(name, _)| *name == item.stack.item) else {
            continue;
        };
        if lit {
            commands.entity(entity).insert(PointLight { intensity: lumens, ..default() });
        } else {
            commands.entity(entity).remove::<PointLight>();
        }
    }
}

/// A flashlight held shines from the one holding it (topside-town
/// "Everything works"), anyone alike.
pub fn flashlights(
    mut commands: Commands,
    people: Query<(Entity, &crate::actor::Hotbar, Has<PointLight>), With<crate::actor::Actor>>,
) {
    use crate::content::{FLASHLIGHT, LIGHT_LUMENS};
    let lumens = LIGHT_LUMENS.iter().find(|(name, _)| *name == FLASHLIGHT).map_or(0.0, |(_, l)| *l);
    for (person, hotbar, shining) in &people {
        let held = hotbar.0.held_stack().is_some_and(|s| s.item == FLASHLIGHT);
        if held && !shining {
            commands.entity(person).insert(PointLight { intensity: lumens, ..default() });
        } else if !held && shining {
            commands.entity(person).remove::<PointLight>();
        }
    }
}

/// Whether the research station nearest a point has power.
pub fn station_powered(at: Vec3, kind: StationKind, stations: &Query<(&Interactable, &Transform, Option<&Powered>)>) -> bool {
    stations
        .iter()
        .filter(|(i, ..)| i.kind == InteractKind::Station(kind))
        .min_by(|a, b| a.1.translation.distance(at).total_cmp(&b.1.translation.distance(at)))
        .is_some_and(|(.., p)| p.is_some_and(|p| p.0))
}
