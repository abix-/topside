//! What every actor carries (authority.md: the player and every NPC
//! share the same components), the one actor map, and the one spawn
//! path every NPC enters through. These are Bevy components wrapping
//! modforge's engine-agnostic types; modforge owns the rules.

use avian2d::prelude::*;
use bevy::prelude::*;
use modforge::actor::{ActorDef, ActorId, ActorMap};
use modforge::brain::{Activity, CombatState};
use modforge::item;
use modforge::survival::{Condition, SurvivalStats};

use crate::combat::{Equipment, Firing, Health, Protection, Rolling};
use crate::content::{ActorRegistryRes, DOOR_ENEMY, FactionRegistryRes, ItemRegistryRes, def_for_danger};
use crate::terrain::WorldRes;
use crate::input::{Actions, ThisTick};
use crate::interact::{Container, WorldItem};
use crate::player::PLAYER_SIZE;
use crate::structure::MonumentsRes;
use crate::{SimTick, WorldSeed};

/// Slots in a person's inventory (the 3D game's number).
pub const INVENTORY_SLOTS: usize = 20;

/// Bound for the actor map's rows: the 1000 persistent people plus
/// animals and headroom (the 3D game's number).
pub const MAX_ACTORS: usize = 8192;

/// Above the ground and the buildings.
const NPC_Z: f32 = 0.9;
const NPC_COLOR: Color = Color::srgb(0.75, 0.25, 0.2);
const NOSE_COLOR: Color = Color::srgb(0.25, 0.1, 0.1);

/// Which way a person faces, in radians: 0 is along +x, a quarter turn
/// is +y (up the screen).
#[derive(Component, Default, Clone, Copy, Debug, PartialEq)]
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
#[derive(Component, Default, Clone, Copy, Debug, PartialEq)]
pub struct Dead;

/// What a person carries.
#[derive(Component)]
pub struct Inventory(pub item::Inventory);

impl Default for Inventory {
    fn default() -> Self {
        Self(item::Inventory::new(INVENTORY_SLOTS))
    }
}

/// The hotbar with the slot in the hands (modforge's: the item stays in
/// the bar, the held slot is what the person holds).
#[derive(Component, Default)]
pub struct Hotbar(pub modforge::hud::Hotbar);

/// The one actor map (authority.md ActorMap): modforge issues every
/// `ActorId`, never reused.
#[derive(Resource)]
pub struct ActorMapRes(pub ActorMap<Entity>);

impl Default for ActorMapRes {
    fn default() -> Self {
        Self(ActorMap::new(MAX_ACTORS))
    }
}

/// The persistent identity on every NPC.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Actor(pub ActorId);

/// An NPC: which def it was spawned from and its faction (None for a
/// neutral).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Npc {
    pub def: String,
    pub faction: Option<String>,
}

/// A person's seven axes, from the genome (modforge).
#[derive(Component, Default, Debug, Clone, Copy, PartialEq)]
pub struct Personality(pub modforge::actor::Personality);

/// What this person knows (modforge::memory). The brain reads only
/// this.
#[derive(Component, Default, Debug, Clone, PartialEq)]
pub struct Memory(pub modforge::memory::Memory);

/// Where this person lives and returns to.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Home(pub Vec2);

/// A person's needs (modforge survival).
#[derive(Component, Default, Debug, Clone, Copy, PartialEq)]
pub struct Survival(pub SurvivalStats);

/// What is true of this person right now for the needs: asleep, at
/// home, a threat in sight.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq)]
pub struct ActorCondition(pub Condition);

/// What an NPC is doing, kept between thinks (modforge::brain).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct ActorActivity(pub Activity);

impl Default for ActorActivity {
    fn default() -> Self {
        Self(Activity::Idle)
    }
}

/// Whether an NPC is fighting, kept between thinks.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ActorCombat(pub CombatState);

impl Default for ActorCombat {
    fn default() -> Self {
        Self(CombatState::None)
    }
}

/// What the one spawn path needs from the world.
#[derive(bevy::ecs::system::SystemParam)]
pub struct SpawnActor<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub items: Res<'w, ItemRegistryRes>,
    pub factions: Res<'w, FactionRegistryRes>,
    pub actors: ResMut<'w, ActorMapRes>,
    pub seed: Res<'w, WorldSeed>,
}

