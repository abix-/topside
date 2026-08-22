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
use modforge::item;

use crate::cel::CelMaterial;
use crate::combat::{Firing, Health, Protection};
use crate::content::{ActorRegistryRes, FactionRegistryRes, ItemRegistryRes};
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

/// The world seed every person's traits are drawn with. Set by the
/// world roll; zero in headless tests that roll no world.
#[derive(Resource, Default)]
pub struct WorldSeed(pub u64);

/// A person's seven axes (modforge), on every actor. The player's
/// are all zero until the RPG lands.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq)]
pub struct Personality(pub modforge::actor::Personality);

/// What this person knows (modforge::memory): the things it has
/// seen, who hurt it, the last threat. The brain reads only this.
#[derive(Component, Default, Debug, Clone, PartialEq)]
pub struct Memory(pub modforge::memory::Memory);

/// Where this person sleeps and returns to: its home site's root and
/// position. None for a drifter.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Home {
    pub root: Entity,
    pub position: Vec3,
}

/// An NPC: which def it was spawned from and its faction. The player
/// has no `Npc`; everything else on an actor is shared (see
/// `spawn_actor`). Identity is the `Actor` component beside it.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Npc {
    pub def: String,
    /// A registered faction, or None for a neutral.
    pub faction: Option<String>,
}

/// What the one spawn path needs from the world.
#[derive(bevy::ecs::system::SystemParam)]
pub struct SpawnActor<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub meshes: ResMut<'w, Assets<Mesh>>,
    pub materials: ResMut<'w, Assets<CelMaterial>>,
    pub items: Res<'w, ItemRegistryRes>,
    pub factions: Res<'w, FactionRegistryRes>,
    pub actors: ResMut<'w, ActorMapRes>,
    pub seed: Res<'w, WorldSeed>,
}

