//! Health, protection, and the one way a hit lands on an actor
//! (combat.md "How a hit is resolved"). modforge decides every hit
//! (`resolve_hit`) and what armor a person has (`Protection::worn`,
//! one total, no body parts); topside only finds who was hit and
//! carries out death.

use avian2d::prelude::*;
use bevy::prelude::*;
use modforge::combat::{
    BULLET_SPEED, FireTimer, Health as HealthStats, Hit, HitResult, MultiDamage, Protection as ProtectionStats,
    Swing, Trigger, pellet_directions_2d, resolve_hit,
};
use modforge::item::{self, ItemRegistry};

use crate::actor::{Actor, ActorMapRes, Dead, Facing, Hotbar, Inventory, Personality};
use crate::content::{DamageRegistryRes, ItemRegistryRes};
use crate::input::{Action, ActionQueue, ThisTick};
use crate::interact::{Container, spawn_item};
use crate::player::{PLAYER_SIZE, Player};

/// Health every actor starts with (the 3D game's number).
pub const START_HEALTH: f32 = 100.0;

#[derive(Component, Debug, PartialEq)]
pub struct Health(pub HealthStats);

impl Default for Health {
    fn default() -> Self {
        Self(HealthStats::new(START_HEALTH))
    }
}

/// A person's own protection before anything worn: armor and
/// resistances from what they are.
#[derive(Component, Default, Debug)]
pub struct Protection(pub ProtectionStats);

/// What a person wears.
#[derive(Component, Default)]
pub struct Equipment(pub item::Equipment);

/// The one path a hit lands on an actor: its protection is its own
/// plus every piece it wears, added into one total, then modforge's
/// damage function decides.
pub fn hit_actor(
    hit: &Hit<'_>,
    own: &Protection,
    worn: &Equipment,
    items: &ItemRegistry,
    health: &mut Health,
) -> HitResult {
    let mut protection = own.0.worn(&worn.0, |name| items.def(name).and_then(|d| d.armor));
    resolve_hit(hit, &mut protection, &mut health.0)
}

/// When this person may swing or fire next, and what they are
/// committed to (modforge's fire timer: Quake 3's weapon timer with
/// Gothic's beat).
#[derive(Component, Default, Debug, PartialEq)]
pub struct Firing(pub FireTimer);

/// How far the overhead steps in, in metres.
pub const OVERHEAD_STEP: f32 = 0.5;

/// Every actor with the attack held this tick and a melee weapon in
/// its hands makes one of Gothic's four moves (combat.md "Melee"): the
/// move held, read relative to where it faces, picks the swing;
/// modforge's timer decides whether it goes now (the beat, commitment);
/// everyone inside the swing's shape takes one hit through
/// `hit_actor`, unless they are blocking. The overhead steps in.
pub fn melee(
    time: Res<Time<Fixed>>,
    items: Res<ItemRegistryRes>,
    damage: Res<DamageRegistryRes>,
    move_and_slide: MoveAndSlide,
    mut actors: ParamSet<(
        Query<(
            Entity,
            &ThisTick,
            &Facing,
            &Hotbar,
            &Health,
            Option<&Personality>,
            &Collider,
            &mut Firing,
            &mut Transform,
        ), Without<Dead>>,
        Query<(Entity, &Transform, &Firing, &Protection, &Equipment, &mut Health), Without<Dead>>,
    )>,
) {
    let dt = time.delta_secs();
    // Every swing that goes this tick: who, from where, which way,
    // which move, how far, how hard, with what.
    let mut swings: Vec<(Entity, Vec2, Vec2, Swing, f32, f32, String)> = Vec::new();
    for (entity, this_tick, facing, hotbar, health, personality, shape, mut firing, mut transform) in &mut actors.p0() {
        firing.0.tick(dt);
        if !ActionQueue::has(&this_tick.0, Action::Attack) {
            // Let go: the next press may chain on the beat.
            firing.0.release();
            continue;
        }
        // What is in the hands: the held hotbar slot (Rust).
        let Some(weapon) = hotbar
            .0
            .held_stack()
            .and_then(|s| items.0.def(&s.item))
            .and_then(|d| d.combat.as_ref())
        else {
            continue;
        };
        // Guns are their own row; this is the hand weapons.
        if weapon.ammo.is_some() {
            continue;
        }
        let personality = personality.map(|p| p.0).unwrap_or_default();
        let (x, y) = ActionQueue::movement(&this_tick.0);
        let swing = Swing::from_ground_move(Vec2::new(x, y), facing.direction());
        let trigger = firing
            .0
            .pull_melee(personality.swing_delay(weapon.delay), swing, personality.can_chain());
        if trigger != Trigger::Fire {
            continue;
        }
        if swing == Swing::Overhead {
            let filter = SpatialQueryFilter::from_excluded_entities([entity]);
            let to = move_and_slide
                .move_and_slide(
                    shape,
                    transform.translation.truncate(),
                    0.0,
                    facing.direction() * OVERHEAD_STEP / dt,
                    time.delta(),
                    &MoveAndSlideConfig::default(),
                    &filter,
                    |_| MoveAndSlideHitResponse::Accept,
                )
                .position;
            transform.translation = to.extend(transform.translation.z);
        }
        swings.push((
            entity,
            transform.translation.truncate(),
            facing.direction(),
            swing,
            weapon.reach * personality.range_mult(),
            personality.damage_mult(health.0.fraction()),
            weapon.damage.clone(),
        ));
    }
    // Land them: one hit on everyone inside each swing's shape.
    for (attacker, here, facing, swing, reach, scale, damage_name) in swings {
        let Some(def) = damage.0.def(&damage_name) else {
            continue;
        };
        for (target, at, guard, own, target_worn, mut target_health) in &mut actors.p1() {
            let offset = at.translation.truncate() - here;
            if target == attacker || !swing.covers(offset, facing, reach) || guard.0.blocking() {
                continue;
            }
            let hit = Hit {
                def,
                self_inflicted: false,
                distance: offset.length(),
                location_scale: scale,
            };
            hit_actor(&hit, own, target_worn, &items.0, &mut target_health);
        }
    }
}

