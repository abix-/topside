//! Trouble at home (todo Events 5; modforge `event::Home`, Claude's list
//! the operator took, 2026-11-24): what an event does inside the player's
//! own bunker, through the systems that own each thing: fire through
//! src/fire.rs, sickness through modforge `sickness` and the one hit
//! path, feelings through memory (modforge `relationship::feeling`).
//! Who and what is picked is rolled from the event's seed, in ActorId
//! order. Every number here is Claude's, for the operator to change.

use bevy::prelude::*;
use modforge::combat::Hit;
use modforge::event::Home;
use modforge::memory::{Did, Line};

use crate::Ground;
use crate::actor::{Actor, Dead, Home as HomeAt, Npc, Persona};
use crate::combat::{Equipment, Health, Protection, hit_actor};
use crate::content::{BUNKER_AT, DamageRegistryRes, ItemRegistryRes, LayerRegistryRes};
use crate::interact::{Container, WorldItem};

/// How far from the player's bunker someone lives, or something lies, to
/// be at home.
const AT_HOME: f32 = 30.0;

/// How sickness goes (modforge `sickness`).
const SICKNESS: modforge::sickness::SicknessDef = modforge::sickness::SicknessDef { lasts: 1800.0, reach: 2.5, catch_chance: 0.002 };

/// The faction one who breaks runs to.
const BROKE_TO: &str = "raiders";

/// The lines heard when two fall out or fall for each other, felt as
/// content's `SAID_FELT` has them.
pub const FELL_OUT: &str = "fell out";
pub const FELL_FOR: &str = "fell for";

/// Someone sick, and for how long (saved).
#[derive(Component, Clone, Copy, Default, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Sick(pub modforge::sickness::Sickness);

/// The trouble happens, rolled from `seed`.
#[allow(clippy::type_complexity)]
pub fn trouble(
    In((home, seed)): In<(Home, u64)>,
    mut commands: Commands,
    mut people: Query<(Entity, &Actor, &HomeAt, &mut Npc, &mut Persona, Option<&mut crate::actor::Inventory>, Option<&crate::actor::Store>), Without<Dead>>,
    mut world_items: ParamSet<(
        Query<(Entity, &WorldItem, &Transform, &crate::interact::ThingId), Without<Actor>>,
        Query<(&Transform, &mut WorldItem), (Without<Container>, Without<Actor>)>,
    )>,
    mut stores: Query<&mut Container, Without<Actor>>,
    items: Res<ItemRegistryRes>,
    shelters: Query<&crate::structure::Shelter>,
) {
    let roll = |salt: u64, n: usize| modforge::roll::salted_index(seed, 0x4031_0000 + salt, n.max(1) as u64) as usize;
    // The player's bunker people, by their ids.
    let mut ours: Vec<(Entity, modforge::actor::ActorId)> =
        people.iter().filter(|(_, _, at, ..)| at.0.ground().distance(BUNKER_AT) < AT_HOME).map(|(e, a, ..)| (e, a.0)).collect();
    ours.sort_by_key(|(_, id)| *id);
    match home {
        // A short circuit: the burnable thing at home picked catches.
        Home::Fire => {
            let mut burnable: Vec<(Entity, u64)> = world_items
                .p0()
                .iter()
                .filter(|(_, item, at, _)| crate::content::BURNS.contains(&item.stack.item.as_str()) && at.translation.ground().distance(BUNKER_AT) < AT_HOME)
                .map(|(e, .., id)| (e, id.0))
                .collect();
            burnable.sort_by_key(|(_, id)| *id);
            if let Some((entity, _)) = burnable.get(roll(0, burnable.len())) {
                commands.entity(*entity).insert(crate::fire::Burning(modforge::fire::Fire::default()));
            }
        }
        Home::Sickness => {
            if let Some((entity, _)) = ours.get(roll(1, ours.len())) {
                commands.entity(*entity).insert(Sick::default());
            }
        }
        // The bottom level floods: the food lying there spoils (the
        // water itself is not drawn yet).
        Home::Flood => {
            let at_home = |at: &Transform| at.translation.ground().distance(BUNKER_AT) < AT_HOME && crate::structure::sheltered(&shelters, at.translation.ground());
            let mut lying = world_items.p1();
            let bottom = lying.iter().filter(|(at, _)| at_home(at)).map(|(at, _)| at.translation.y).fold(f32::MAX, f32::min);
            for (at, mut item) in &mut lying {
                if at_home(at) && at.translation.y < bottom + 1.0 && crate::content::KEEPS_DAYS.iter().any(|(name, _)| *name == item.stack.item) {
                    item.stack.item = crate::content::ROTTEN_FOOD.to_string();
                }
            }
        }
        // Someone breaks: takes the first stack from their store and turns
        // on the rest.
        Home::Breaks => {
            let Some(&(entity, _)) = ours.get(roll(2, ours.len())) else {
                return;
            };
            let Ok((.., mut npc, mut persona, inventory, store)) = people.get_mut(entity) else {
                return;
            };
            npc.faction = Some(BROKE_TO.to_string());
            persona.0.memory.did(Did::Done("broke and ran".to_string()), 0);
            let stolen = store.and_then(|s| stores.get_mut(s.0).ok()).and_then(|mut held| held.0.slots.iter_mut().find_map(|s| s.take()));
            if let (Some(stack), Some(mut inventory)) = (stolen, inventory) {
                let most = items.0.def(&stack.item).map_or(1, |d| d.max_stack);
                // What does not fit is dropped where it was: gone from the store.
                let _ = inventory.0.add(stack, most);
            }
        }
        // Two fall out, or for each other: each hears it of the other.
        Home::FallOut | Home::InLove => {
            if ours.len() < 2 {
                return;
            }
            let a = roll(3, ours.len());
            let b = (a + 1 + roll(4, ours.len() - 1)) % ours.len();
            let name = if home == Home::FallOut { FELL_OUT } else { FELL_FOR };
            for (me, them) in [(ours[a], ours[b]), (ours[b], ours[a])] {
                if let Ok((.., mut persona, _, _)) = people.get_mut(me.0) {
                    persona.0.memory.did(Did::Heard(them.1, Line { name: name.to_string(), words: name.to_string() }), 0);
                }
            }
        }
    }
}

