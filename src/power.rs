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
/// Each house's front door, once when it is made: locked with the
/// house's lock (Works 14). A door the save brings back is never locked
/// again (`ThingsMade::locked`).
pub fn lock_front_doors(
    world: Option<Res<crate::terrain::WorldRes>>,
    mut things_made: ResMut<crate::terrain::ThingsMade>,
    mut doors: Query<(&crate::interact::Door, &crate::interact::ThingId, &mut Interactable), Added<crate::interact::Door>>,
) {
    let Some(town) = world.as_ref().and_then(|w| w.world.town.as_ref()) else {
        return;
    };
    if doors.is_empty() {
        return;
    }
    let fronts: Vec<Vec2> = town.lots.iter().filter(|l| l.house).map(|l| l.at + modforge::house::front_door(l.front)).collect();
    for (door, id, mut thing) in &mut doors {
        if door.lying || !things_made.locked.insert(id.0) {
            continue;
        }
        if let Some(house) = fronts.iter().position(|f| f.distance(door.closed_center) < 1.0) {
            thing.kind = InteractKind::Locked { lock: crate::content::front_lock(house), door: true };
        }
    }
}

/// Once a day at the collection hour: every trash can emptied into the
/// dump's piles, in ThingId order, the first pile with room first
/// (modforge `trash`, Works 17).
pub fn collect_trash(
    mut things_made: ResMut<crate::terrain::ThingsMade>,
    (tick, day): (Res<crate::SimTick>, Res<crate::survival::Day>),
    items: Res<crate::content::ItemRegistryRes>,
    mut boxes: Query<(&crate::interact::WorldItem, &crate::interact::ThingId, &mut Container)>,
) {
    let today = crate::survival::today(&tick, *day);
    if !modforge::trash::due(today, crate::content::COLLECTION_HOUR, things_made.collected) {
        return;
    }
    things_made.collected = today.floor() as u32;
    let mut cans: Vec<(u64, modforge::item::Inventory)> = Vec::new();
    for (item, id, mut held) in &mut boxes {
        if item.stack.item == crate::content::TRASH_CAN {
            cans.push((id.0, std::mem::replace(&mut held.0, modforge::item::Inventory::new(0))));
        }
    }
    cans.sort_by_key(|c| c.0);
    let max_stack = |name: &str| items.0.def(name).map_or(1, |d| d.max_stack);
    let mut piles: Vec<_> = boxes.iter_mut().filter(|(item, ..)| item.stack.item == crate::content::DUMP_PILE).collect();
    piles.sort_by_key(|p| p.1.0);
    for (_, can) in cans.iter_mut() {
        for (_, _, pile) in piles.iter_mut() {
            modforge::trash::collect(can, &mut pile.0, max_stack);
        }
    }
    drop(piles);
    // The cans back, with what did not fit.
    let mut back: std::collections::HashMap<u64, modforge::item::Inventory> = cans.into_iter().collect();
    for (item, id, mut held) in &mut boxes {
        if item.stack.item == crate::content::TRASH_CAN && let Some(can) = back.remove(&id.0) {
            held.0 = can;
        }
    }
    info!("the trash was taken to the dump");
}

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
    // A smoke detector in each house's hall, and a hydrant each 90 m of
    // street (Works 15).
    for lot in town.lots.iter().filter(|l| l.house) {
        make(&mut commands, crate::content::SMOKE_DETECTOR, lot.at + modforge::house::smoke_detector(lot.front), None);
        // Its garden beds, in the back yard (Works 18).
        for at in modforge::house::garden(lot.front) {
            make(&mut commands, crate::content::GARDEN_BED, lot.at + at, None);
        }
    }
    // Each house's TV, radio and phone, off; its mailbox at the street,
    // holding a letter (Works 16).
    let letter = items.0.def("note");
    for (house, lot) in town.lots.iter().filter(|l| l.house).enumerate() {
        for (kind, at) in modforge::house::den(lot.front) {
            if kind == crate::content::TRASH_CAN {
                if let Some(made) = make(&mut commands, kind, lot.at + at, Some(InteractKind::Container)) {
                    let room = items.0.def(kind).and_then(|d| d.storage).unwrap_or(0);
                    commands.entity(made).insert(Container(modforge::item::Inventory::new(room)));
                }
                continue;
            }
            if kind != crate::content::MAILBOX {
                make(&mut commands, kind, lot.at + at, Some(InteractKind::Switch { on: false }));
                continue;
            }
            let Some(made) = make(&mut commands, kind, lot.at + at, Some(InteractKind::Container)) else {
                continue;
            };
            let mut held = modforge::item::Inventory::new(items.0.def(kind).and_then(|d| d.storage).unwrap_or(0));
            if let Some(def) = letter {
                let mut stack = modforge::item::create(def, 1, &[], 0.0, 0);
                let words = crate::content::LETTERS[house % crate::content::LETTERS.len()];
                stack.note = Some(modforge::item::Note { text: words.to_string(), signed: String::new() });
                let _ = held.add(stack, 1);
            }
            commands.entity(made).insert(Container(held));
        }
    }
    // Every place's things (topside-town "What the town has"): a station,
    // or a box found holding its stock.
    for (place, thing, at) in town.place_things() {
        if let Some(&(_, station)) = crate::content::PLACE_STATIONS.iter().find(|(n, _)| *n == thing) {
            make(&mut commands, thing, at, Some(InteractKind::Station(station)));
            continue;
        }
        // A switch (a substation's breaker, a radio), turned on and off;
        // the substation's are found on.
        if crate::content::PLACE_SWITCHES.contains(&thing) {
            make(&mut commands, thing, at, Some(InteractKind::Switch { on: thing == crate::content::SUBSTATION_SWITCH }));
            continue;
        }
        // A thing carried off (an extinguisher), picked up.
        if items.0.def(thing).is_some_and(|d| d.storage.is_none() && matches!(d.kind, modforge::item::ItemKind::Tool | modforge::item::ItemKind::Medicine | modforge::item::ItemKind::Weapon)) {
            make(&mut commands, thing, at, Some(InteractKind::Pickup));
            continue;
        }
        // A source (a pump, a tap), used where it stands.
        if items.0.def(thing).is_some_and(|d| d.kind == modforge::item::ItemKind::Source) {
            make(&mut commands, thing, at, Some(InteractKind::Source));
            continue;
        }
        // A field's bed, found sown and ripe where the place says.
        if thing == crate::content::GARDEN_BED {
            let Some(bed) = make(&mut commands, thing, at, None) else {
                continue;
            };
            let sown = crate::content::PLACE_SOWN.iter().find(|(p, _)| *p == place).and_then(|(_, seed)| modforge::grow::crop_of(&crate::content::CROPS, seed));
            if let Some(crop) = sown {
                let day = -crate::content::CROPS[crop].days;
                commands.entity(bed).insert(crate::garden::Growing(modforge::grow::Sown { crop, day }));
            }
            continue;
        }
        // A box under lock (the sheriff's armory), opened with its key.
        let kind = match crate::content::PLACE_LOCKS.iter().find(|(t, _)| *t == thing) {
            Some(&(_, lock)) => InteractKind::Locked { lock, door: false },
            None => InteractKind::Container,
        };
        let Some(made) = make(&mut commands, thing, at, Some(kind)) else {
            continue;
        };
        let mut held = modforge::item::Inventory::new(items.0.def(thing).and_then(|d| d.storage).unwrap_or(0));
        // A gas pump, a store of fuel cans, is found full: a stack of
        // cans in each slot (Works 12).
        if let (true, Some(can)) = (thing == crate::content::GAS_PUMP, items.0.def(crate::content::FUEL)) {
            for _ in 0..held.slots.len() {
                let _ = held.add(modforge::item::create(can, can.max_stack, &[], 0.0, 0), can.max_stack);
            }
        }
        // The post office's mail sack, found holding the town's letters
        // (Works 16).
        if let (true, Some(def)) = (thing == crate::content::MAIL_SACK, letter) {
            for words in crate::content::LETTERS {
                let mut stack = modforge::item::create(def, 1, &[], 0.0, 0);
                stack.note = Some(modforge::item::Note { text: words.to_string(), signed: String::new() });
                let _ = held.add(stack, 1);
            }
        }
        // A dump pile, found holding junk (Works 17).
        if thing == crate::content::DUMP_PILE {
            for &(junk, count) in &crate::content::DUMP_HOLDS {
                if let Some(def) = items.0.def(junk) {
                    let _ = held.add(modforge::item::create(def, count, &[], 0.0, 0), def.max_stack);
                }
            }
        }
        for &(_, _, item, count) in crate::content::PLACE_HOLDS.iter().filter(|(p, t, ..)| *p == place && *t == thing) {
            if let Some(def) = items.0.def(item) {
                let _ = held.add(modforge::item::create(def, count, &[], 0.0, 0), def.max_stack);
            }
        }
        commands.entity(made).insert(Container(held));
    }
    for at in town.streetlights(crate::content::HYDRANTS_APART) {
        make(&mut commands, crate::content::HYDRANT, at + Vec2::new(1.5, 0.0), None);
    }
    let safes = town.lots.iter().filter(|l| l.house).map(|l| l.at + modforge::house::safe(l.front));
    let rooms_of = town.wiring().into_iter().zip(town.heating()).zip(lights).zip(town.kitchens()).zip(town.clothing()).zip(town.storage()).zip(safes);
    for (house, (((((((breaker_at, rooms), heating), (lamps, candle)), kitchen), clothing), storage), safe_at)) in rooms_of.enumerate() {
        let Some(breaker) = make(&mut commands, crate::content::BREAKER_BOX, breaker_at, Some(InteractKind::Switch { on: true })) else {
            continue;
        };
        // Its safe, locked, found holding shells and scrap (Works 14).
        let safe_lock = crate::content::safe_lock(house);
        if let Some(made) = make(&mut commands, crate::content::SAFE, safe_at, Some(InteractKind::Locked { lock: safe_lock, door: false })) {
            let mut held = modforge::item::Inventory::new(items.0.def(crate::content::SAFE).and_then(|d| d.storage).unwrap_or(0));
            for &(thing, count) in &crate::content::SAFE_HOLDS {
                if let Some(def) = items.0.def(thing) {
                    let _ = held.add(modforge::item::create(def, count, &[], 0.0, 0), def.max_stack);
                }
            }
            commands.entity(made).insert(Container(held));
        }
        // The keys: the front door's on the garage shelf, else in the
        // kitchen cabinet; the safe's in the dresser.
        let has_shelf = storage.iter().any(|(kind, _)| *kind == crate::content::GARAGE_SHELF);
        let front_key_in = if has_shelf { crate::content::GARAGE_SHELF } else { crate::content::KITCHEN_CABINET };
        let key = |lock: u32| items.0.def(&modforge::hud::key_for(lock)).map(|d| modforge::item::create(d, 1, &[], 0.0, 0));
        let mut safe_key_put = false;
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
            if kind == front_key_in && let Some(k) = key(crate::content::front_lock(house)) {
                let _ = held.add(k, 1);
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
            if kind == crate::content::DRESSER && !safe_key_put && let Some(k) = key(safe_lock) {
                let _ = held.add(k, 1);
                safe_key_put = true;
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
