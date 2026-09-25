//! Health, protection, and the one way a hit lands on an actor
//! (combat.md "How a hit is resolved"). modforge decides every hit
//! (`resolve_hit`) and what armor a person has (`Protection::worn`,
//! one total, no body parts); topside only finds who was hit and
//! carries out death.

use avian2d::prelude::*;
use bevy::prelude::*;
use modforge::combat::{
    FireTimer, Health as HealthStats, Hit, HitResult, Protection as ProtectionStats, Swing, Trigger,
    resolve_hit,
};
use modforge::item::{self, EquipSlot, ItemRegistry};

use crate::actor::{Actor, ActorMapRes, Facing, Personality};
use crate::content::{DamageRegistryRes, ItemRegistryRes};
use crate::input::{Action, ActionQueue, ThisTick};

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
            &Equipment,
            &Health,
            Option<&Personality>,
            &Collider,
            &mut Firing,
            &mut Transform,
        )>,
        Query<(Entity, &Transform, &Firing, &Protection, &Equipment, &mut Health)>,
    )>,
) {
    let dt = time.delta_secs();
    // Every swing that goes this tick: who, from where, which way,
    // which move, how far, how hard, with what.
    let mut swings: Vec<(Entity, Vec2, Vec2, Swing, f32, f32, String)> = Vec::new();
    for (entity, this_tick, facing, worn, health, personality, shape, mut firing, mut transform) in &mut actors.p0() {
        firing.0.tick(dt);
        if !ActionQueue::has(&this_tick.0, Action::Attack) {
            // Let go: the next press may chain on the beat.
            firing.0.release();
            continue;
        }
        let Some(weapon) = worn
            .0
            .get(EquipSlot::Weapon)
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

/// Every actor at zero health dies: it leaves the actor map (its id
/// never returns) and its body leaves the world.
pub fn die(mut commands: Commands, mut map: ResMut<ActorMapRes>, actors: Query<(Entity, &Health, Option<&Actor>)>) {
    for (entity, health, actor) in &actors {
        if health.0.current <= 0.0 {
            if let Some(actor) = actor {
                map.0.remove(actor.0);
            }
            commands.entity(entity).despawn();
        }
    }
}