/// Every step: each sick person is hurt once a game second through the
/// one hit path, gives it to those near, and gets over it in time.
#[allow(clippy::type_complexity)]
pub fn sicken(
    mut commands: Commands,
    time: Res<Time<Fixed>>,
    tick: Res<crate::SimTick>,
    harm: (Res<ItemRegistryRes>, Res<LayerRegistryRes>, Res<DamageRegistryRes>),
    mut sick: Query<(Entity, &Transform, &mut Sick, &Protection, &Equipment, &mut Health, Option<&mut Persona>), Without<Dead>>,
    well: Query<(Entity, &Actor, &Transform), (Without<Sick>, Without<Dead>, With<Npc>)>,
) {
    let (items, layers, damage) = harm;
    let dt = time.delta_secs();
    let second = tick.0 % 60 == 0;
    let mut near: Vec<Vec3> = Vec::new();
    for (entity, at, mut sickness, own, worn, mut health, mut persona) in &mut sick {
        if sickness.0.tick(&SICKNESS, dt) {
            commands.entity(entity).remove::<Sick>();
            continue;
        }
        near.push(at.translation);
        if let (true, Some(def)) = (second, damage.0.def(crate::content::SICKNESS_DAMAGE)) {
            let hit = Hit { def, self_inflicted: false, distance: 0.0, location_scale: 1.0 };
            hit_actor(&hit, own, worn, &items.0, &layers.0, Default::default(), &mut health, persona.as_deref_mut(), None, tick.0);
        }
    }
    for (entity, actor, at) in &well {
        let roll = modforge::roll::salted_index(tick.0 ^ u64::from(actor.0.0) << 32, 0x5_1C4, 10_001) as f32 / 10_000.0;
        if near.iter().any(|p| modforge::sickness::catches(&SICKNESS, p.distance(at.translation), dt, roll)) {
            commands.entity(entity).insert(Sick::default());
        }
    }
}
