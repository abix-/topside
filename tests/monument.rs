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
fn every_loot_spot_has_a_box_with_something_in_it() {
    let (mut app, _) = game();
    let spots: usize = app
        .world()
        .resource::<MonumentsRes>()
        .0
        .iter()
        .map(|(_, m)| m.loot_spots.len())
        .sum();
    assert!(spots > 0, "the world has loot spots");
    let world = app.world_mut();
    let boxes: Vec<usize> = world
        .query_filtered::<&topside::interact::Container, With<topside::terrain::WorldPiece>>()
        .iter(world)
        .map(|c| c.0.slots.iter().flatten().count())
        .collect();
    assert_eq!(boxes.len(), spots, "one box per spot");
    assert!(boxes.iter().all(|&stacks| stacks > 0), "every box holds something");
}

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
