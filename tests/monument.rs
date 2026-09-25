//! Every site's monument stands: each member building's ground floor
//! walls are solid where modforge's floor plan puts them.

mod common;

use avian2d::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use common::game;
use modforge::structure::{floor_plan, parts_of};
use topside::structure::{MonumentsRes, StructureRoot};

#[test]
fn every_building_stands_and_its_walls_are_solid() {
    let (mut app, _) = game();
    // One more step so the physics world has every collider.
    app.update();
    let world = app.world_mut();
    let monuments: Vec<(Vec2, Vec<(Vec2, Vec2)>)> = world
        .resource::<MonumentsRes>()
        .0
        .iter()
        .map(|(at, m)| {
            let walls = m
                .members
                .iter()
                .flat_map(|member| {
                    let offset = Vec2::new(member.offset.x, member.offset.z);
                    floor_plan(&parts_of(&member.structure), 0.0)
                        .into_iter()
                        .filter(|p| p.solid)
                        .map(move |p| (offset + p.min, offset + p.max))
                        .collect::<Vec<_>>()
                })
                .collect();
            (*at, walls)
        })
        .collect();
    let members: usize = world
        .resource::<MonumentsRes>()
        .0
        .iter()
        .map(|(_, m)| m.members.len())
        .sum();
    let roots = world.query_filtered::<(), With<StructureRoot>>().iter(world).count();
    assert_eq!(roots, members + 1, "every member building and the bunker");

    let (at, walls) = monuments
        .iter()
        .find(|(_, walls)| !walls.is_empty())
        .expect("some monument has buildings");
    let points: Vec<Vec2> = walls.iter().map(|(min, max)| *at + (*min + *max) / 2.0).collect();
    let hits = world
        .run_system_once(move |query: SpatialQuery| {
            points
                .iter()
                .filter(|p| !query.point_intersections(**p, &SpatialQueryFilter::default()).is_empty())
                .count()
        })
        .unwrap();
    assert_eq!(hits, walls.len(), "every solid part of the first building stops a body");
}
