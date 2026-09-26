//! Who can see what (design.md "What 2D changes": nobody sees behind
//! walls, the way Project Zomboid does it). One rule for everyone: a
//! line between two points is clear unless a building or a closed door
//! stands on it. Trees and water are low and do not block the view; an
//! open door has no collider.

use avian2d::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::actor::{Dead, Npc};
use crate::interact::{Door, WorldItem};
use crate::player::Player;
use crate::structure::StructureRoot;

/// Farther than this, nothing is drawn for the player anyway.
pub const VIEW_DISTANCE: f32 = 60.0;

/// The one line of sight check.
#[derive(SystemParam)]
pub struct Sight<'w, 's> {
    spatial: SpatialQuery<'w, 's>,
    blockers: Query<'w, 's, (), Or<(With<StructureRoot>, With<Door>, With<crate::terrain::BlocksSight>)>>,
}

impl Sight<'_, '_> {
    /// Nothing that blocks the view stands between `from` and `to`.
    pub fn clear(&self, from: Vec2, to: Vec2) -> bool {
        let Ok((dir, distance)) = Dir2::new_and_length(to - from) else {
            return true;
        };
        self.spatial
            .cast_ray_predicate(from, dir, distance, true, &SpatialQueryFilter::default(), &|e| {
                self.blockers.contains(e)
            })
            .is_none()
    }
}

/// The player sees only what their person can see: people, items, and
/// bullets behind a wall (or too far off) are hidden.
pub fn player_sight(
    sight: Sight,
    players: Query<&Transform, (With<Player>, Without<Dead>)>,
    mut seen: Query<
        (&Transform, &mut Visibility),
        (Or<(With<Npc>, With<WorldItem>, With<crate::combat::Bullet>)>, Without<Player>, Without<Dead>),
    >,
) {
    let Ok(player) = players.single() else {
        return;
    };
    let eye = player.translation.truncate();
    for (transform, mut visibility) in &mut seen {
        let at = transform.translation.truncate();
        let visible = at.distance(eye) <= VIEW_DISTANCE && sight.clear(eye, at);
        visibility.set_if_neq(if visible { Visibility::Inherited } else { Visibility::Hidden });
    }
}
