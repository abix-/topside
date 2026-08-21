//! Topside's binding for modforge::combat: the actor components and
//! the one fire system. modforge decides what a hit does and when a
//! shooter may fire; this file casts rays in the Avian world and
//! routes the hit to a piece or an actor (design.md "How a hit
//! works"). The player and every NPC fire through the same system;
//! only who sets `Attacking` differs.

use avian3d::prelude::*;
use bevy::prelude::*;
use modforge::combat::{FireTimer, Hit, Trigger};
use modforge::item::EquipSlot;
pub use modforge::combat::{Health as HealthStats, Protection as ProtectionStats};

use crate::actor::Equipment;
use crate::content::{DamageRegistryRes, ItemRegistryRes};
use crate::input::{Action, ActionQueue, ThisTick};
use crate::player::EYE_HEIGHT;
use crate::structure::{Pieces, StructureRoot, damage_at};

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
    items: Res<ItemRegistryRes>,
    damage: Res<DamageRegistryRes>,
    spatial: SpatialQuery,
    mut commands: Commands,
    mut shooters: Query<(Entity, &Transform, &Equipment, &ThisTick, &mut Firing)>,
    mut structures: Query<(&Transform, &mut Pieces), With<StructureRoot>>,
    mut actors: Query<(&mut Health, &mut Protection)>,
) {
    let dt = time.delta_secs();
    for (shooter, transform, equipment, this_tick, mut firing) in &mut shooters {
        firing.0.tick(dt);
        if !ActionQueue::has(&this_tick.0, Action::Attack) {
            continue;
        }
        let Some(weapon) = equipment
            .0
            .get(EquipSlot::Weapon)
            .and_then(|stack| items.0.def(&stack.item))
            .and_then(|def| def.combat.as_ref())
        else {
            continue;
        };
        let Some(def) = damage.0.def(&weapon.damage) else {
            continue;
        };
        // Melee and loaded guns have ammo; ammo counting lands with guns.
        if firing.0.pull(weapon.delay, true) != Trigger::Fire {
            continue;
        }

        let eye = transform.translation;
        let forward = transform.forward();
        let filter = SpatialQueryFilter::from_excluded_entities([shooter]);
        // The line first, then the fat sweep.
        let hit = spatial
            .cast_ray(eye, forward, weapon.reach, true, &filter)
            .map(|h| (h.entity, eye + forward * h.distance, h.distance))
            .or_else(|| {
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
        let Some((target, point, distance)) = hit else {
            continue;
        };
        let swing = Hit {
            def,
            self_inflicted: target == shooter,
            distance,
            location_scale: 1.0,
        };
        if let Ok((root_transform, mut pieces)) = structures.get_mut(target) {
            damage_at(&mut commands, target, root_transform, &mut pieces, point, &swing);
        } else if let Ok((mut health, mut protection)) = actors.get_mut(target) {
            modforge::combat::resolve_hit(&swing, &mut protection.0, &mut health.0);
        }
    }
}

/// The player's eye sits at EYE_HEIGHT above the feet; exported so
/// tests can aim.
pub const EYE: f32 = EYE_HEIGHT;
