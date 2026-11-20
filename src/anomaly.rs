//! The first teeth (todo 10i1; design.md "The first hour" rule 5, "Maze,
//! mined"): one anomaly, a burner, its tell scorched ground, placed near
//! the second site from the bunker; anyone who walks into it is hit
//! through the one hit path (`combat::hit_actor`, modforge `resolve_hit`)
//! and dies, player and NPC alike.

use bevy::prelude::*;
use modforge::combat::Hit;
use modforge::world2d::Registries;

use crate::actor::{Dead, Memory};
use crate::combat::{Equipment, Health, Protection, hit_actor};
use crate::content::{ANOMALY, ANOMALY_FROM_SITE, ANOMALY_RADIUS, BiomeRegistryRes, DamageRegistryRes, ItemRegistryRes, MonumentRegistryRes};
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

/// Every tick, anyone living inside an anomaly takes its hit.
pub fn burn(
    tick: Res<crate::SimTick>,
    items: Res<ItemRegistryRes>,
    layers: Res<crate::content::LayerRegistryRes>,
    damage: Res<DamageRegistryRes>,
    anomalies: Query<&Transform, With<Anomaly>>,
    mut actors: Query<(&Transform, &Protection, &Equipment, &mut Health, Option<&mut Memory>), (Without<Dead>, Without<Anomaly>)>,
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