/// One bullet in flight (combat.md "Guns": every shot a visible bullet
/// moved in the fixed step, a ray from its last position to its new
/// one, no drop).
#[derive(Component)]
pub struct Bullet {
    pub shooter: Entity,
    pub damage: String,
    pub direction: Vec2,
    /// How far it has flown, for falloff and the maximum range.
    pub travelled: f32,
    pub range: f32,
}

const BULLET_SIZE: f32 = 0.18;
const BULLET_Z: f32 = 1.5;
const BULLET_COLOR: Color = Color::srgb(1.0, 0.85, 0.3);

/// Every actor with the trigger held and a gun in its hands fires
/// through modforge's timer: a shot takes one round of the gun's ammo
/// from the inventory (none left is the empty click) and sends one
/// bullet per pellet, spread by modforge's seeded pattern.
/// The fire timer is ticked once a step by `melee`, which runs before.
pub fn fire(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    items: Res<ItemRegistryRes>,
    mut shooters: Query<
        (Entity, &ThisTick, &Facing, &Hotbar, &Transform, &mut Firing, Option<&mut Inventory>),
        Without<Dead>,
    >,
) {
    for (entity, this_tick, facing, hotbar, transform, mut firing, inventory) in &mut shooters {
        let Some(gun) = hotbar
            .0
            .held_stack()
            .and_then(|s| items.0.def(&s.item))
            .and_then(|d| d.combat.as_ref())
        else {
            continue;
        };
        // Hand weapons are `melee`'s.
        let Some(ammo) = gun.ammo.as_deref() else {
            continue;
        };
        if !ActionQueue::has(&this_tick.0, Action::Attack) {
            continue;
        }
        let mut inventory = inventory;
        let round = inventory.as_mut().and_then(|inv| {
            inv.0
                .slots
                .iter()
                .position(|s| s.as_ref().is_some_and(|s| s.item == ammo))
        });
        if firing.0.pull(gun.delay, round.is_some()) != Trigger::Fire {
            continue;
        }
        if let (Some(inv), Some(slot)) = (inventory.as_mut(), round) {
            inv.0.remove(slot, 1);
        }
        let muzzle = transform.translation.truncate() + facing.direction() * (PLAYER_SIZE / 2.0 + 0.1);
        let seed = tick.0 ^ entity.to_bits();
        for direction in pellet_directions_2d(facing.direction(), gun.spread_degrees, gun.pellets, seed) {
            commands.spawn((
                Bullet {
                    shooter: entity,
                    damage: gun.damage.clone(),
                    direction,
                    travelled: 0.0,
                    range: gun.reach,
                },
                Sprite {
                    color: BULLET_COLOR,
                    custom_size: Some(Vec2::splat(BULLET_SIZE)),
                    ..default()
                },
                Transform::from_translation(muzzle.extend(BULLET_Z)),
            ));
        }
    }
}

