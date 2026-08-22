//! The one path from a rolled world to the scene: chunk colliders,
//! flat ground under every monument, every monument at its site, a
//! walkable road.

use std::time::Duration;

use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*, time::TimeUpdateStrategy};
use modforge::worldgen::{Generator, roll_world};
use topside::{
    AppState, build_app,
    cel::CelMaterial,
    content::{BiomeRegistryRes, ItemRegistryRes, MonumentRegistryRes, topside_def},
    structure::{MonumentRoot, StructureRoot},
    terrain::{CHUNK, WorldPiece, spawn_world},
};

#[test]
fn a_rolled_world_stands_in_the_scene() {
    let mut app = build_app(true);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / 60.0,
    )));
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<CelMaterial>>();
    // A small world so the test is quick: the same def, a quarter the
    // side, fewer sites.
    let mut def = topside_def();
    def.size = 600.0;
    def.monuments = vec![("roadside stop".to_string(), 3), ("military outpost".to_string(), 1)];
    let world = {
        let w = app.world();
        roll_world(
            &def,
            11,
            &w.resource::<BiomeRegistryRes>().0,
            &w.resource::<MonumentRegistryRes>().monuments,
        )
        .unwrap()
    };
    let roots = {
        let mut state: SystemState<(
            Commands,
            ResMut<Assets<Mesh>>,
            ResMut<Assets<CelMaterial>>,
            Res<BiomeRegistryRes>,
            Res<MonumentRegistryRes>,
            Res<ItemRegistryRes>,
        )> = SystemState::new(app.world_mut());
        let (mut commands, mut meshes, mut materials, biomes, monuments, items) =
            state.get_mut(app.world_mut()).unwrap();
        let roots = spawn_world(
            &mut commands,
            &mut meshes,
            &mut materials,
            &world,
            &biomes.0,
            &monuments.monuments,
            &monuments.buildings,
            &items.0,
            0.0,
        )
        .unwrap();
        state.apply(app.world_mut());
        roots
    };
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();
    app.update();
    let w = app.world_mut();

    // One collider per chunk block.
    let chunks = w
        .query_filtered::<&Collider, (With<WorldPiece>, With<RigidBody>, With<Children>)>()
        .iter(w)
        .count();
    let per_side = world.cells.div_ceil(CHUNK);
    assert_eq!(chunks, per_side * per_side, "one collider per chunk");

    // Every rolled monument stands at its site, on the ground there.
    assert_eq!(roots.len(), world.sites.len());
    for (root, site) in roots.iter().zip(&world.sites) {
        assert!(w.get::<MonumentRoot>(*root).is_some());
        let origin = w.get::<MonumentRoot>(*root).unwrap().origin;
        assert_eq!(origin.x, site.position.x);
        assert_eq!(origin.z, site.position.y);
        assert!((origin.y - world.height_at(site.position)).abs() < 1e-3);
        // Flat under it: the same height a few metres out.
        let h = world.height_at(site.position);
        let out = world.height_at(site.position + Vec2::new(12.0, 0.0));
        assert!((h - out).abs() < 1e-3, "ground slopes under a monument");
    }

    // A ray down onto a road cell lands on the terrain collider at
    // the heightmap's height: the road is walkable ground.
    let road = &world.roads[0];
    let mid = road.points[road.points.len() / 2];
    let expected = world.height_at(mid);
    let mut spatial: SystemState<SpatialQuery> = SystemState::new(w);
    let spatial = spatial.get(w).unwrap();
    let hit = spatial
        .cast_ray(
            Vec3::new(mid.x, expected + 50.0, mid.y),
            Dir3::NEG_Y,
            200.0,
            true,
            &SpatialQueryFilter::default(),
        )
        .expect("the road has ground under it");
    let landed = expected + 50.0 - hit.distance;
    assert!((landed - expected).abs() < 1.0, "road ground at {landed}, heightmap {expected}");
}

/// The Labyrinth's walls are real structures: one root with a
/// collider per wall run, and a ray across a corridor stops at the
/// wall's face.
#[test]
fn the_labyrinth_walls_stand_as_structures() {
    let mut app = build_app(true);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / 60.0,
    )));
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<CelMaterial>>();
    let mut def = topside_def();
    def.size = 400.0;
    def.water_level = 0.0;
    def.generator = Generator::Labyrinth {
        corridor: 40.0,
        wall_height: 6.0,
        wall_thickness: 1.0,
    };
    def.monuments = vec![("wreck".to_string(), 6)];
    let world = {
        let w = app.world();
        roll_world(
            &def,
            5,
            &w.resource::<BiomeRegistryRes>().0,
            &w.resource::<MonumentRegistryRes>().monuments,
        )
        .unwrap()
    };
    assert!(world.walls.len() > 20);
    {
        let mut state: SystemState<(
            Commands,
            ResMut<Assets<Mesh>>,
            ResMut<Assets<CelMaterial>>,
            Res<BiomeRegistryRes>,
            Res<MonumentRegistryRes>,
            Res<ItemRegistryRes>,
        )> = SystemState::new(app.world_mut());
        let (mut commands, mut meshes, mut materials, biomes, monuments, items) =
            state.get_mut(app.world_mut()).unwrap();
        spawn_world(
            &mut commands,
            &mut meshes,
            &mut materials,
            &world,
            &biomes.0,
            &monuments.monuments,
            &monuments.buildings,
            &items.0,
            0.0,
        )
        .unwrap();
        state.apply(app.world_mut());
    }
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();
    app.update();
    let w = app.world_mut();

    // One structure root with a collider per wall run, beyond the
    // monument roots.
    let wall_roots = w
        .query_filtered::<&Collider, (With<StructureRoot>, With<WorldPiece>, Without<MonumentRoot>)>()
        .iter(w)
        .count();
    assert_eq!(wall_roots, world.walls.len(), "one structure per wall run");

    // Stand 5 m off the middle of a long wall and look at it: the ray
    // stops at its face, half a thickness before the centre line.
    let wall = world
        .walls
        .iter()
        .max_by(|a, b| a.start.distance(a.end).total_cmp(&b.start.distance(b.end)))
        .unwrap();
    let mid = (wall.start + wall.end) / 2.0;
    let across = if (wall.end - wall.start).x.abs() > (wall.end - wall.start).y.abs() {
        Vec3::Z
    } else {
        Vec3::X
    };
    let eye = Vec3::new(mid.x, 1.7, mid.y) - across * 5.0;
    let mut spatial: SystemState<SpatialQuery> = SystemState::new(w);
    let spatial = spatial.get(w).unwrap();
    let hit = spatial
        .cast_ray(eye, Dir3::new(across).unwrap(), 20.0, true, &SpatialQueryFilter::default())
        .expect("the wall is there");
    assert!(
        (hit.distance - (5.0 - wall.thickness / 2.0)).abs() < 0.05,
        "the wall's face is {} m away",
        hit.distance
    );
}
