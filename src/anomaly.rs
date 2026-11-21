//! The first teeth (todo 10i1; design.md "The first hour" rule 5, "Maze,
//! mined"): one anomaly, a burner, its tell scorched ground, placed near
//! the second site from the bunker; anyone who walks into it is hit
//! through the one hit path (`combat::hit_actor`, modforge `resolve_hit`)
//! and dies, player and NPC alike.

use bevy::prelude::*;
use modforge::combat::Hit;
use modforge::world2d::Registries;

use crate::actor::{Dead, Persona};
use crate::combat::{Equipment, Health, Protection, hit_actor};
use crate::content::{
    ANOMALY, ANOMALY_FROM_SITE, ANOMALY_RADIUS, BOLT, BOLT_REACH, BOLT_SPEED, BiomeRegistryRes, DamageRegistryRes, GOES_OFF_SECS,
    ItemRegistryRes, MonumentRegistryRes,
};
use crate::terrain::{WorldPiece, WorldRes};
use crate::{Ground as _, on_ground};

/// A patch of ground that kills whoever enters it. Goes with its world
/// (`WorldPiece`), so a storm takes it and the next world has its own.
#[derive(Component)]
pub struct Anomaly;

/// The world (its seed) the anomaly was placed in, so it is placed once a
/// world.
#[derive(Resource, Default)]
pub struct AnomalyPlaced(pub Option<u64>);

/// How far round the bunker the sites are looked for (Claude's, for the
/// operator to change).
const LOOK: f32 = 600.0;

/// How far above or below the anomaly someone can be and still be in it.
const DEPTH: f32 = 2.0;

/// Once a world, after the player wakes: the anomaly is placed between
/// the second nearest site and the bunker, just outside the site's
/// ground. A world with fewer than two sites has none.
pub fn place(
    mut commands: Commands,
    mut placed: ResMut<AnomalyPlaced>,
    mut world: ResMut<WorldRes>,
    biomes: Res<BiomeRegistryRes>,
    monuments: Res<MonumentRegistryRes>,
) {
    if placed.0 == Some(world.seed) {
        return;
    }
    placed.0 = Some(world.seed);
    let bunker = world.bunker;
    let r = Registries {
        biomes: &biomes.0,
        monuments: &monuments.monuments,
        buildings: &monuments.buildings,
    };
    let mut sites = world.world.sites_in(r, bunker - Vec2::splat(LOOK), bunker + Vec2::splat(LOOK));
    sites.sort_by(|a, b| a.position.distance(bunker).total_cmp(&b.position.distance(bunker)));
    let Some(second) = sites.get(1) else {
        warn!("anomaly: fewer than two sites near the bunker; no anomaly in this world");
        return;
    };
    let toward = (bunker - second.position).normalize_or_zero();
    let at = second.position + toward * (second.ground + ANOMALY_FROM_SITE);
    info!("anomaly: a {ANOMALY} at {at}, near the {} at {}", second.monument, second.position);
    commands.spawn((
        Anomaly,
        WorldPiece,
        crate::model::Model(crate::content::ANOMALY_TELL.to_string()),
        Transform::from_translation(on_ground(at, 0.0)),
    ));
}

/// A bolt in the air (todo 10i2): where it flies on the ground, how far it
/// has come, and the height it flies and lands at.
#[derive(Component)]
pub struct Bolt {
    pub direction: Vec2,
    pub travelled: f32,
    pub height: f32,
}

/// An anomaly a bolt has set off, for this many seconds more.
#[derive(Component)]
pub struct GoingOff(pub f32);

/// The tick someone last threw, so one press throws one bolt.
#[derive(Component)]
pub struct Threw(pub u64);

/// The least time between two throws, in ticks (Claude's).
const THROW_EVERY: u64 = 36;

