//! The one actor registry as an app resource (authority.md ActorMap):
//! modforge owns the rules (ids never reused, rows reused LIFO, two
//! counts never mixed); this file binds it to Bevy entities. Only the
//! map and the future GPU uploader ever see a row number.

use bevy::prelude::*;
use modforge::actor::ActorMap;
pub use modforge::actor::ActorId;

/// Bound for every row-indexed GPU buffer: the 1000 persistent
/// people plus horde walkers, animals, and headroom.
pub const MAX_ACTORS: usize = 8192;

#[derive(Resource)]
pub struct ActorMapRes(pub ActorMap<Entity>);

impl Default for ActorMapRes {
    fn default() -> Self {
        Self(ActorMap::new(MAX_ACTORS))
    }
}

/// The persistent identity on every actor entity.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Actor(pub ActorId);