/// The ONE path through which an NPC enters the world (authority.md:
/// Endless's materialize_npc). The NPC gets what the player has, a
/// body the world can hit, its `ActorId` from the one map, and who it
/// is from the genome. `faction` overrides the def's. None when the
/// map is full or the faction is not registered.
pub fn spawn_actor(spawn: &mut SpawnActor, def: &ActorDef, at: Vec2, faction: Option<&str>) -> Option<Entity> {
    let faction = faction.map(String::from).or_else(|| def.faction.clone());
    if let Some(name) = faction.as_deref()
        && spawn.factions.0.def(name).is_none()
    {
        return None;
    }
    // The def's weapon in the first bar slot and in the hands, the same
    // way the player holds one.
    let mut hotbar = Hotbar::default();
    if let Some(weapon) = def.weapon.as_deref().and_then(|w| spawn.items.0.def(w)) {
        hotbar.0.slots.slots[0] = Some(item::create(weapon, 1, &[], 0.0, 0));
        hotbar.0.held = Some(0);
    }
    let entity = spawn
        .commands
        .spawn((
            Npc {
                def: def.name.clone(),
                faction,
            },
            Transform::from_translation(at.extend(NPC_Z)),
            (
                Facing::default(),
                Inventory::default(),
                hotbar,
                Equipment::default(),
                Firing::default(),
                Rolling::default(),
                crate::nav::NavPath::default(),
            ),
            (
                Survival::default(),
                ActorCondition::default(),
                Memory::default(),
                ActorActivity::default(),
                ActorCombat::default(),
            ),
            Protection(def.protection.clone()),
            Actions::default(),
            ThisTick::default(),
            RigidBody::Kinematic,
            Collider::circle(PLAYER_SIZE / 2.0),
            Sprite {
                color: NPC_COLOR,
                custom_size: Some(Vec2::splat(PLAYER_SIZE)),
                ..default()
            },
            children![(
                Sprite {
                    color: NOSE_COLOR,
                    custom_size: Some(Vec2::splat(PLAYER_SIZE / 3.0)),
                    ..default()
                },
                Transform::from_xyz(PLAYER_SIZE / 2.0, 0.0, 0.1),
            )],
        ))
        .id();
    let Some((id, _row)) = spawn.actors.0.spawn(entity) else {
        spawn.commands.entity(entity).despawn();
        return None;
    };
    let personality = modforge::actor::Personality::seed(id, spawn.seed.0);
    let mut health = def.health();
    health.max *= personality.health_mult();
    health.current = health.max;
    spawn
        .commands
        .entity(entity)
        .insert((Actor(id), Personality(personality), Health(health)));
    Some(entity)
}

/// The one waiting outside the bunker door, through `spawn_actor`.
pub fn spawn_door_enemy(mut spawn: SpawnActor, registry: Res<ActorRegistryRes>, world: Res<WorldRes>) {
    let (name, [x, z]) = DOOR_ENEMY;
    let def = registry.0.def(name).expect("the door enemy is registered");
    spawn_actor(&mut spawn, def, world.0.def.bunker + Vec2::new(x, z), None);
}

/// How far from a monument's centre its boxes count as home, in metres.
const HOME_BOXES_WITHIN: f32 = 60.0;

/// Fill every monument's NPC spots through `spawn_actor`, once. Each
/// person lives there and knows the place from the start (life.md
/// "Home").
pub fn populate_monuments(
    mut spawn: SpawnActor,
    registry: Res<ActorRegistryRes>,
    monuments: Res<MonumentsRes>,
    tick: Res<SimTick>,
    boxes: Query<(Entity, &Transform, &WorldItem), With<Container>>,
) {
    for (i, (at, monument)) in monuments.0.iter().enumerate() {
        let mut known = modforge::memory::Memory::default();
        known.see(i as u64, &monument.kind, Vec3::new(at.x, 0.0, at.y), tick.0);
        // The boxes at home, seen but not looked inside.
        for (entity, t, item) in &boxes {
            let p = t.translation.truncate();
            if p.distance(*at) <= HOME_BOXES_WITHIN {
                known.see(entity.to_bits(), &item.stack.item, Vec3::new(p.x, 0.0, p.y), tick.0);
            }
        }
        for spot in &monument.npc_spots {
            let Some(def) = registry.0.def(def_for_danger(spot.danger)) else {
                continue;
            };
            let stand = *at + Vec2::new(spot.position.x, spot.position.z);
            if let Some(npc) = spawn_actor(&mut spawn, def, stand, None) {
                spawn.commands.entity(npc).insert((Home(stand), Memory(known.clone())));
            }
        }
    }
}
