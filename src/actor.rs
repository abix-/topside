//! What every actor carries. The player and every NPC are the same
//! kind of actor on the same structs; only who drives them differs
//! (design.md). Three separate item holders, each its own thing:
//! the inventory, the hotbar, and the equipment slots. Survival
//! stats live in survival.rs, the same way.
//!
//! These are Bevy components wrapping modforge's engine-agnostic
//! types; modforge owns the rules, this file owns the binding.

use avian3d::prelude::*;
use bevy::prelude::*;
use modforge::actor::ActorDef;
use modforge::hud::HOTBAR_SLOTS;
use modforge::item::{self, EquipSlot};

use crate::cel::CelMaterial;
use crate::combat::{Firing, Health, Protection};
use crate::content::{ActorRegistryRes, ItemRegistryRes};
use crate::actor_map::{Actor, ActorMapRes};
use crate::input::{Actions, ThisTick};
use crate::structure::{BODY_RADIUS, BODY_TOP, MonumentRoot, body_shape};
use crate::survival::Survival;

pub const INVENTORY_SLOTS: usize = 20;

/// Where the eyes sit above the feet. An actor's transform is at
/// its eyes (the player's camera); the body hangs below.
pub const EYE_HEIGHT: f32 = 1.7;

/// Which way an actor faces: yaw about y, pitch up and down. `look`
/// turns it, `movement` walks along it, `fire` aims along it. The
/// player's camera and an NPC's body share it.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq)]
pub struct Facing {
    pub yaw: f32,
    pub pitch: f32,
}

impl Facing {
    pub fn rotation(&self) -> Quat {
        Quat::from_euler(EulerRot::YXZ, self.yaw, self.pitch, 0.0)
    }
}

/// An NPC: which def it was spawned from and its faction. The player
/// has no `Npc`; everything else on an actor is shared (see
/// `spawn_actor`). Identity is the `Actor` component beside it.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Npc {
    pub def: String,
    pub faction: String,
}

/// What the one spawn path needs from the world.
#[derive(bevy::ecs::system::SystemParam)]
pub struct SpawnActor<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub meshes: ResMut<'w, Assets<Mesh>>,
    pub materials: ResMut<'w, Assets<CelMaterial>>,
    pub items: Res<'w, ItemRegistryRes>,
    pub actors: ResMut<'w, ActorMapRes>,
}

/// The ONE path through which an NPC enters the world (authority.md:
/// Endless's materialize_npc discipline). Monument NPC spots, edge
/// arrivals, saves, and tests all come here. The actor gets exactly
/// what the player has (inventory, hotbar, equipment, survival,
/// health, protection, a fire timer, an action queue) plus a body
/// the world can hit and its `ActorId` from the one map. `feet` is
/// where it stands. Returns None when the map has no row left.
pub fn spawn_actor(spawn: &mut SpawnActor, def: &ActorDef, feet: Vec3, yaw: f32) -> Option<Entity> {
    let facing = Facing { yaw, pitch: 0.0 };
    let mut equipment = Equipment::default();
    if let Some(weapon) = def.weapon.as_deref().and_then(|w| spawn.items.0.def(w)) {
        let stack = item::create(weapon, 1, &[], 0.0, 0);
        equipment.0.set(EquipSlot::Weapon, Some(stack));
    }
    let half = BODY_TOP / 2.0;
    let entity = spawn
        .commands
        .spawn((
            Npc {
                def: def.name.clone(),
                faction: def.faction.clone(),
            },
            Transform::from_translation(feet + Vec3::Y * EYE_HEIGHT).with_rotation(facing.rotation()),
            Visibility::default(),
            facing,
            Inventory::default(),
            Hotbar::default(),
            equipment,
            Survival::default(),
            Health(def.health()),
            Protection(def.protection.clone()),
            Firing::default(),
            Actions::default(),
            ThisTick::default(),
            RigidBody::Kinematic,
        ))
        .with_children(|parent| {
            // The body: a capsule from the feet up, hit by the fire
            // system and walked into by other bodies.
            parent.spawn((
                body_shape(),
                Mesh3d(
                    spawn
                        .meshes
                        .add(Capsule3d::new(BODY_RADIUS, BODY_TOP - 2.0 * BODY_RADIUS)),
                ),
                MeshMaterial3d(spawn.materials.add(CelMaterial::from_srgb(0.6, 0.2, 0.2))),
                Transform::from_xyz(0.0, half - EYE_HEIGHT, 0.0),
            ));
        })
        .id();
    let Some((id, _row)) = spawn.actors.0.spawn(entity) else {
        // No row left: the actor never existed.
        spawn.commands.entity(entity).despawn();
        return None;
    };
    spawn.commands.entity(entity).insert(Actor(id));
    Some(entity)
}

/// A monument whose NPC spots have been filled.
#[derive(Component)]
pub struct Populated;

/// Which def stands at a spot of this danger.
pub fn def_for_danger(danger: u32) -> &'static str {
    if danger < 3 { "scavenger" } else { "raider" }
}

/// Run condition: a monument root still waits for its NPCs.
pub fn monuments_to_populate(roots: Query<(), (With<MonumentRoot>, Without<Populated>)>) -> bool {
    !roots.is_empty()
}

/// Fill every new monument's NPC spots through `spawn_actor`, once.
/// The NPCs hang under the monument root so despawning it takes
/// them too.
pub fn populate_monuments(
    mut spawn: SpawnActor,
    registry: Res<ActorRegistryRes>,
    roots: Query<(Entity, &MonumentRoot), Without<Populated>>,
) {
    for (root, monument) in &roots {
        for spot in &monument.npc_spots {
            let Some(def) = registry.0.def(def_for_danger(spot.danger)) else {
                continue;
            };
            // Face away from the building's front, toward where the
            // player walks in from.
            if let Some(npc) = spawn_actor(&mut spawn, def, spot.position, 0.0) {
                spawn.commands.entity(npc).insert(ChildOf(root));
            }
        }
        spawn.commands.entity(root).insert(Populated);
    }
}

/// The main inventory.
#[derive(Component)]
pub struct Inventory(pub item::Inventory);

impl Default for Inventory {
    fn default() -> Self {
        Self(item::Inventory::new(INVENTORY_SLOTS))
    }
}

/// The hotbar: its own small inventory, always on screen.
#[derive(Component)]
pub struct Hotbar(pub item::Inventory);

impl Default for Hotbar {
    fn default() -> Self {
        Self(item::Inventory::new(HOTBAR_SLOTS))
    }
}

/// Worn and held gear, one named slot each.
#[derive(Component, Default)]
pub struct Equipment(pub item::Equipment);
