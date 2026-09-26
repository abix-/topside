//! Who can see what (design.md: nobody sees behind walls). One rule for
//! everyone: a line at eye height between two points is clear unless a
//! building, a closed door, or a cliff, tree, or rock stands on it. Water
//! and furniture are low and are seen over; an open door has no collider.
//! The player's own eyes are the first person camera: what a wall hides
//! is simply not drawn.

use avian3d::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::interact::Door;
use crate::structure::StructureRoot;
use crate::{EYE_HEIGHT, on_ground};

/// The one line of sight check.
#[derive(SystemParam)]
pub struct Sight<'w, 's> {
    spatial: SpatialQuery<'w, 's>,
    blockers: Query<'w, 's, (), Or<(With<StructureRoot>, With<Door>, With<crate::terrain::BlocksSight>)>>,
}

impl Sight<'_, '_> {
    /// Nothing that blocks the view stands between the eyes of someone at
    /// `from` and the eyes of someone at `to` (ground points).
    pub fn clear(&self, from: Vec2, to: Vec2) -> bool {
        let (from, to) = (on_ground(from, EYE_HEIGHT), on_ground(to, EYE_HEIGHT));
        let Ok((dir, distance)) = Dir3::new_and_length(to - from) else {
            return true;
        };
        self.spatial
            .cast_ray_predicate(from, dir, distance, true, &SpatialQueryFilter::default(), &|e| {
                self.blockers.contains(e)
            })
            .is_none()
    }
}
