//! Topside's binding for modforge::combat: the actor components and
//! the one fire system. modforge decides what a hit does and when a
//! shooter may fire; this file casts rays in the Avian world and
//! routes the hit to a piece or an actor (design.md "How a hit
//! works"). The player and every NPC fire through the same system;
//! only who sets `Attacking` differs.

use avian3d::prelude::*;
use bevy::prelude::*;
use modforge::combat::{BodyArea, FireTimer, Hit, MultiDamage, Trigger, pellet_directions};
pub use modforge::combat::{Health as HealthStats, Protection as ProtectionStats};

use crate::SimTick;
use crate::actor::{Equipment, Hotbar, Inventory};
use crate::content::{DamageRegistryRes, ItemRegistryRes};
use crate::input::{Action, ActionQueue, ThisTick};
use crate::player::EYE_HEIGHT;
use crate::structure::{BODY_TOP, Pieces, StructureRoot, damage_at};

/// Health on any actor: the player and every NPC.
#[derive(Component, Debug, PartialEq)]
pub struct Health(pub HealthStats);

impl Default for Health {
    fn default() -> Self {
        Self(HealthStats::new(100.0))
    }
}

/// What stands between a hit and this actor's health.
#[derive(Component, Default, Debug, PartialEq)]
pub struct Protection(pub ProtectionStats);

/// When this actor may fire next (Quake 3's weapon timer).
#[derive(Component, Default, Debug, PartialEq)]
pub struct Firing(pub FireTimer);

/// Melee misses by a hair feel wrong, so after the line misses a
/// fat sweep gets a second chance (Half-Life's crowbar: trace a
/// line, then a hull).
const SWING_HALF_WIDTH: f32 = 0.25;

/// The fire system. For every actor holding a weapon with `Attack`
/// among this tick's actions and the timer ready: cast from the eye
/// along its facing for the weapon's reach; on a miss sweep a box
/// the same way; resolve the hit on what it touched. The keyboard
/// and an NPC's AI push the same `Attack`.
pub fn fire(
    time: Res<Time<Fixed>>,
    tick: Res<SimTick>,
    items: Res<ItemRegistryRes>,
    damage: Res<DamageRegistryRes>,
    spatial: SpatialQuery,
    mut commands: Commands,
    mut shooters: Query<(
        Entity,
        &Transform,
        &ThisTick,
        &mut Firing,
        &mut Inventory,
        &mut Hotbar,
        Option<&Children>,
    )>,
    mut structures: Query<(&Transform, &mut Pieces), With<StructureRoot>>,
    mut actors: Query<(&Transform, &Equipment, &mut Health, &mut Protection)>,
    bodies: Query<&ChildOf>,
) {
    let dt = time.delta_secs();
    let armor_of = |name: &str| items.0.def(name).and_then(|d| d.armor);
    for (shooter, transform, this_tick, mut firing, mut inventory, mut hotbar, children) in
        &mut shooters
    {
        firing.0.tick(dt);
        if !ActionQueue::has(&this_tick.0, Action::Attack) {
            continue;
        }
        // What is in the hands: the held bar slot (Rust).
        let Some(weapon) = hotbar
            .0
            .held_stack()
            .and_then(|stack| items.0.def(&stack.item))
            .and_then(|def| def.combat.as_ref())
        else {
            continue;
        };
        let Some(def) = damage.0.def(&weapon.damage) else {
            continue;
        };
        // A gun needs a round: from the hotbar first, then the bag.
        // Melee always has one.
        let has_ammo = match weapon.ammo.as_deref() {
            None => true,
            Some(ammo) => hotbar.0.slots.count_of(ammo) > 0 || inventory.0.count_of(ammo) > 0,
        };
        if firing.0.pull(weapon.delay, has_ammo) != Trigger::Fire {
            continue;
        }
        if let Some(ammo) = weapon.ammo.as_deref() {
            let _ = hotbar.0.slots.take_one(ammo) || inventory.0.take_one(ammo);
        }

        let eye = transform.translation;
        let aim = transform.forward().as_vec3();
        let own: Vec<Entity> = std::iter::once(shooter)
            .chain(children.into_iter().flat_map(|c| c.iter()))
            .collect();
        let filter = SpatialQueryFilter::from_excluded_entities(own);
        let mut pellets = MultiDamage::default();
        let mut points: Vec<(Entity, Vec3)> = Vec::new();
        for dir in pellet_directions(aim, weapon.spread_degrees, weapon.pellets, tick.0) {
            let forward = Dir3::new(dir).unwrap_or(transform.forward());
            // The line first; melee gets the fat sweep on a miss.
            let hit = spatial
                .cast_ray(eye, forward, weapon.reach, true, &filter)
                .map(|h| (h.entity, eye + forward * h.distance, h.distance))
                .or_else(|| {
                    if weapon.ammo.is_some() {
                        return None;
                    }
                    spatial
                        .cast_shape(
                            &Collider::cuboid(
                                SWING_HALF_WIDTH * 2.0,
                                SWING_HALF_WIDTH * 2.0,
                                SWING_HALF_WIDTH * 2.0,
                            ),
                            eye,
                            Quat::IDENTITY,
                            forward,
                            &ShapeCastConfig::from_max_distance(weapon.reach),
                            &filter,
                        )
                        .map(|h| (h.entity, h.point1, h.distance))
                });
            if let Some((target, point, distance)) = hit {
                pellets.add(target, distance);
                if !points.iter().any(|(e, _)| *e == target) {
                    points.push((target, point));
                }
            }
        }
        // Every pellet on one target lands as one hit (Half-Life).
        for (target, count, distance) in pellets.drain() {
            let point = points
                .iter()
                .find(|(e, _)| *e == target)
                .map(|(_, p)| *p)
                .unwrap_or(eye);
            let mut swing = Hit {
                def,
                self_inflicted: target == shooter,
                distance,
                location_scale: count as f32,
            };
            if let Ok((root_transform, mut pieces)) = structures.get_mut(target) {
                damage_at(&mut commands, target, root_transform, &mut pieces, point, &swing);
                continue;
            }
            // An actor's body hangs under its root as a child; the
            // health lives on the root.
            let actor = match bodies.get(target) {
                Ok(child_of) => child_of.parent(),
                Err(_) => target,
            };
            let Ok((t, worn, mut health, protection)) = actors.get_mut(actor) else {
                continue;
            };
            // Where on the body, and what is worn there.
            let feet = t.translation.y - EYE_HEIGHT;
            let area = BodyArea::from_height((point.y - feet) / BODY_TOP);
            swing.location_scale *= area.scale();
            let mut guarded = protection.0.for_area(area, &worn.0, armor_of);
            modforge::combat::resolve_hit(&swing, &mut guarded, &mut health.0);
        }
    }
}

/// The player's eye sits at EYE_HEIGHT above the feet; exported so
/// tests can aim.
pub const EYE: f32 = EYE_HEIGHT;
