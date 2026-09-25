//! Walking around things (docs/pathing.md, the one authority). The
//! brain's walk arrives as an aim at the target and a move; this finds
//! the way there on modforge's walk grid (`walk::WalkMap`), painted
//! from every static collider in the world as plain data, and steers
//! each person to the next point of it. Endless's design: a straight
//! line on the grid needs no search; searches wait in one queue and
//! each step works through at most `PLANS_PER_STEP` of them and
//! `PLAN_BUDGET_MS` of time; a way that cannot be found waits
//! `RETRY_AFTER` before it is asked again. People and doors never block
//! a way (a person opens a closed door with the player's own Use);
//! walls, water, trees, and boxes do.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use avian2d::prelude::*;
use bevy::prelude::*;
use modforge::brain::REACH;
use modforge::walk::{CHUNK, ChunkKey, Rect, WalkMap, cell_of};

use crate::SimTick;
use crate::actor::{Dead, Npc};
use crate::input::{Action, ActionQueue, ThisTick};
use crate::interact::Door;

/// Chunk doorways searched before a way counts as not found
/// (pathing.md "Search by chunk, then across chunks"): a 150 m trip
/// crosses about five chunks with a few doorways each.
const PLAN_LIMIT: usize = 2_000;
/// Searches worked through each step at most (Endless: 200 a frame).
const PLANS_PER_STEP: usize = 50;
/// Time spent searching each step at most (Endless: 2 ms a frame).
const PLAN_BUDGET_MS: u64 = 2;
/// A way older than this is found again, in ticks: the world between
/// changes (doors, boxes, the storm).
const REPLAN_EVERY: u64 = 300;
/// A moving goal is planned for at most this often, in ticks.
const REPLAN_MIN: u64 = 30;
/// A way not found is asked for again after this, in ticks (Endless: 2 s).
const RETRY_AFTER: u64 = 120;
/// A waypoint this near is reached.
const WAYPOINT_REACH: f32 = 0.6;

/// The one walk grid of the world.
#[derive(Resource, Default)]
pub struct Walk(pub WalkMap);

/// The people waiting for a way to be found, first come first served.
#[derive(Resource, Default)]
pub struct PlanQueue(pub VecDeque<Entity>);

/// One search that found no way, for the control plane: who, what the
/// brain had them doing, from where to where, whether the tiles at
/// each end are blocked, and how many tiles the search looked at.
#[derive(Clone, Debug)]
pub struct FailedWay {
    pub tick: u64,
    pub who: String,
    pub doing: String,
    pub from: Vec2,
    pub to: Vec2,
    pub from_blocked: bool,
    pub to_blocked: bool,
    pub searched: usize,
}

/// The last failed searches, newest last.
#[derive(Resource, Default)]
pub struct FailedWays(pub VecDeque<FailedWay>);

/// How many failed searches are kept for the control plane.
const FAILED_KEPT: usize = 30;

/// The way a person is following.
#[derive(Component, Default)]
pub struct NavPath {
    pub waypoints: VecDeque<Vec2>,
    /// The goal the way leads to.
    pub goal: Vec2,
    /// The tick the way was found; None before the first.
    pub planned_at: Option<u64>,
    /// Waiting in the queue.
    pub queued: bool,
    /// No way was found: not asked again before this tick.
    pub retry_at: u64,
}