/// Anyone attacking with a bolt in their hands throws it ahead, where
/// they face (design.md "Maze, mined": "throwing a bolt is an action on
/// the queue like any other"); the one in the hands is used up.
pub fn throw(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    mut throwers: Query<
        (Entity, &crate::input::ThisTick, &crate::actor::Facing, &Transform, &mut crate::actor::Hotbar, Option<&Threw>),
        Without<Dead>,
    >,
) {
    for (entity, this_tick, facing, at, mut hotbar, threw) in &mut throwers {
        if !crate::input::ActionQueue::has(&this_tick.0, crate::input::Action::Attack)
            || threw.is_some_and(|t| tick.0 < t.0 + THROW_EVERY)
        {
            continue;
        }
        let Some(slot) = hotbar.0.held.filter(|_| hotbar.0.held_stack().is_some_and(|s| s.item == BOLT)) else {
            continue;
        };
        hotbar.0.slots.remove(slot, 1);
        commands.entity(entity).insert(Threw(tick.0));
        let height = crate::interact::feet_of(at).y;
        let from = at.translation.ground() + facing.direction() * 0.5;
        commands.spawn((
            Bolt { direction: facing.direction(), travelled: 0.0, height },
            crate::model::Model(BOLT.to_string()),
            Transform::from_translation(on_ground(from, height + 1.2)),
        ));
    }
}

/// Every bolt in the air flies a fixed step. It lands at its reach, or in
/// the first anomaly it comes over, and that anomaly goes off; landed, it
/// is a bolt lying on the ground to pick up again.
#[allow(clippy::too_many_arguments)]
pub fn fly(
    mut commands: Commands,
    time: Res<Time<Fixed>>,
    items: Res<ItemRegistryRes>,
    mut ids: ResMut<crate::interact::ThingIssuer>,
    tick: Res<crate::SimTick>,
    anomalies: Query<(Entity, &Transform), (With<Anomaly>, Without<Bolt>)>,
    mut bolts: Query<(Entity, &mut Bolt, &mut Transform)>,
) {
    let step = BOLT_SPEED * time.delta_secs();
    for (entity, mut bolt, mut transform) in &mut bolts {
        let at = transform.translation.ground() + bolt.direction * step;
        bolt.travelled += step;
        let left = 1.0 - (bolt.travelled / BOLT_REACH).min(1.0);
        transform.translation = on_ground(at, bolt.height + 1.2 * left);
        let into = anomalies.iter().find(|(_, a)| a.translation.ground().distance(at) < ANOMALY_RADIUS);
        if into.is_none() && bolt.travelled < BOLT_REACH {
            continue;
        }
        if let Some((anomaly, _)) = into {
            info!("anomaly: a bolt sets the {ANOMALY} off at {at}");
            commands.entity(anomaly).insert(GoingOff(GOES_OFF_SECS));
        }
        commands.entity(entity).despawn();
        if let Some(def) = items.0.def(BOLT) {
            let stack = modforge::item::create(def, 1, &[], 0.0, tick.0);
            crate::interact::spawn_item(&mut commands, &mut ids, def, stack, on_ground(at, bolt.height));
        }
    }
}

/// An anomaly going off flares: its tell swells up while it lasts.
pub fn flare(mut commands: Commands, time: Res<Time<Fixed>>, mut going: Query<(Entity, &mut GoingOff, &mut Transform)>) {
    for (entity, mut off, mut transform) in &mut going {
        off.0 -= time.delta_secs();
        if off.0 <= 0.0 {
            transform.scale = Vec3::ONE;
            commands.entity(entity).remove::<GoingOff>();
        } else {
            transform.scale = Vec3::new(1.4, 6.0, 1.4);
        }
    }
}

/// Every tick, anyone living inside an anomaly takes its hit.
pub fn burn(
    tick: Res<crate::SimTick>,
    items: Res<ItemRegistryRes>,
    layers: Res<crate::content::LayerRegistryRes>,
    damage: Res<DamageRegistryRes>,
    anomalies: Query<&Transform, With<Anomaly>>,
    mut actors: Query<(&Transform, &Protection, &Equipment, &mut Health, Option<&mut Persona>), (Without<Dead>, Without<Anomaly>)>,
) {
    let Some(def) = damage.0.def(ANOMALY) else {
        return;
    };
    for anomaly in &anomalies {
        for (at, own, worn, mut health, mut memory) in &mut actors {
            let inside = at.translation.ground().distance(anomaly.translation.ground()) < ANOMALY_RADIUS
                && (at.translation.y - anomaly.translation.y).abs() < DEPTH;
            if !inside || health.0.current <= 0.0 {
                continue;
            }
            let hit = Hit {
                def,
                self_inflicted: false,
                distance: 0.0,
                location_scale: 1.0,
            };
            hit_actor(&hit, own, worn, &items.0, &layers.0, Default::default(), &mut health, memory.as_deref_mut(), None, tick.0);
        }
    }
}
