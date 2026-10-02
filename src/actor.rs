//! What every actor carries (authority.md: the player and every NPC
//! share the same components), the one actor map, and the one spawn
//! path every NPC enters through. These are Bevy components wrapping
//! modforge's engine-agnostic types; modforge owns the rules.

use avian3d::prelude::*;
use bevy::prelude::*;
use modforge::actor::{ActorDef, ActorId, ActorMap};
use modforge::brain::AWAKE_REACTION;
use modforge::statetree::Record;
use modforge::item;
use modforge::survival::{Condition, SurvivalStats};

use crate::combat::{Equipment, Firing, Health, Protection, Rolling};
use crate::content::{
    ActorRegistryRes, BUNKER_PEOPLE_AT, BUNKER_STORE_AT, BiomeRegistryRes, DOOR_ENEMY, FactionRegistryRes,
    ItemRegistryRes, MonumentRegistryRes, PLAYER_BUNKER_PEOPLE, PLAYER_FACTION, bunker_faction, def_for_danger,
};
use crate::terrain::WorldRes;
use crate::input::{Actions, ThisTick};
use crate::interact::{Container, Hud, OpenContainer, OpenStation, ThingId, WorldItem};
use crate::player::{BODY_CENTRE, body};
use crate::structure::MonumentsRes;
use crate::{Ground, SimTick, WorldSeed, on_ground};

/// Slots in a person's inventory: Rust's 6 by 4.
pub const INVENTORY_SLOTS: usize = 24;

/// Bound for the actor map's rows: the 1000 persistent people plus
/// animals and headroom (the 3D game's number).
pub const MAX_ACTORS: usize = 8192;

/// What a person looks like (modforge's `Look`: their body, the piece in
/// each of its slots, their skin and hair), rolled from who they are.
#[derive(Component, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Look(pub modforge::actor::Look);

/// Which way a person faces on the ground, in radians: 0 is along +x, a
/// quarter turn is along +y.
#[derive(Component, Default, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Facing(pub f32);

impl Facing {
    /// The unit direction faced.
    pub fn direction(self) -> Vec2 {
        Vec2::from_angle(self.0)
    }
}

/// A person who has died and not come back yet: not in the world, not
/// seen, not acting. An NPC never has it (it leaves the world); the
/// player has it between the death message and respawning.
#[derive(Component, Default, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Dead;

/// Dead for good (`combat::leave`): never back. Every topside person the
/// storm takes with their world, and every topside person who dies where
/// death sticks.
#[derive(Component, Default, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Gone;

/// The tick a person next decides at (life.md "How often a person
/// thinks"): set from their reaction time after every decision.
#[derive(Component, Default, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct NextThink(pub u64);

/// What a person carries.
#[derive(Component, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Inventory(pub item::Inventory);

impl Default for Inventory {
    fn default() -> Self {
        Self(item::Inventory::new(INVENTORY_SLOTS))
    }
}

/// The hotbar with the slot in the hands (modforge's: the item stays in
/// the bar, the held slot is what the person holds).
#[derive(Component, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Hotbar(pub modforge::hud::Hotbar);

/// The one actor map (authority.md ActorMap): modforge issues every
/// `ActorId`, never reused. Saved whole; after a load each person's entity
/// is swapped for its new one (save::loaded).
#[derive(Resource, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct ActorMapRes(pub ActorMap<Entity>);

impl Default for ActorMapRes {
    fn default() -> Self {
        Self(ActorMap::new(MAX_ACTORS))
    }
}

/// The persistent identity on every person, the player too. A person is
/// saved, and their body is made from them whenever they come into the
/// world, spawned or loaded.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
#[require(moonshine_save::prelude::Save, RigidBody = RigidBody::Kinematic, Collider = body(), Visibility)]
pub struct Actor(pub ActorId);

/// An NPC: which def it was spawned from and its faction (None for a
/// neutral).
#[derive(Component, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Npc {
    pub def: String,
    pub faction: Option<String>,
}

/// A person's seven axes, from the genome (modforge).
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Personality(pub modforge::actor::Personality);

/// What this person knows (modforge::memory). The brain reads only
/// this.
#[derive(Component, Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Memory(pub modforge::memory::Memory);

