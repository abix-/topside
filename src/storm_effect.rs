//! What a storm does beyond killing and rolling anew (todo "Storm effect
//! 1" to "Storm effect 9"; modforge `storm::Effect`; the storm types in
//! content/storm_types.yaml, Claude's list the operator took, 2026-11-24:
//! "I like your suggestions on the storm. Go with your suggestions."). The
//! storm runs each effect of its type once it has passed
//! (`storm::shift_world`); the deep storm acts while it rages
//! (`storm::storm`). Loot, new people and hunters are events now
//! (src/event.rs; operator, 2026-11-24: "events and storms are
//! different"), and an event can do any effect here. Every number here
//! is Claude's, for the operator to change.

use bevy::prelude::*;
use modforge::storm::Effect;

use crate::actor::{Dead, Inventory, Map, Npc};
use crate::content::{BUNKER_AT, BUNKER_LEVEL};
use crate::interact::{Container, WorldItem};
use crate::structure::{Shelter, sheltered};
use crate::Ground;

/// How far from the player's bunker, each way, something is dropped.
const REACH: f32 = 150.0;

/// The faction those caught by a changing storm come back in.
pub const CHANGED_FACTION: &str = "raiders";

/// What fixes electronics a static storm killed (one put in the struck
/// thing).
const FIX_WITH: &str = "scrap";

/// Carry out one effect, of a storm that has passed or of an event.
pub fn run(commands: &mut Commands, effect: Effect) {
    match effect {
        Effect::Shuffle => commands.run_system_cached(shuffle),
        Effect::Rot => commands.run_system_cached(rot),
        Effect::Changing => commands.run_system_cached(changing),
        Effect::Static => commands.run_system_cached(struck),
        Effect::Forgetting => commands.run_system_cached(forgetting),
        // The deep storm acts while it rages (`reaches`).
        Effect::Deep => {}
    }
}

/// Whether the deep storm reaches someone whose body's centre is at
/// `at`: anyone not below the first level down, under a roof or not.
pub fn reaches(at: Vec3) -> bool {
    at.y - crate::player::BODY_CENTRE > -BUNKER_LEVEL / 2.0
}

/// The `n`th open ground point something is dropped on, rolled from the
/// seed: around the player's bunker, never under a roof.
pub fn drop_point(seed: u64, salt: u64, n: u32, shelters: &Query<&Shelter>) -> Option<Vec2> {
    (0..16u32).find_map(|tries| {
        let roll = |k: u64| modforge::roll::salted_index(seed ^ u64::from(n) << 16 ^ u64::from(tries) << 32, salt + k, 20_001) as f32 / 10_000.0 - 1.0;
        let at = BUNKER_AT + Vec2::new(roll(0), roll(1)) * REACH;
        (!sheltered(shelters, at)).then_some(at)
    })
}

/// The shuffle storm (todo "Storm effect 2"; Control's moving rooms): the
/// town laid again with its places moved about, the seed kept, so the
/// ground, the houses and the bunkers stay where they were.
fn shuffle(
    mut commands: Commands,
    mut state: ResMut<crate::storm::StormRes>,
    world: Option<Res<crate::terrain::WorldRes>>,
    pieces: Query<Entity, With<crate::terrain::WorldPiece>>,
) {
    // Each place's lot before the move, in the town's order of places.
    let was: Vec<modforge::town::Lot> = world
        .as_deref()
        .and_then(|w| w.world.town.as_ref())
        .map(|town| town.places().iter().map(|(lot, _)| town.lots[*lot].clone()).collect())
        .unwrap_or_default();
    state.shuffles += 1;
    for entity in &pieces {
        commands.entity(entity).despawn();
    }
    commands.run_system_cached(crate::terrain::roll);
    commands.run_system_cached_with(carry, was);
}

