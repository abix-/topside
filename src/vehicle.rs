//! Vehicles (todo 57) on modforge's rules (modforge vehicle.rs). A car
//! is an actor (`spawn_actor` with the car's `ActorDef`) that also has a
//! `Vehicle`. Use (E) beside it: holding fuel puts a can in; holding the
//! repair's scrap mends it when broken; else one gets in, if a seat is
//! free, the first in driving. While in, a person's own moves and aim go
//! to the car's queue instead of their body (the driver's; a
//! passenger's are dropped), and they ride where it is; Use again gets
//! out. It burns fuel only while it moves, stops when the tank is empty
//! or it has broken down. Everyone is equal: any person gets in the same
//! way, through the same actions.

use std::collections::BTreeMap;

use avian3d::prelude::ColliderDisabled;
use bevy::prelude::*;
use modforge::actions::Action;

use crate::actor::{Actor, Dead, Hotbar, Inventory};
use crate::content::{CAR_FUEL, CAR_PARKED, CAR_REPAIR, FUEL};
use crate::input::ThisTick;
use crate::{Ground, on_ground};

/// How near one must be to use a vehicle, metres (Claude's).
const REACH: f32 = 3.0;

/// A vehicle's fuel (seconds of driving), whether it is broken, and how
/// long it has been driven (saved).
#[derive(Component, Clone, Copy, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Vehicle {
    pub fuel: f32,
    pub broken: bool,
    pub driven: f32,
    /// Which vehicle it is (content `VEHICLES`), 0 the car.
    #[serde(default)]
    pub kind: u8,
}

impl Vehicle {
    /// Its rules (content `vehicle_def`).
    pub fn def(&self) -> &'static modforge::vehicle::VehicleDef {
        crate::content::vehicle_def(self.kind)
    }
}

/// Who is in a vehicle, and which, by its ActorId (saved).
#[derive(Component, Clone, Copy, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Riding(pub modforge::actor::ActorId);

/// Whether the world's car is parked yet (saved).
#[derive(Resource, Default, Clone, Copy, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct Parked(pub bool);

/// Once a world: the car parked by the player's bunker, and a vehicle in
/// every house's driveway (Works 13), with a little fuel.
pub fn park(
    mut spawn: crate::actor::SpawnActor,
    mut parked: ResMut<Parked>,
    registry: Res<crate::content::ActorRegistryRes>,
    world: Option<Res<crate::terrain::WorldRes>>,
) {
    let (false, Some(world)) = (parked.0, world) else {
        return;
    };
    parked.0 = true;
    let mut spots = vec![(0usize, world.bunker + Vec2::from(CAR_PARKED))];
    if let Some(town) = world.world.town.as_ref() {
        // Before the garage door, a few metres out.
        let driveways = town.fixed_houses().map(|(_, l)| l).map(|l| l.at + modforge::house::driveway(l.front));
        spots.extend(driveways.enumerate().map(|(i, at)| (crate::content::driveway(i), at)));
    }
    for (kind, at) in spots {
        let Some(&(name, rules)) = crate::content::VEHICLES.get(kind) else {
            continue;
        };
        let Some(def) = registry.0.def(name) else {
            continue;
        };
        if let Some(made) = crate::actor::spawn_actor(&mut spawn, def, on_ground(at, 0.0), None) {
            let fuel = if rules.per_can > 0.0 { CAR_FUEL } else { rules.tank };
            spawn.commands.entity(made).insert(Vehicle { fuel, broken: false, driven: 0.0, kind: kind as u8 });
        }
    }
}

