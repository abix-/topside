//! What a storm does beyond killing and rolling anew (todo "Storm effect
//! 1" to "Storm effect 9"; modforge `storm::Effect`; the storm types in
//! content/storm_types.yaml, Claude's list the operator took, 2026-11-24:
//! "I like your suggestions on the storm. Go with your suggestions."). The
//! storm runs each effect of its type once it has passed
//! (`storm::shift_world`); the deep storm acts while it rages
//! (`storm::storm`). Every number here is Claude's, for the operator to
//! change.

use bevy::prelude::*;
use modforge::storm::Effect;

use crate::actor::{Actor, ActorMapRes, Dead, Inventory, Map, Npc, SpawnActor, spawn_actor};
use crate::content::{BUNKER_AT, BUNKER_LEVEL, ItemRegistryRes, LayerRegistryRes};
use crate::interact::{Container, ThingIssuer, WorldItem, spawn_item};
use crate::structure::{Shelter, sheltered};
use crate::survival::Day;
use crate::{Ground, SimTick, WorldSeed, on_ground};

/// How far from the player's bunker, each way, a storm drops what it
/// brings.
const REACH: f32 = 150.0;

/// How many loot boxes a bounty storm drops, and the danger they are
/// filled at (higher than the town's, so often a higher tier).
const BOUNTY_BOXES: u32 = 6;
const BOUNTY_DANGER: u32 = 3;

/// How many people an arrival storm drops.
const ARRIVALS: u32 = 4;

/// The faction those caught by a changing storm come back in.
const CHANGED_FACTION: &str = "raiders";

/// What the hunting storm brings, and what fixes electronics a static
/// storm killed (one put in the struck thing).
const HUNTER: &str = "storm hunter";
const FIX_WITH: &str = "scrap";