/// After a shuffle, what lay in a place that moved goes with it, to the
/// same spot in it on its new lot, turned as it is turned.
fn carry(In(was): In<Vec<modforge::town::Lot>>, world: Res<crate::terrain::WorldRes>, mut lying: Query<&mut Transform, With<WorldItem>>) {
    let Some(town) = world.world.town.as_ref() else {
        return;
    };
    let now: Vec<&modforge::town::Lot> = town.places().iter().map(|(lot, _)| &town.lots[*lot]).collect();
    for mut at in &mut lying {
        let p = at.translation.ground();
        let Some((from, to)) = was.iter().zip(&now).find(|(from, _)| from.covers(p)) else {
            continue;
        };
        if from.at == to.at {
            continue;
        }
        // Turned from the old front to the new (both unit vectors).
        let offset = (p - from.at).rotate(Vec2::new(from.front.x, -from.front.y)).rotate(to.front);
        let moved = to.at + offset;
        at.translation.x = moved.x;
        at.translation.z = moved.y;
    }
}

/// The rot storm (todo "Storm effect 3"; Project Zomboid's decay in one
/// night): food outside turns rotten, lying, in a box or carried.
fn rot(
    shelters: Query<&Shelter>,
    mut lying: Query<(&Transform, &mut WorldItem, Option<&mut Container>)>,
    mut carried: Query<(&Transform, &mut Inventory), Without<WorldItem>>,
) {
    let turn = |stack: &mut modforge::item::ItemStack| {
        if crate::content::KEEPS_DAYS.iter().any(|(item, _)| *item == stack.item) {
            stack.item = crate::content::ROTTEN_FOOD.to_string();
        }
    };
    for (at, mut item, held) in &mut lying {
        if sheltered(&shelters, at.translation.ground()) {
            continue;
        }
        turn(&mut item.stack);
        if let Some(mut held) = held {
            held.0.slots.iter_mut().flatten().for_each(turn);
        }
    }
    for (at, mut held) in &mut carried {
        if !sheltered(&shelters, at.translation.ground()) {
            held.0.slots.iter_mut().flatten().for_each(turn);
        }
    }
}

/// The changing storm (todo "Storm effect 7"; STALKER's zombified
/// stalkers): the people caught outside live, and come back hostile.
/// The player too, everyone being equal (Claude's, from topside-authority
/// "Everyone is equal"): caught outside, they come back one of the
/// changed, `Changed` standing for the faction they have no `Npc` to hold.
fn changing(
    mut commands: Commands,
    shelters: Query<&Shelter>,
    mut people: Query<(Entity, &Transform, Option<&mut Npc>), (Or<(With<Npc>, With<crate::player::Player>)>, Without<Dead>)>,
) {
    for (entity, at, npc) in &mut people {
        if sheltered(&shelters, at.translation.ground()) {
            continue;
        }
        match npc {
            Some(mut npc) => npc.faction = Some(CHANGED_FACTION.to_string()),
            None => {
                commands.entity(entity).insert(Changed);
            }
        }
    }
}

/// On the player a changing storm caught outside: the others see them as
/// one of `CHANGED_FACTION` (saved).
#[derive(Component, Clone, Copy, Default, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Changed;

/// A thing whose electronics a static storm killed: it does not work
/// until one `FIX_WITH` is put in it (saved).
#[derive(Component, Clone, Copy, Default, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Struck;

/// Static (todo "Storm effect 5"; Fallout's EMP, RimWorld's solar
/// flare): every generator stops until fixed.
fn struck(mut commands: Commands, things: Query<(Entity, &WorldItem)>) {
    for (entity, item) in &things {
        if item.stack.item == crate::content::GENERATOR {
            commands.entity(entity).insert(Struck);
        }
    }
}

/// Every step: a struck thing with one `FIX_WITH` put in it takes it and
/// works again.
pub fn fix_struck(mut commands: Commands, mut struck: Query<(Entity, &mut Container), With<Struck>>) {
    for (entity, mut held) in &mut struck {
        if held.0.count_of(FIX_WITH) > 0 {
            held.0.take_one(FIX_WITH);
            commands.entity(entity).remove::<Struck>();
        }
    }
}

/// The forgetting storm (todo "Storm effect 6"): whoever was outside,
/// the player too, loses the map they carry.
fn forgetting(shelters: Query<&Shelter>, mut people: Query<(&Transform, &mut Map), Without<Dead>>) {
    for (at, mut map) in &mut people {
        if !sheltered(&shelters, at.translation.ground()) {
            *map = Map::default();
        }
    }
}
