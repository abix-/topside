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
use crate::EYE_HEIGHT;

/// How far a person's eyes reach on their map, in tiles (an NPC's sight,
/// content's `ActorDef::sight`).
pub const MAP_SIGHT: i32 = 25;

/// Every fixed step, anyone standing on a tile they have not looked
/// around from adds to their map what their eyes reach from it (modforge
/// `Map::look`): what stops a body on the walk grid blocks the eyes,
/// except water, which is seen over.
pub fn look_around(
    tick: Res<crate::SimTick>,
    mut walk: ResMut<crate::nav::Walk>,
    chunks: Option<Res<crate::terrain::Chunks>>,
    mut laps: ResMut<crate::profile::Laps>,
    mut people: Query<(&Transform, &mut crate::actor::Map), Without<crate::actor::Dead>>,
) {
    let Some(chunks) = chunks else {
        return;
    };
    let mut looked = 0;
    for (at, mut map) in &mut people {
        let tile = modforge::walk::cell_of(crate::Ground::ground(at.translation));
        if map.looked_from == Some(tile) {
            continue;
        }
        map.looked_from = Some(tile);
        looked += 1;
        let mut opaque =
            |c: (i32, i32)| walk.0.blocked(c) && chunks.ground_at(c) != Some(modforge::ground::Ground::Water);
        map.map.look(tile, MAP_SIGHT, &mut opaque, tick.0);
    }
    laps.count("people looking around", looked);
}

/// Each bunker's map: what its people have told it (life.md "The map each
/// person carries": what their bunker tells them).
#[derive(Resource, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct BunkerMaps(pub std::collections::HashMap<String, modforge::map::Map>);

/// Once a second, every bunker person at home tells their bunker what they
/// saw and learns what the others did: their map joined into their
/// bunker's, and their bunker's into theirs.
pub fn share_maps(
    tick: Res<crate::SimTick>,
    mut bunkers: ResMut<BunkerMaps>,
    mut people: Query<
        (&Transform, &crate::actor::Npc, &crate::actor::Home, &mut crate::actor::Map),
        Without<crate::actor::Dead>,
    >,
) {
    if tick.0 % 60 != 0 {
        return;
    }
    for (at, npc, home, mut map) in &mut people {
        let Some(bunker) = npc.faction.as_ref().filter(|f| f.starts_with("bunker")) else {
            continue;
        };
        if home.0.distance(crate::interact::feet_of(at)) > crate::ai::HOME_REACH {
            continue;
        }
        let shared = bunkers.0.entry(bunker.clone()).or_default();
        shared.join(&map.map);
        let before = map.map.tiles_seen();
        map.map.join(shared);
        map.told += map.map.tiles_seen() - before;
    }
}

/// The one line of sight check.
#[derive(SystemParam)]
pub struct Sight<'w, 's> {
    spatial: SpatialQuery<'w, 's>,
    blockers: Query<'w, 's, (), Or<(With<StructureRoot>, With<Door>, With<crate::terrain::BlocksSight>)>>,
}

impl Sight<'_, '_> {
    /// Nothing that blocks the view stands between the eyes of someone
    /// whose feet are at `from` and eye height over `to` (where someone's
    /// feet are, or a thing rests), heights included: a floor between them
    /// blocks it as a wall does.
    pub fn clear(&self, from: Vec3, to: Vec3) -> bool {
        let (from, to) = (from + Vec3::Y * EYE_HEIGHT, to + Vec3::Y * EYE_HEIGHT);
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