/// Carry out one effect of the storm that has passed.
pub fn run(commands: &mut Commands, effect: Effect) {
    match effect {
        Effect::Bounty => commands.run_system_cached(bounty),
        Effect::Shuffle => commands.run_system_cached(shuffle),
        Effect::Rot => commands.run_system_cached(rot),
        Effect::Arrival => commands.run_system_cached(arrival),
        Effect::Changing => commands.run_system_cached(changing),
        Effect::Hunting => commands.run_system_cached(hunting),
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

/// The `n`th open ground point a storm drops something on, rolled from
/// the seed: around the player's bunker, never under a roof.
fn drop_point(seed: u64, salt: u64, n: u32, shelters: &Query<&Shelter>) -> Option<Vec2> {
    (0..16u32).find_map(|tries| {
        let roll = |k: u64| modforge::roll::salted_index(seed ^ u64::from(n) << 16 ^ u64::from(tries) << 32, salt + k, 20_001) as f32 / 10_000.0 - 1.0;
        let at = BUNKER_AT + Vec2::new(roll(0), roll(1)) * REACH;
        (!sheltered(shelters, at)).then_some(at)
    })
}

/// The bounty storm (todo "Storm effect 1"; Rust's airdrops): loot boxes
/// on the open ground.
#[allow(clippy::too_many_arguments)]
fn bounty(
    mut commands: Commands,
    mut ids: ResMut<ThingIssuer>,
    items: Res<ItemRegistryRes>,
    layers: Res<LayerRegistryRes>,
    (seed, tick, day): (Res<WorldSeed>, Res<SimTick>, Res<Day>),
    shelters: Query<&Shelter>,
) {
    let Some(box_def) = items.0.def("storage box") else {
        return;
    };
    let table = crate::content::loot_table();
    let today = crate::survival::today(&tick, *day);
    for n in 0..BOUNTY_BOXES {
        let Some(at) = drop_point(seed.0 ^ tick.0, 0xB0_0700, n, &shelters) else {
            continue;
        };
        let box_seed = seed.0 ^ tick.0 ^ u64::from(n) << 40;
        let contents = crate::structure::fill_box(&items, &layers, &table, box_def, BOUNTY_DANGER, box_seed, today);
        let stack = modforge::item::create(box_def, 1, &[], 0.0, box_seed);
        let (entity, _) = spawn_item(&mut commands, &mut ids, box_def, stack, on_ground(at, 0.0));
        commands.entity(entity).insert(Container(contents));
    }
}

/// The shuffle storm (todo "Storm effect 2"; Control's moving rooms): the
/// town laid again with its places moved about, the seed kept, so the
/// ground, the houses and the bunkers stay where they were.
fn shuffle(mut commands: Commands, mut state: ResMut<crate::storm::StormRes>, pieces: Query<Entity, With<crate::terrain::WorldPiece>>) {
    state.shuffles += 1;
    for entity in &pieces {
        commands.entity(entity).despawn();
    }
    commands.run_system_cached(crate::terrain::roll);
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

/// The arrival storm (todo "Storm effect 4"; RimWorld's drop pods): new
/// people on the open ground.
fn arrival(
    mut spawn: SpawnActor,
    actors: Res<crate::content::ActorRegistryRes>,
    tick: Res<SimTick>,
    shelters: Query<&Shelter>,
) {
    let Some(def) = actors.0.def("wanderer").cloned() else {
        return;
    };
    let seed = spawn.seed.0;
    for n in 0..ARRIVALS {
        if let Some(at) = drop_point(seed ^ tick.0, 0xA2_0100, n, &shelters) {
            spawn_actor(&mut spawn, &def, on_ground(at, 0.0), None);
        }
    }
}

/// The changing storm (todo "Storm effect 7"; STALKER's zombified
/// stalkers): the people caught outside live, and come back hostile.
fn changing(shelters: Query<&Shelter>, mut people: Query<(&Transform, &mut Npc), Without<Dead>>) {
    for (at, mut npc) in &mut people {
        if !sheltered(&shelters, at.translation.ground()) {
            npc.faction = Some(CHANGED_FACTION.to_string());
        }
    }
}

/// What the hunting storm brought, and the day it came: it leaves the
/// first morning after (saved).
#[derive(Component, Clone, Copy, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct TillMorning(pub u32);

/// The hunting storm (todo "Storm effect 8"; 7 Days to Die's blood moon):
/// a hunter on the surface till morning.
fn hunting(
    mut spawn: SpawnActor,
    actors: Res<crate::content::ActorRegistryRes>,
    (tick, day): (Res<SimTick>, Res<Day>),
    shelters: Query<&Shelter>,
) {
    let Some(def) = actors.0.def(HUNTER).cloned() else {
        return;
    };
    let seed = spawn.seed.0;
    let came = crate::survival::today(&tick, *day) as u32;
    if let Some(at) = drop_point(seed ^ tick.0, 0x4E_7700, 0, &shelters)
        && let Some(hunter) = spawn_actor(&mut spawn, &def, on_ground(at, 0.0), None)
    {
        spawn.commands.entity(hunter).insert(TillMorning(came));
    }
}

/// Every step: a hunter leaves once a morning after the day it came has
/// come.
pub fn hunters_leave(
    mut commands: Commands,
    (tick, day, reality): (Res<SimTick>, Res<Day>, Res<crate::Reality>),
    mut map: ResMut<ActorMapRes>,
    hunters: Query<(Entity, &TillMorning, Option<&Actor>)>,
) {
    if crate::survival::night(&tick, *day, &reality) {
        return;
    }
    let today = crate::survival::today(&tick, *day) as u32;
    for (entity, came, actor) in &hunters {
        if today > came.0 {
            crate::combat::leave(&mut commands, &mut map, entity, actor);
        }
    }
}

/// A thing whose electronics a static storm killed: it does not work
/// until one `FIX_WITH` is put in it (saved).
#[derive(Component, Clone, Copy, Default, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Struck;

/// The static storm (todo "Storm effect 5"; Fallout's EMP): every
/// generator stops until fixed.
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