/// The rectangles a collider stands on, in world metres: a box, or a
/// compound of boxes (a building, a chunk of water and trees).
fn rects_of(collider: &Collider, at: Vec2) -> Vec<Rect> {
    let shape = collider.shape_scaled();
    if let Some(cuboid) = shape.as_cuboid() {
        return vec![Rect::around(at, cuboid.half_extents * 2.0)];
    }
    shape
        .as_compound()
        .map(|compound| {
            compound
                .shapes()
                .iter()
                .filter_map(|(pose, part)| {
                    part.as_cuboid()
                        .map(|c| Rect::around(at + pose.translation, c.half_extents * 2.0))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Every static collider that appears goes onto the walk grid; every one
/// that goes comes off it. Doors and sensors never block a way.
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
            walk.0.add(entity.to_bits(), rects_of(collider, transform.translation.truncate()));
        }
    }
}

/// Every door, filed by the walk grid chunk it stands in (doors do not
/// move), so a walker looks only at the doors around it, never at every
/// door in the world.
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
    added: Query<(Entity, &Door), Added<Door>>,
    mut gone: RemovedComponents<Door>,
) {
    for entity in gone.read() {
        if let Some(key) = index.chunk_of.remove(&entity)
            && let Some(doors) = index.by_chunk.get_mut(&key)
        {
            doors.retain(|e| *e != entity);
        }
    }
    for (entity, door) in &added {
        let key = chunk_at(door.closed_center);
        index.by_chunk.entry(key).or_default().push(entity);
        index.chunk_of.insert(entity, key);
    }
}

/// A closed door within reach ahead of someone at `here` stepping along
/// `way`: only the doors in the chunks around them are looked at.
fn closed_door_ahead(index: &DoorIndex, doors: &Query<&Door>, here: Vec2, way: Vec2) -> bool {
    let (cx, cy) = chunk_at(here);
    (cy - 1..=cy + 1)
        .flat_map(|y| (cx - 1..=cx + 1).map(move |x| (x, y)))
        .filter_map(|key| index.by_chunk.get(&key))
        .flatten()
        .filter_map(|e| doors.get(*e).ok())
        .any(|d| {
            let to = d.closed_center - here;
            !d.open && to.length() <= crate::interact::REACH && to.dot(way) > 0.0
        })
}

/// Every NPC the brain sent walking this tick: a way asked for when
/// there is none, it is old, or the goal moved; the queue worked through
/// within the budget; then each steps to the next point of its way.
pub fn steer(
    tick: Res<SimTick>,
    mut walk: ResMut<Walk>,
    mut queue: ResMut<PlanQueue>,
    door_index: Res<DoorIndex>,
    doors: Query<&Door>,
    mut laps: ResMut<crate::profile::Laps>,
    mut failed: ResMut<FailedWays>,
    people: Query<(&Npc, &crate::actor::ActorActivity)>,
    mut walkers: Query<(Entity, &Transform, &mut ThisTick, &mut NavPath), (With<Npc>, Without<Dead>)>,
) {
    let ms = |since: Instant| since.elapsed().as_secs_f64() * 1000.0;
    // Who needs a way.
    let started = Instant::now();
    let mut walking = 0;
    for (entity, transform, this_tick, mut path) in &mut walkers {
        let Some(target) = walking_to(&this_tick) else {
            continue;
        };
        walking += 1;
        if path.queued || tick.0 < path.retry_at || transform.translation.truncate().distance(target) <= REACH {
            continue;
        }
        let age = path.planned_at.map_or(u64::MAX, |at| tick.0.saturating_sub(at));
        let moved = path.goal.distance(target) > 2.0 && age >= REPLAN_MIN;
        if age >= REPLAN_EVERY || moved {
            path.queued = true;
            queue.0.push_back(entity);
        }
    }
    laps.record("steer: who needs a way", ms(started));
    laps.count("people walking", walking);
    // Work through the queue within the budget; the rest wait.
    let started = Instant::now();
    let (mut done, mut straight, mut found, mut not_found) = (0, 0, 0, 0);
    while done < PLANS_PER_STEP && started.elapsed() < Duration::from_millis(PLAN_BUDGET_MS) {
        let Some(entity) = queue.0.pop_front() else {
            break;
        };
        let Ok((_, transform, this_tick, mut path)) = walkers.get_mut(entity) else {
            continue;
        };
        path.queued = false;
        let Some(target) = walking_to(&this_tick) else {
            continue;
        };
        done += 1;
        let here = transform.translation.truncate();
        path.goal = target;
        path.planned_at = Some(tick.0);
        if walk.0.line_clear(here, target) {
            path.waypoints.clear();
            straight += 1;
            continue;
        }
        match walk.0.find_way(here, target, PLAN_LIMIT) {
            Some(points) => {
                path.waypoints = points.into();
                found += 1;
            }
            None => {
                path.waypoints.clear();
                path.retry_at = tick.0 + RETRY_AFTER;
                not_found += 1;
                let searched = walk.0.last_searched;
                let (who, doing) = people
                    .get(entity)
                    .map(|(npc, activity)| (format!("{} ({:?})", npc.def, npc.faction), format!("{:?}", activity.0)))
                    .unwrap_or_default();
                failed.0.push_back(FailedWay {
                    tick: tick.0,
                    who,
                    doing,
                    from: here,
                    to: target,
                    from_blocked: walk.0.blocked(modforge::walk::cell_of(here)),
                    to_blocked: walk.0.blocked(modforge::walk::cell_of(target)),
                    searched,
                });
                if failed.0.len() > FAILED_KEPT {
                    failed.0.pop_front();
                }
            }
        }
    }
    laps.record("steer: finding ways", ms(started));
    laps.count("ways: straight line", straight);
    laps.count("ways: searched and found", found);
    laps.count("ways: searched, not found", not_found);
    laps.count("ways: waiting in the queue", queue.0.len());
    laps.count("walk grid: chunks painted", walk.0.painted_chunks());
    // Step along the way.
    let started = Instant::now();
    for (_, transform, mut this_tick, mut path) in &mut walkers {
        let Some(target) = walking_to(&this_tick) else {
            continue;
        };
        let here = transform.translation.truncate();
        // Arrived: stop here, not at the next decision.
        if here.distance(target) <= REACH {
            path.waypoints.clear();
            set_move(&mut this_tick, Vec2::ZERO, tick.0);
            continue;
        }
        while path.waypoints.front().is_some_and(|p| p.distance(here) <= WAYPOINT_REACH) {
            path.waypoints.pop_front();
        }
        let next = path.waypoints.front().copied().unwrap_or(target);
        // A closed door in the way: open it, with the player's own Use.
        // The aim stays on where they are going, and the door ahead is
        // the thing nearest it.
        if closed_door_ahead(&door_index, &doors, here, next - here) && !ActionQueue::has(&this_tick.0, Action::Use) {
            this_tick.0.push(modforge::actions::Stamped {
                tick: tick.0,
                action: Action::Use,
            });
        }
        // Steer the step to the next point of the way; the aim stays on
        // the goal, so it holds between decisions.
        if !path.waypoints.is_empty() {
            set_move(&mut this_tick, (next - here).normalize_or_zero(), tick.0);
        }
    }
    laps.record("steer: stepping along", ms(started));
}

/// Where the brain has this person walking this tick, if anywhere.
fn walking_to(this_tick: &ThisTick) -> Option<Vec2> {
    let (x, y) = ActionQueue::movement(&this_tick.0);
    let (tx, ty) = ActionQueue::aim(&this_tick.0)?;
    (x != 0.0 || y != 0.0).then_some(Vec2::new(tx, ty))
}

/// Replace this tick's move with a step along `way`.
fn set_move(this_tick: &mut ThisTick, way: Vec2, now: u64) {
    let stamp = this_tick.0.last().map_or(now, |s| s.tick);
    this_tick.0.retain(|s| !matches!(s.action, Action::Move { .. }));
    this_tick.0.push(modforge::actions::Stamped {
        tick: stamp,
        action: Action::Move { x: way.x, y: way.y },
    });
}