/// How someone stands toward `them`, by the one rule (modforge
/// `relationship::stands`): their own feeling (`memory`), else their
/// faction's standing (every living member's feeling, `members`), else the
/// faction table between `faction` and theirs.
pub fn stands_toward<'a>(
    (memory, faction): (&Memory, Option<&str>),
    members: impl Iterator<Item = &'a Memory>,
    (them, their_faction): (ActorId, Option<&str>),
    factions: &modforge::faction::FactionRegistry,
    heard: impl Fn(&str) -> f32 + Copy,
) -> modforge::faction::Relation {
    let own = modforge::relationship::feeling(&memory.0, them, heard);
    let standing = modforge::relationship::standing(members.map(|m| modforge::relationship::feeling(&m.0, them, heard)));
    modforge::relationship::stands(own, standing, factions.relation(faction, their_faction))
}

/// The map this person carries (modforge `map`, life.md "The map each
/// person carries"): every tile they have seen; the tile they last looked
/// around from; and how many tiles their bunker told them of that they
/// had not seen themselves.
#[derive(Component, Default, Debug, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Map {
    pub map: modforge::map::Map,
    pub looked_from: Option<(i32, i32)>,
    pub told: usize,
}

/// Where this person lives and returns to: the spot their feet stand on,
/// its height included (everything is 3D).
#[derive(Component, Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Home(pub Vec3);

/// A place this person was asked to go to and wait at (an episode's
/// errand, episodes.md), its height included; the brain weighs it like
/// any other choice.
#[derive(Component, Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Asked(pub Vec3);
/// Where this person was spawned, for the control plane: whether they
/// started somewhere or walked there.
#[derive(Component, Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct SpawnedAt(pub Vec3);

/// Their bunker's store (design.md "Factions have real stores"): the box
/// they eat from and bring loot home to.
#[derive(Component, Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Store(#[entities] pub Entity);

/// A person's needs (modforge survival).
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Survival(pub SurvivalStats);

/// What is true of this person right now for the needs: asleep, at
/// home, a threat in sight.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct ActorCondition(pub Condition);

/// What an NPC is doing: their brain's StateTree record, the one record
/// of it, kept between thinks (modforge docs/statetree.md).
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct ActorRecord(pub Record);

/// What the one spawn path needs from the world.
#[derive(bevy::ecs::system::SystemParam)]
pub struct SpawnActor<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub items: Res<'w, ItemRegistryRes>,
    pub layers: Res<'w, crate::content::LayerRegistryRes>,
    pub factions: Res<'w, FactionRegistryRes>,
    pub actors: ResMut<'w, ActorMapRes>,
    pub seed: Res<'w, WorldSeed>,
}

/// The ONE path through which an NPC enters the world (authority.md:
/// Endless's materialize_npc): the person (`spawn_person`, the same as
/// the player's) and the brain that controls it. `faction` overrides the
/// def's; `at` is where their feet stand, its height included. None when
/// the map is full or the faction is not registered.
pub fn spawn_actor(spawn: &mut SpawnActor, def: &ActorDef, at: Vec3, faction: Option<&str>) -> Option<Entity> {
    let faction = faction.map(String::from).or_else(|| def.faction.clone());
    let entity = spawn_person(spawn, def, at, faction.as_deref())?;
    spawn.commands.entity(entity).insert(Npc {
        def: def.name.clone(),
        faction,
    });
    Some(entity)
}

