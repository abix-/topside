//! Every site's monument stands: each member building's ground floor
//! walls are solid where modforge's tile plan puts them.

mod common;

use avian2d::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use common::game;
use modforge::structure::tile_plan;
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
    // Each solid run of every building's tile plan, where the building
    // snapped to the tiles.
    let monuments: Vec<Vec<(Vec2, Vec2)>> = world
        .resource::<MonumentsRes>()
        .0
        .iter()
        .map(|(at, m)| {
            m.members
                .iter()
                .flat_map(|member| {
                    let placed = (*at + Vec2::new(member.offset.x, member.offset.z)).round();
                    tile_plan(&member.structure)
                        .into_iter()
                        .filter(|r| r.kind.solid())
                        .map(move |r| {
                            (placed + Vec2::new(r.from as f32, r.row as f32), placed + Vec2::new(r.to as f32, r.row as f32 + 1.0))
                        })
                        .collect::<Vec<_>>()
                })
                .collect()
        })
        .collect();
    let members: usize = world
        .resource::<MonumentsRes>()
        .0
        .iter()
        .map(|(_, m)| m.members.len())
        .sum();
    let roots = world.query_filtered::<(), With<StructureRoot>>().iter(world).count();
    assert_eq!(
        roots,
        members + 1 + topside::content::OTHER_BUNKERS.len(),
        "every member building, the bunker, and the other bunkers"
    );

    let walls = monuments.iter().find(|walls| !walls.is_empty()).expect("some monument has buildings");
    let points: Vec<Vec2> = walls.iter().map(|(min, max)| (*min + *max) / 2.0).collect();
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
