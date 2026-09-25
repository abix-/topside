//! Walking around things (life.md "Movement": the brain says "go
//! there"). The brain's walk arrives as an aim at the target and a move;
//! when the straight way there is blocked by something solid, the way
//! round is planned with modforge's one grid path search on a one metre
//! grid, and the person steers to the next point of it. People and open
//! doors never block a way; walls, closed doors, water, and trees do.

use std::collections::VecDeque;

use avian2d::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use modforge::brain::REACH;
use modforge::path::{Cell, grid_path};

use crate::SimTick;
use crate::actor::{Dead, Npc};
use crate::input::{Action, ActionQueue, ThisTick};
use crate::player::PLAYER_SIZE;

/// Cells expanded before a plan gives up (a 60 m by 60 m search).
const PLAN_LIMIT: usize = 3600;
/// A plan older than this is made again, in ticks.
const REPLAN_EVERY: u64 = 120;
/// A moving goal is planned for at most this often, in ticks.
const REPLAN_MIN: u64 = 30;
/// A waypoint this near is reached.
const WAYPOINT_REACH: f32 = 0.6;

/// The way round a person is following.
#[derive(Component, Default)]
pub struct NavPath {
    pub waypoints: VecDeque<Vec2>,
    pub goal: Vec2,
    /// The tick of the last plan; None before the first.
    pub planned_at: Option<u64>,
}

/// What stands in the way: only static bodies that are not sensors.
#[derive(SystemParam)]
pub struct Nav<'w, 's> {
    spatial: SpatialQuery<'w, 's>,
    bodies: Query<'w, 's, &'static RigidBody>,
    sensors: Query<'w, 's, (), With<Sensor>>,
}

impl Nav<'_, '_> {
    fn solid(&self, e: Entity) -> bool {
        !self.sensors.contains(e) && self.bodies.get(e).is_ok_and(|b| *b == RigidBody::Static)
    }

    /// A body can walk straight from `from` to `to`.
    pub fn clear(&self, who: Entity, from: Vec2, to: Vec2) -> bool {
        let Ok((dir, distance)) = Dir2::new_and_length(to - from) else {
            return true;
        };
        let shape = Collider::circle(PLAYER_SIZE / 2.0);
        let filter = SpatialQueryFilter::from_excluded_entities([who]);
        self.spatial
            .cast_shape_predicate(
                &shape,
                from,
                0.0,
                dir,
                &ShapeCastConfig::from_max_distance(distance),
                &filter,
                &|e| self.solid(e),
            )
            .is_none()
    }

    fn blocked(&self, cell: Cell) -> bool {
        let centre = centre(cell);
        self.spatial
            .point_intersections(centre, &SpatialQueryFilter::default())
            .into_iter()
            .any(|e| self.solid(e))
    }

    /// The way round from `from` to `to` as points to walk through,
    /// ending at `to`; None when there is none nearby. Each cell asks
    /// the physics world once per plan.
    pub fn plan(&self, from: Vec2, to: Vec2) -> Option<VecDeque<Vec2>> {
        let (start, goal) = (cell(from), cell(to));
        let known: std::cell::RefCell<std::collections::HashMap<Cell, bool>> = Default::default();
        let blocked = |c: Cell| *known.borrow_mut().entry(c).or_insert_with(|| self.blocked(c));
        let step = |a: Cell, b: Cell| {
            (b == goal || !blocked(b)).then(|| if a.0 != b.0 && a.1 != b.1 { 1.414 } else { 1.0 })
        };
        let estimate = |c: Cell| centre(c).distance(centre(goal));
        let cells = grid_path(start, goal, step, estimate, PLAN_LIMIT)?;
        let mut points: VecDeque<Vec2> = cells.into_iter().skip(1).map(centre).collect();
        points.pop_back();
        points.push_back(to);
        Some(points)
    }
}

fn cell(p: Vec2) -> Cell {
    (p.x.floor() as i32, p.y.floor() as i32)
}

fn centre(c: Cell) -> Vec2 {
    Vec2::new(c.0 as f32 + 0.5, c.1 as f32 + 0.5)
}

/// Every NPC the brain sent walking this tick: straight when the way is
/// clear, else along the way round, re-planned when it is old or the
/// goal moved.
pub fn steer(
    tick: Res<SimTick>,
    nav: Nav,
    mut walkers: Query<(Entity, &Transform, &mut ThisTick, &mut NavPath), (With<Npc>, Without<Dead>)>,
) {
    for (entity, transform, mut this_tick, mut path) in &mut walkers {
        let (x, y) = ActionQueue::movement(&this_tick.0);
        let Some((tx, ty)) = ActionQueue::aim(&this_tick.0) else {
            continue;
        };
        if x == 0.0 && y == 0.0 {
            continue;
        }
        let here = transform.translation.truncate();
        let target = Vec2::new(tx, ty);
        // Arrived: stop here, not at the next decision.
        if here.distance(target) <= REACH {
            path.waypoints.clear();
            set_move(&mut this_tick, Vec2::ZERO, tick.0);
            continue;
        }
        if nav.clear(entity, here, target) {
            path.waypoints.clear();
            continue;
        }
        // Plan again only when the plan is old, or the goal moved and the
        // last plan is not brand new; a plan that failed or ran out is
        // not retried every tick.
        let age = path.planned_at.map_or(u64::MAX, |at| tick.0.saturating_sub(at));
        let moved = path.goal.distance(target) > 2.0 && age >= REPLAN_MIN;
        if age >= REPLAN_EVERY || moved {
            path.waypoints = nav.plan(here, target).unwrap_or_default();
            path.goal = target;
            path.planned_at = Some(tick.0);
        }
        while path.waypoints.front().is_some_and(|p| p.distance(here) <= WAYPOINT_REACH) {
            path.waypoints.pop_front();
        }
        let Some(&next) = path.waypoints.front() else {
            continue;
        };
        // Steer the step to the next point of the way round; the aim stays
        // on the goal, so it holds between decisions.
        set_move(&mut this_tick, (next - here).normalize_or_zero(), tick.0);
    }
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