/// The person under the controller (design.md: no difference between a
/// bunker person and the player but AI against human control): what
/// every person has, a body, an `ActorId` from the one map, who they are
/// from the genome. `spawn_actor` adds the brain, `spawn_player` the
/// human. Spots are standable because world generation places them so
/// (design.md "A valid world"). None when the map is full or the faction
/// is not registered.
pub fn spawn_person(
    spawn: &mut SpawnActor,
    def: &ActorDef,
    at: Vec3,
    faction: Option<&str>,
) -> Option<Entity> {
    if let Some(name) = faction
        && spawn.factions.0.def(name).is_none()
    {
        return None;
    }
    // The def's weapon in the first bar slot and in the hands, the same
    // way the player holds one.
    let mut hotbar = Hotbar::default();
    if let Some(weapon) = def.weapon.as_deref().and_then(|w| spawn.items.0.def(w)) {
        match item::create_layered(weapon, &def.weapon_layers, &spawn.layers.0, 1, &[], 0.0, 0) {
            Ok(stack) => {
                hotbar.0.slots.slots[0] = Some(stack);
                hotbar.0.held = Some(0);
            }
            Err(e) => warn!("{}'s weapon: {e}", def.name),
        }
    }
    let entity = spawn
        .commands
        .spawn((
            Transform::from_translation(at + Vec3::Y * BODY_CENTRE),
            (
                Facing::default(),
                Inventory::default(),
                hotbar,
                Equipment::default(),
                Firing::default(),
                Rolling::default(),
                crate::player::Fall::default(),
            ),
            (
                Survival::default(),
                ActorCondition::default(),
                Memory::default(),
                Map::default(),
                ActorRecord::default(),
            ),
            (Hud::default(), OpenContainer::default(), OpenStation::default()),
            Protection(def.protection.clone()),
            Actions::default(),
            ThisTick::default(),
        ))
        .id();
    let Some((id, _row)) = spawn.actors.0.spawn(entity) else {
        spawn.commands.entity(entity).despawn();
        return None;
    };
    let personality = modforge::actor::Personality::seed(id, spawn.seed.0);
    // What they look like, rolled from who they are.
    let look = modforge::actor::Look::roll(id, spawn.seed.0, &crate::content::look_def());
    spawn.commands.entity(entity).insert(Look(look));
    // Everybody has a name (operator, 2026-09-27), Bevy's `Name` on the
    // person, rolled from who they are.
    let name = modforge::actor::roll_name(&crate::content::PERSON_NAMES, id, spawn.seed.0);
    spawn.commands.entity(entity).insert(Name::new(name));
    let mut health = def.health();
    health.max *= personality.health_mult();
    health.current = health.max;
    // Thinking spread across the first second by who they are, so people
    // do not all decide on the same tick.
    let first_think = id.0 % (AWAKE_REACTION * 60.0) as u64;
    spawn.commands.entity(entity).insert((
        Actor(id),
        Personality(personality),
        Health(health),
        NextThink(first_think),
        SpawnedAt(at),
    ));
    Some(entity)
}

/// The one waiting outside the bunker door, through `spawn_actor`.
pub fn spawn_door_enemy(mut spawn: SpawnActor, registry: Res<ActorRegistryRes>, world: Res<WorldRes>) {
    let (name, [x, z]) = DOOR_ENEMY;
    let def = registry.0.def(name).expect("the door enemy is registered");
    spawn_actor(&mut spawn, def, on_ground(world.bunker + Vec2::new(x, z), 0.0), None);
}

/// The top bit of a remembered key marks a place (a site, by
/// `episode::site_key`; a river's water); without it the key is a thing's
/// id (a box, `ThingId`). Keeps the two from
/// ever being mistaken for each other.
pub const PLACE_KEY: u64 = 1 << 63;

/// How far from a monument's centre its boxes count as home, in metres.
const HOME_BOXES_WITHIN: f32 = 60.0;

/// Fill every monument's NPC spots through `spawn_actor`. These are
/// topside people: found there and knowing the place, but with no home;
/// the storm kills them with their world (design.md "Bunker people and
/// topside people"). Each monument once, as its chunk is made.
pub fn populate_monuments(
    mut spawn: SpawnActor,
    registry: Res<ActorRegistryRes>,
    mut monuments: ResMut<MonumentsRes>,
    tick: Res<SimTick>,
    boxes: Query<(&ThingId, &Transform, &WorldItem), With<Container>>,
) {
    let due = std::mem::take(&mut monuments.1);
    for (at, monument) in due.into_iter().filter_map(|i| monuments.0.get(i)) {
        let known = home_knowledge(*at, &monument.kind, &boxes, tick.0);
        for spot in &monument.npc_spots {
            let Some(def) = registry.0.def(def_for_danger(spot.danger)) else {
                continue;
            };
            // On the floor its spot is on.
            let stand = on_ground(*at + Vec2::new(spot.position.x, spot.position.z), spot.position.y);
            settle(&mut spawn, def, stand, &known);
        }
    }
}

/// The topside people world generation placed on open land (design.md
/// "The small world first"), through the one settling path: no home,
/// knowing nothing yet. Once for each world the storm rolls.
pub fn populate_topside_people(
    mut spawn: SpawnActor,
    registry: Res<ActorRegistryRes>,
    mut world: ResMut<WorldRes>,
    mut things_made: ResMut<crate::terrain::ThingsMade>,
    biomes: Res<BiomeRegistryRes>,
    monuments: Res<MonumentRegistryRes>,
) {
    // Once a world; after a load they are in the save.
    if things_made.people {
        return;
    }
    things_made.people = true;
    let r = modforge::world2d::Registries {
        biomes: &biomes.0,
        monuments: &monuments.monuments,
        buildings: &monuments.buildings,
    };
    let known = Memory(modforge::memory::Memory::default());
    for (name, at) in world.world.topside_people(r) {
        if let Some(def) = registry.0.def(&name) {
            settle(&mut spawn, def, on_ground(at, 0.0), &known);
        }
    }
}