/// The ONE path through which an NPC enters the world (authority.md:
/// Endless's materialize_npc discipline). Monument NPC spots, edge
/// arrivals, saves, and tests all come here. The actor gets exactly
/// what the player has (inventory, hotbar, equipment, survival,
/// health, protection, a fire timer, an action queue) plus a body
/// the world can hit and its `ActorId` from the one map. `feet` is
/// where it stands. `faction` overrides the def's (a placed actor
/// takes its placer's). Returns None when the map has no row left or
/// the faction is not registered.
pub fn spawn_actor(
    spawn: &mut SpawnActor,
    def: &ActorDef,
    feet: Vec3,
    yaw: f32,
    faction: Option<&str>,
) -> Option<Entity> {
    let faction = faction.map(String::from).or_else(|| def.faction.clone());
    if let Some(name) = faction.as_deref()
        && spawn.factions.0.def(name).is_none()
    {
        return None;
    }
    let facing = Facing { yaw, pitch: 0.0 };
    // The weapon goes in the first bar slot and into the hands, the
    // same way the player holds one.
    let mut hotbar = Hotbar::default();
    if let Some(weapon) = def.weapon.as_deref().and_then(|w| spawn.items.0.def(w)) {
        let stack = item::create(weapon, 1, &[], 0.0, 0);
        hotbar.0.slots.slots[0] = Some(stack);
        hotbar.0.held = Some(0);
    }
    let entity = spawn
        .commands
        .spawn((
            Npc {
                def: def.name.clone(),
                faction,
            },
            Transform::from_translation(feet + Vec3::Y * EYE_HEIGHT).with_rotation(facing.rotation()),
            Visibility::default(),
            facing,
            Inventory::default(),
            hotbar,
            Equipment::default(),
            // The life: needs, what is true now, what it knows, what
            // it is doing, whether it fights.
            (
                Survival::default(),
                crate::survival::ActorCondition::default(),
                Memory::default(),
                crate::ai::ActorActivity::default(),
                crate::ai::ActorCombat::default(),
            ),
            Protection(def.protection.clone()),
            Firing::default(),
            Actions::default(),
            ThisTick::default(),
            crate::player::Fall::default(),
            RigidBody::Kinematic,
        ))
        .id();
    spawn_body(
        &mut spawn.commands,
        entity,
        Some((&mut spawn.meshes, &mut spawn.materials)),
    );
    let Some((id, _row)) = spawn.actors.0.spawn(entity) else {
        // No row left: the actor never existed.
        spawn.commands.entity(entity).despawn();
        return None;
    };
    // Who this person is, drawn from the genome by id: the axes and
    // the health they give.
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

/// The body that hangs under an actor's eyes: a capsule from the
/// feet up that the fire system hits and other bodies walk into. The
/// ONE body for every actor; the player's has no mesh (the camera
/// sits inside it). Returns the body entity.
pub fn spawn_body(
    commands: &mut Commands,
    actor: Entity,
    visuals: Option<(&mut Assets<Mesh>, &mut Assets<CelMaterial>)>,
) -> Entity {
    let half = BODY_TOP / 2.0;
    let mut body = commands.spawn((
        Body,
        body_shape(),
        Transform::from_xyz(0.0, half - EYE_HEIGHT, 0.0),
        ChildOf(actor),
    ));
    if let Some((meshes, materials)) = visuals {
        body.insert((
            Mesh3d(meshes.add(Capsule3d::new(BODY_RADIUS, BODY_TOP - 2.0 * BODY_RADIUS))),
            MeshMaterial3d(materials.add(CelMaterial::from_srgb(0.6, 0.2, 0.2))),
        ));
    }
    body.id()
}

/// Marks an actor's body collider (the child under its root).
#[derive(Component)]
pub struct Body;

/// Run condition: some actor other than the player is at zero.
pub fn any_dead(actors: Query<&Health, (With<Actor>, Without<crate::player::Player>)>) -> bool {
    actors.iter().any(|h| h.0.is_dead())
}

/// Every actor but the player at zero health dies here, NPC, animal,
/// turret, or vehicle alike (todo 11c): its three holders empty into
/// one storage box where it stood (a deployed WorldItem through
/// `spawn_item`, like every box), it leaves the `ActorMap` (the row
/// is freed, the id never returns), and the body despawns with its
/// children. The player's death is its own row.
pub fn die(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CelMaterial>>,
    items: Res<ItemRegistryRes>,
    time: Res<Time<Fixed>>,
    mut actors: ResMut<ActorMapRes>,
    mut dying: Query<
        (Entity, &Actor, &Transform, &Health, &mut Inventory, &mut Hotbar, &mut Equipment),
        Without<crate::player::Player>,
    >,
) {
    for (entity, actor, transform, health, mut inventory, mut hotbar, mut equipment) in &mut dying {
        if !health.0.is_dead() {
            continue;
        }
        let loot = item::loot_all(&mut inventory.0, &mut hotbar.0.slots, &mut equipment.0);
        if !loot.is_empty()
            && let Some(box_def) = items.0.def("storage box")
        {
            // A death box holds everything, however much that is.
            let mut contents = item::Inventory::new(loot.len().max(box_def.storage.unwrap_or(1)));
            for stack in loot {
                let max = items.0.def(&stack.item).map_or(1, |d| d.max_stack);
                contents.add(stack, max);
            }
            let feet = transform.translation - Vec3::Y * EYE_HEIGHT;
            let box_entity = crate::interact::spawn_item(
                &mut commands,
                &mut meshes,
                &mut materials,
                box_def,
                item::create(box_def, 1, &[], time.elapsed_secs(), actor.0.0),
                feet + Vec3::Y * 0.35,
                [0.35, 0.25, 0.2],
            );
            commands.entity(box_entity).insert(crate::interact::Container(contents));
        }
        actors.0.remove(actor.0);
        commands.entity(entity).despawn();
    }
}

/// Every new player gets the same body an NPC gets, so NPC swings
/// land on it. No mesh: the camera sits inside.
pub fn body_for_new_players(
    mut commands: Commands,
    players: Query<Entity, Added<crate::player::Player>>,
) {
    for player in &players {
        commands.entity(player).insert(RigidBody::Kinematic);
        spawn_body(&mut commands, player, None);
    }
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
    tick: Res<crate::SimTick>,
    roots: Query<(Entity, &MonumentRoot, Option<&Children>), Without<Populated>>,
    boxes: Query<(&Transform, &crate::interact::WorldItem), With<crate::interact::Container>>,
) {
    for (root, monument, children) in &roots {
        // What a person who lives here knows from the start: the
        // place itself and the boxes in it (life.md "Home").
        let mut home_memory = modforge::memory::Memory::default();
        home_memory.see(root.to_bits(), &monument.kind, monument.origin, tick.0);
        for child in children.into_iter().flat_map(|c| c.iter()) {
            if let Ok((transform, item)) = boxes.get(child)
                && let Some(def) = spawn.items.0.def(&item.stack.item)
            {
                home_memory.see(child.to_bits(), &def.name, transform.translation, tick.0);
            }
        }
        for spot in &monument.npc_spots {
            let Some(def) = registry.0.def(def_for_danger(spot.danger)) else {
                continue;
            };
            // Face away from the building's front, toward where the
            // player walks in from.
            if let Some(npc) = spawn_actor(&mut spawn, def, spot.position, 0.0, None) {
                spawn.commands.entity(npc).insert((
                    ChildOf(root),
                    Home {
                        root,
                        position: spot.position,
                    },
                    Memory(home_memory.clone()),
                ));
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

/// The hotbar: its own small inventory, always on screen, with the
/// slot in the hands (Rust: the held item stays in the bar).
#[derive(Component, Default)]
pub struct Hotbar(pub modforge::hud::Hotbar);

/// Worn and held gear, one named slot each.
#[derive(Component, Default)]
pub struct Equipment(pub item::Equipment);