/// Every bullet flies one fixed step: a ray from where it is to where
/// it will be. Items lying about do not stop it; a wall does; a person
/// takes the hit. Pellets that land on one person in one step are one
/// hit (modforge's `MultiDamage`), through `hit_actor`. Nothing flies
/// past its range.
pub fn fly(
    mut commands: Commands,
    time: Res<Time<Fixed>>,
    items: Res<ItemRegistryRes>,
    damage: Res<DamageRegistryRes>,
    spatial: SpatialQuery,
    sensors: Query<(), With<Sensor>>,
    mut bullets: Query<(Entity, &mut Bullet, &mut Transform)>,
    mut targets: Query<(&Protection, &Equipment, &mut Health), Without<Dead>>,
) {
    let step = BULLET_SPEED * time.delta_secs();
    // (damage def, pellets summed per target) this step.
    let mut landed: Vec<(String, MultiDamage<Entity>)> = Vec::new();
    for (entity, mut bullet, mut transform) in &mut bullets {
        let from = transform.translation.truncate();
        let reach = step.min(bullet.range - bullet.travelled);
        let filter = SpatialQueryFilter::from_excluded_entities([bullet.shooter]);
        let hit = Dir2::new(bullet.direction).ok().and_then(|dir| {
            spatial.cast_ray_predicate(from, dir, reach, true, &filter, &|e| !sensors.contains(e))
        });
        if let Some(hit) = hit {
            if targets.contains(hit.entity) {
                let at = bullet.travelled + hit.distance;
                match landed.iter_mut().find(|(d, _)| *d == bullet.damage) {
                    Some((_, multi)) => multi.add(hit.entity, at),
                    None => {
                        let mut multi = MultiDamage::default();
                        multi.add(hit.entity, at);
                        landed.push((bullet.damage.clone(), multi));
                    }
                }
            }
            commands.entity(entity).despawn();
            continue;
        }
        bullet.travelled += reach;
        transform.translation = (from + bullet.direction * reach).extend(transform.translation.z);
        if bullet.travelled >= bullet.range - 1e-4 {
            commands.entity(entity).despawn();
        }
    }
    for (name, mut multi) in landed {
        let Some(def) = damage.0.def(&name) else {
            continue;
        };
        for (target, pellets, distance) in multi.drain() {
            let Ok((own, worn, mut health)) = targets.get_mut(target) else {
                continue;
            };
            let hit = Hit {
                def,
                self_inflicted: false,
                distance,
                location_scale: pellets as f32,
            };
            hit_actor(&hit, own, worn, &items.0, &mut health);
        }
    }
}

/// The colour of the box a dead person leaves.
const DEATH_BOX_COLOR: [f32; 3] = [0.35, 0.25, 0.2];

/// Every actor at zero health dies the same way, the player included
/// (design.md "Death and difficulty": everyone is equal): everything it
/// carried (inventory, hotbar, worn gear) goes into one storage box
/// where it fell (modforge's `loot_all`, a box through `spawn_item`).
/// An NPC then leaves the actor map (its id never returns) and the
/// world; the player wakes in the bunker with nothing.
pub fn die(
    mut commands: Commands,
    items: Res<ItemRegistryRes>,
    tick: Res<crate::SimTick>,
    mut map: ResMut<ActorMapRes>,
    mut dying: Query<(
        Entity,
        &mut Health,
        &Transform,
        Has<Player>,
        Option<&Actor>,
        Option<&mut Firing>,
        Option<&mut Inventory>,
        Option<&mut Hotbar>,
        Option<&mut Equipment>,
    )>,
) {
    for (entity, mut health, transform, is_player, actor, firing, inventory, hotbar, worn) in &mut dying {
        if health.0.current > 0.0 {
            continue;
        }
        if let (Some(mut inventory), Some(mut hotbar), Some(mut worn)) = (inventory, hotbar, worn) {
            let loot = item::loot_all(&mut inventory.0, &mut hotbar.0.slots, &mut worn.0);
            if !loot.is_empty()
                && let Some(box_def) = items.0.def("storage box")
            {
                // A death box holds everything, however much that is.
                let mut contents = item::Inventory::new(loot.len().max(box_def.storage.unwrap_or(1)));
                for stack in loot {
                    let max = items.0.def(&stack.item).map_or(1, |d| d.max_stack);
                    contents.add(stack, max);
                }
                let salt = actor.map_or(0, |a| a.0.0);
                let dropped = spawn_item(
                    &mut commands,
                    box_def,
                    item::create(box_def, 1, &[], tick.0 as f32, salt),
                    transform.translation.truncate(),
                    DEATH_BOX_COLOR,
                );
                commands.entity(dropped).insert(Container(contents));
            }
        }
        if is_player {
            // The death message and the choice to respawn (design.md
            // "Death and difficulty"); the world keeps running meanwhile.
            health.0.current = health.0.max;
            if let Some(mut firing) = firing {
                *firing = Firing::default();
            }
            commands
                .entity(entity)
                .insert((Dead, Visibility::Hidden, ColliderDisabled));
            continue;
        }
        if let Some(actor) = actor {
            map.0.remove(actor.0);
        }
        commands.entity(entity).despawn();
    }
}
