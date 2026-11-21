//! Walls on the ground's grid, and doors (docs/pathing.md). Ways are
//! found and walked on the navigation mesh (navmesh.rs); this keeps the
//! ground's walk grid (`walk::WalkMap`, painted from every static
//! collider) only as the wall map each person's map and the minimap read
//! (sight.rs, minimap.rs), and every door: where it is, opening a closed
//! one ahead, and going through a doorway into memory.

use avian3d::prelude::*;
use bevy::prelude::*;
use modforge::walk::{CHUNK, ChunkKey, Rect, WalkMap, cell_of};

use crate::player::BODY_HEIGHT;
use crate::{Ground, SimTick};
use crate::actor::Dead;
use crate::interact::Door;

/// The one walk grid of the world: the ground's walls, for the maps.
#[derive(Resource, Default)]
pub struct Walk(pub WalkMap);

/// The rectangles a collider stands on, in world metres: a box, or a
/// compound of boxes (a building, a chunk of water and trees), each by
/// its footprint on the ground. The walk grid is the ground's: a part of a
/// compound counts only where it stands in a person's way on the ground,
/// not a floor above (a storey's slab, its walls) and not a slab lying on
/// the ground.
fn rects_of(collider: &Collider, at: Vec2) -> Vec<Rect> {
    let footprint = |half: Vec3| Vec2::new(half.x, half.z) * 2.0;
    let in_the_way = |centre: f32, half: f32| centre - half < BODY_HEIGHT && centre + half > 0.3;
    let shape = collider.shape_scaled();
    if let Some(cuboid) = shape.as_cuboid() {
        return vec![Rect::around(at, footprint(cuboid.half_extents))];
    }
    shape
        .as_compound()
        .map(|compound| {
            compound
                .shapes()
                .iter()
                .filter(|(pose, part)| part.as_cuboid().is_some_and(|c| in_the_way(pose.translation.y, c.half_extents.y)))
                .filter_map(|(pose, part)| {
                    part.as_cuboid()
                        .map(|c| Rect::around(at + pose.translation.ground(), footprint(c.half_extents)))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Every static collider that appears goes onto the walk grid; every one
/// that goes comes off it. Doors and sensors are no walls.
pub fn record_solids(
    mut walk: ResMut<Walk>,
    added: Query<(Entity, &Collider, &RigidBody, &Transform), (Added<Collider>, Without<Sensor>, Without<Door>)>,
    mut gone: RemovedComponents<Collider>,
) {
    for entity in gone.read() {
        walk.0.remove(entity.to_bits());
    }
    for (entity, collider, body, transform) in &added {
        if *body == RigidBody::Static {
            walk.0.add(entity.to_bits(), rects_of(collider, transform.translation.ground()));
        }
    }
}

/// Every door, filed by the chunk it stands in (doors do not move), so a
/// walker looks only at the doors around it, never at every door in the
/// world.
#[derive(Resource, Default)]
pub struct DoorIndex {
    by_chunk: std::collections::HashMap<ChunkKey, Vec<Entity>>,
    chunk_of: std::collections::HashMap<Entity, ChunkKey>,
}

fn chunk_at(p: Vec2) -> ChunkKey {
    let c = cell_of(p);
    (c.0.div_euclid(CHUNK), c.1.div_euclid(CHUNK))
}

/// Doors that appear are filed; doors that go are dropped.
pub fn record_doors(
    mut index: ResMut<DoorIndex>,
    added: Query<(Entity, &Door, &Transform), Added<Door>>,
    mut gone: RemovedComponents<Door>,
) {
    for entity in gone.read() {
        if let Some(key) = index.chunk_of.remove(&entity)
            && let Some(doors) = index.by_chunk.get_mut(&key)
        {
            doors.retain(|e| *e != entity);
        }
    }
    // Every door, on every level (everything is 3D: a door's height is its
    // own, read where it is used).
    for (entity, door, _) in added.iter() {
        let key = chunk_at(door.closed_center);
        index.by_chunk.entry(key).or_default().push(entity);
        index.chunk_of.insert(entity, key);
    }
}

/// How near a door, on the walker's own side of it, the place they go to
/// can be and they have come to the door, not through it (to knock on it),
/// metres.
const COME_TO_A_DOOR: f32 = 3.0;

/// A closed door to go through: within reach ahead of someone whose feet
/// are at `feet`, stepping along `way` on the ground, on their level, unless
/// where they go (`to`) is near it on their own side, on its level. Only the doors in the
/// chunks around them are looked at.
pub fn closed_door_ahead(index: &DoorIndex, doors: &Query<(&Door, &Transform)>, feet: Vec3, way: Vec2, to: Vec3) -> bool {
    let (cx, cy) = chunk_at(feet.ground());
    (cy - 1..=cy + 1)
        .flat_map(|y| (cx - 1..=cx + 1).map(move |x| (x, y)))
        .filter_map(|key| index.by_chunk.get(&key))
        .flatten()
        .filter_map(|e| doors.get(*e).ok())
        .any(|(d, t)| {
            let ahead = d.closed_center - feet.ground();
            let come_to = (to.y - t.translation.y).abs() < 1.5
                && d.side_of(to, t.translation.y) == d.side_of(feet, t.translation.y)
                && d.closed_center.distance(to.ground()) <= COME_TO_A_DOOR;
            !d.open && !come_to && crate::interact::in_reach(feet, crate::on_ground(d.closed_center, t.translation.y)) && ahead.dot(way) > 0.0
        })
}

/// How near a door's closed place someone stands to be in its doorway.
pub const IN_DOORWAY: f32 = 0.6;

/// The door whose doorway someone with feet at `feet` is within `within`
/// of, on their level, if any.
pub fn doorway_at(index: &DoorIndex, doors: &Query<(&Door, &Transform)>, feet: Vec3, within: f32) -> Option<Entity> {
    let (cx, cy) = chunk_at(feet.ground());
    (cy - 1..=cy + 1)
        .flat_map(|y| (cx - 1..=cx + 1).map(move |x| (x, y)))
        .filter_map(|key| index.by_chunk.get(&key))
        .flatten()
        .copied()
        .find(|e| doors.get(*e).is_ok_and(|(d, t)| d.closed_center.distance(feet.ground()) <= within && (t.translation.y - feet.y).abs() < 1.0))
}

/// Going through a doorway goes into the person's memory (life.md "What
/// a person did"), once each time they step into it, player and NPC
/// alike.
pub fn doorways(
    tick: Res<SimTick>,
    index: Res<DoorIndex>,
    doors: Query<(&Door, &Transform)>,
    mut people: Query<(Entity, &Transform, &mut crate::actor::Persona), Without<Dead>>,
    mut inside: Local<std::collections::HashMap<Entity, Entity>>,
) {
    for (person, at, mut memory) in &mut people {
        match doorway_at(&index, &doors, crate::interact::feet_of(at), IN_DOORWAY) {
            Some(door) => {
                if inside.insert(person, door) != Some(door) {
                    memory.0.memory.did(modforge::memory::Did::Doorway, tick.0);
                }
            }
            None => {
                inside.remove(&person);
            }
        }
    }
}