/// Every step, before anyone moves: Use beside a vehicle fuels, mends,
/// gets in or out; then each vehicle's queue is its driver's moves and
/// aim, or nothing.
#[allow(clippy::type_complexity)]
pub fn drive(
    mut commands: Commands,
    mut vehicles: Query<(Entity, &Transform, &mut Vehicle, &mut ThisTick, &Actor)>,
    mut people: Query<(Entity, &mut ThisTick, &Transform, &mut Inventory, &mut Hotbar, Option<&Riding>, &Actor), (Without<Vehicle>, Without<Dead>)>,
) {
    // In ActorId order (authority.md rule 5): who gets the seat first is
    // the same after a load.
    let order = crate::actor::in_actor_order(people.iter().map(|(e, .., a)| (Some(a.0), e)));
    let car_of: BTreeMap<modforge::actor::ActorId, Entity> = vehicles.iter().map(|(e, .., a)| (a.0, e)).collect();
    // Each vehicle's riders, in ActorId order: the first drives.
    let mut seated: BTreeMap<Entity, Vec<Entity>> = BTreeMap::new();
    for person in &order {
        if let Ok((e, .., Some(r), _)) = people.get(*person)
            && let Some(car) = car_of.get(&r.0)
        {
            seated.entry(*car).or_default().push(e);
        }
    }
    for person in &order {
        let Ok((entity, mut this_tick, at, mut inventory, mut hotbar, riding, _)) = people.get_mut(*person) else {
            continue;
        };
        if !this_tick.0.iter().any(|s| s.action == Action::Use) {
            continue;
        }
        if let Some(riding) = riding {
            // Out, beside it.
            commands.entity(entity).remove::<(Riding, ColliderDisabled)>();
            if let Some(seats) = car_of.get(&riding.0).and_then(|car| seated.get_mut(car)) {
                seats.retain(|e| *e != entity);
            }
            this_tick.0.retain(|s| s.action != Action::Use);
            continue;
        }
        let near = vehicles
            .iter_mut()
            .filter(|(_, t, ..)| t.translation.distance(at.translation) <= REACH)
            .min_by(|a, b| a.1.translation.distance(at.translation).total_cmp(&b.1.translation.distance(at.translation)));
        let Some((car, _, mut vehicle, _, car_id)) = near else {
            continue;
        };
        this_tick.0.retain(|s| s.action != Action::Use);
        let held = hotbar.0.held_stack().map(|s| s.item.clone());
        match held.as_deref() {
            Some(FUEL) => {
                let rules = vehicle.def();
                if rules.per_can > 0.0 && vehicle.fuel < rules.tank && (hotbar.0.slots.take_one(FUEL) || inventory.0.take_one(FUEL)) {
                    vehicle.fuel = modforge::vehicle::fill(rules, vehicle.fuel);
                }
            }
            // Fixed with scrap in hand and a wrench carried (Works 11).
            Some(item) if item == CAR_REPAIR.0 && vehicle.broken => {
                if inventory.0.count_of(crate::content::WRENCH) > 0 && crate::research::pay(&mut inventory.0, &[(CAR_REPAIR.0.to_string(), CAR_REPAIR.1)]) {
                    vehicle.broken = false;
                }
            }
            _ => {
                let seats = seated.entry(car).or_default();
                if (seats.len() as u32) < vehicle.def().seats {
                    seats.push(entity);
                    commands.entity(entity).insert((Riding(car_id.0), ColliderDisabled));
                }
            }
        }
    }
    // Each vehicle's queue: its driver's moves and aim, taken from the
    // driver's body; a passenger's are dropped. Its own thoughts never.
    for (car, _, vehicle, mut queue, _) in &mut vehicles {
        queue.0.clear();
        let Some(seats) = seated.get(&car) else {
            continue;
        };
        for (i, rider) in seats.iter().enumerate() {
            let Ok((_, mut this_tick, ..)) = people.get_mut(*rider) else {
                continue;
            };
            let moves: Vec<_> = this_tick.0.iter().copied().filter(|s| matches!(s.action, Action::Move { .. } | Action::Aim { .. })).collect();
            this_tick.0.retain(|s| !matches!(s.action, Action::Move { .. } | Action::Aim { .. }));
            if i == 0 && !vehicle.broken && vehicle.fuel > 0.0 {
                queue.0.extend(moves);
            }
        }
    }
}

/// Every step, after everyone moves: fuel burnt and breakdowns rolled
/// for each vehicle that moved; its riders where it is.
#[allow(clippy::type_complexity)]
pub fn ride(
    mut commands: Commands,
    time: Res<Time<Fixed>>,
    world: Option<Res<crate::terrain::WorldRes>>,
    mut vehicles: Query<(&Transform, &mut Vehicle, &ThisTick, &Actor), Without<Riding>>,
    mut riders: Query<(Entity, &Riding, &mut Transform, Has<ColliderDisabled>), Without<Vehicle>>,
) {
    let dt = time.delta_secs();
    let seed = world.map_or(0, |w| w.seed);
    for (_, mut vehicle, queue, actor) in &mut vehicles {
        let (x, y) = modforge::actions::ActionQueue::movement(&queue.0);
        let moving = x != 0.0 || y != 0.0;
        vehicle.fuel = modforge::vehicle::burn(vehicle.fuel, dt, moving);
        if moving {
            let before = (vehicle.driven / 60.0) as u64;
            vehicle.driven += dt;
            let minute = (vehicle.driven / 60.0) as u64;
            if minute > before && modforge::vehicle::breaks(vehicle.def(), seed, actor.0.0, minute) {
                vehicle.broken = true;
                info!("a car broke down");
            }
        }
    }
    let where_is: BTreeMap<modforge::actor::ActorId, Vec3> = vehicles.iter().map(|(t, .., a)| (a.0, t.translation)).collect();
    for (rider, riding, mut at, disabled) in &mut riders {
        // A rider's body takes no room while in (after a load too).
        if !disabled {
            commands.entity(rider).insert(ColliderDisabled);
        }
        if let Some(car) = where_is.get(&riding.0) {
            at.translation = on_ground(car.ground(), car.y);
        }
    }
}
