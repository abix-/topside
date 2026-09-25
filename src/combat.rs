//! Health, protection, and the one way a hit lands on an actor
//! (combat.md "How a hit is resolved"). modforge decides every hit
//! (`resolve_hit`) and what armor a person has (`Protection::worn`,
//! one total, no body parts); topside only finds who was hit and
//! carries out death.

use bevy::prelude::*;
use modforge::combat::{Health as HealthStats, Hit, HitResult, Protection as ProtectionStats, resolve_hit};
use modforge::item::{self, ItemRegistry};

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

/// Every actor at zero health dies: its body leaves the world.
pub fn die(mut commands: Commands, actors: Query<(Entity, &Health)>) {
    for (entity, health) in &actors {
        if health.0.current <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}