/// What someone found at the monument at `at` (of type `kind`) knows
/// from the start: the place (by its site's key), and every box there,
/// seen but not looked inside.
pub fn home_knowledge(
    at: Vec2,
    kind: &str,
    boxes: &Query<(&ThingId, &Transform, &WorldItem), With<Container>>,
    now: u64,
) -> Memory {
    let mut known = modforge::memory::Memory::default();
    known.see(crate::episode::site_key(at), kind, on_ground(at, 0.0), now);
    for (id, t, item) in boxes {
        if t.translation.ground().distance(at) <= HOME_BOXES_WITHIN {
            // Where the box rests, its height included.
            known.see(id.0, &item.stack.item, t.translation, now);
        }
    }
    Memory(known)
}

/// What a person sees inside a box: each kind with how many.
pub fn held_in(contents: &item::Inventory) -> Vec<(String, u32)> {
    let mut held: Vec<(String, u32)> = Vec::new();
    for stack in contents.slots.iter().flatten() {
        match held.iter_mut().find(|(k, _)| *k == stack.item) {
            Some((_, n)) => *n += stack.count,
            None => held.push((stack.item.clone(), stack.count)),
        }
    }
    held
}

/// A topside person standing at `stand` (their feet, its height included)
/// knowing `known`: through the one spawn path, with that memory and no
/// home.
pub fn settle(spawn: &mut SpawnActor, def: &ActorDef, stand: Vec3, known: &Memory) -> Option<Entity> {
    let npc = spawn_actor(spawn, def, stand, None)?;
    spawn.commands.entity(npc).insert(known.clone());
    Some(npc)
}

/// The bunker people (design.md "Bunker people and topside people"):
/// `BUNKER_PEOPLE` in every other bunker, and `PLAYER_BUNKER_PEOPLE`
/// beside the player in theirs (Roxanne), all through the one spawn path,
/// of that bunker's faction, home at their spot in it, knowing only that
/// (life.md "Learning to stay alive"). Once: they persist, and respawn
/// at home.
pub fn populate_bunkers(
    mut spawn: SpawnActor,
    registry: Res<ActorRegistryRes>,
    world: Res<WorldRes>,
    stores: Query<(Entity, &Transform), With<Container>>,
) {
    let def = registry.0.def("bunker person").expect("the bunker person is registered");
    // Each bunker: where, its faction, and its people (a name set in
    // content, or rolled, and where they stand in it).
    let mut bunkers: Vec<(Vec2, String, Vec<(Option<&str>, [f32; 2])>)> = vec![(
        world.bunker,
        PLAYER_FACTION.to_string(),
        PLAYER_BUNKER_PEOPLE.iter().map(|(name, at)| (Some(*name), *at)).collect(),
    )];
    for (i, &at) in world.bunkers.iter().enumerate() {
        bunkers.push((at, bunker_faction(i), BUNKER_PEOPLE_AT.iter().map(|at| (None, *at)).collect()));
    }
    for (at, faction, people) in bunkers {
        let [x, z] = BUNKER_STORE_AT;
        let store_at = at + Vec2::new(x, z);
        let store = stores
            .iter()
            .find(|(_, t, ..)| t.translation.ground().distance(store_at) < 0.5)
            .map(|(entity, ..)| entity);
        for (name, [x, z]) in people {
            // In the entrance, on the ground, until they can go down the
            // stairs to their rooms (todo 13a2).
            let stand = on_ground(at + Vec2::new(x, z), 0.0);
            if let Some(npc) = spawn_actor(&mut spawn, def, stand, Some(&faction)) {
                // They know one thing: this bunker is home (life.md
                // "Learning to stay alive"). The store is theirs, but
                // found, not known.
                spawn.commands.entity(npc).insert(Home(stand));
                if let Some(store) = store {
                    spawn.commands.entity(npc).insert(Store(store));
                }
                if let Some(name) = name {
                    spawn.commands.entity(npc).insert(Name::new(name));
                }
            }
        }
    }
}
