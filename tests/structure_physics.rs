use std::time::Duration;

use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*, time::TimeUpdateStrategy};
use topside::{
    AppState, build_app,
    cel::CelMaterial,
    player::{MovementInput, Player},
    structure::{
        CONCRETE_FLOOR, CONCRETE_WALL, Door, Opening, RoomSpec, Side, StairSpec, StructureDef,
        StructureRoot, StructureSolid, spawn_structure,
    },
};

type SpawnStructureParams<'w, 's> = (
    Commands<'w, 's>,
    ResMut<'w, Assets<Mesh>>,
    ResMut<'w, Assets<CelMaterial>>,
);

const EYE_HEIGHT: f32 = 1.7;

fn one_room() -> StructureDef {
    StructureDef {
        name: "one room".to_string(),
        wall_color: CONCRETE_WALL,
        floor_color: CONCRETE_FLOOR,
        rooms: vec![RoomSpec {
            origin: Vec3::ZERO,
            interior: Vec3::new(6.0, 3.0, 8.0),
            wall_thickness: 0.2,
            openings: vec![Opening {
                side: Side::North,
                offset: 0.0,
                width: 1.2,
                sill: 0.0,
                door: true,
            }],
            floor: true,
            ceiling: true,
        }],
        stairs: vec![],
        furniture: vec![],
        lights: vec![],
    }
}

fn stairs() -> StructureDef {
    StructureDef {
        name: "stairs".to_string(),
        wall_color: CONCRETE_WALL,
        floor_color: CONCRETE_FLOOR,
        rooms: vec![],
        stairs: vec![StairSpec {
            base: Vec3::ZERO,
            side: Side::North,
            width: 2.0,
            rise: 3.1,
            landing: 0.5,
        }],
        furniture: vec![],
        lights: vec![],
    }
}

fn spawn(app: &mut App, def: &StructureDef, origin: Vec3) -> Entity {
    let world = app.world_mut();
    let mut state: SystemState<SpawnStructureParams<'_, '_>> = SystemState::new(world);
    let root;
    {
        let (mut commands, mut meshes, mut materials) = state
            .get_mut(world)
            .expect("structure spawn system state is valid");
        root = spawn_structure(&mut commands, &mut meshes, &mut materials, def, origin)
            .expect("structure def is valid");
    }
    state.apply(world);
    root
}

/// The production app, headless, with the structure, a ground
/// plane, and a player standing at `feet` looking along `yaw`.
fn walking_app(def: &StructureDef, feet: Vec3, yaw: f32) -> (App, Entity) {
    let mut app = build_app(true);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / 60.0,
    )));
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<CelMaterial>>();
    app.world_mut()
        .spawn((RigidBody::Static, Collider::half_space(Vec3::Y)));
    spawn(&mut app, def, Vec3::ZERO);
    let player = app
        .world_mut()
        .spawn((
            Transform::from_translation(feet + Vec3::Y * EYE_HEIGHT),
            Player { yaw, pitch: 0.0 },
        ))
        .id();
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    // Let the state change apply and Avian index the colliders.
    app.update();
    app.update();
    (app, player)
}

fn walk(app: &mut App, input: Vec2, ticks: usize) {
    for _ in 0..ticks {
        app.world_mut().resource_mut::<MovementInput>().0 = input;
        app.update();
    }
}

fn feet(app: &App, player: Entity) -> Vec3 {
    app.world().get::<Transform>(player).unwrap().translation - Vec3::Y * EYE_HEIGHT
}

#[test]
fn structure_root_owns_avian_geometry_and_collision() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PhysicsPlugins::default());
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<CelMaterial>>();
    app.finish();
    app.cleanup();

    let root = spawn(&mut app, &one_room(), Vec3::new(10.0, 0.0, -4.0));
    app.update();

    assert_eq!(
        app.world().get::<StructureRoot>(root),
        Some(&StructureRoot)
    );
    assert_eq!(
        app.world().get::<RigidBody>(root),
        Some(&RigidBody::Static)
    );
    let owned_entities: Vec<Entity> = app
        .world()
        .get::<Children>(root)
        .expect("structure root owns spawned entities")
        .iter()
        .collect();

    let solid_entities: Vec<Entity> = owned_entities
        .iter()
        .copied()
        .filter(|entity| app.world().get::<StructureSolid>(*entity).is_some())
        .collect();
    assert!(!solid_entities.is_empty(), "structure must contain solids");
    for entity in solid_entities {
        assert!(app.world().get::<Mesh3d>(entity).is_some());
        assert!(
            app.world().get::<Collider>(entity).is_none(),
            "solids carry no collider of their own; the root's compound has their boxes"
        );
    }
    // The building is one static shape to the physics (performance.md).
    let compound = app
        .world()
        .get::<Collider>(root)
        .expect("the root carries the building's one collider");
    assert!(
        compound.shape().as_compound().is_some(),
        "the root collider is a compound of every solid"
    );
    assert_eq!(
        app.world()
            .get::<ColliderOf>(root)
            .expect("the root collider is attached to its own body")
            .body,
        root
    );

    let door = owned_entities
        .iter()
        .copied()
        .find(|entity| app.world().get::<Door>(*entity).is_some())
        .expect("fixture contains a door");
    assert_eq!(
        app.world().get::<RigidBody>(door),
        Some(&RigidBody::Kinematic)
    );
    assert!(app.world().get::<Collider>(door).is_some());

    app.world_mut().despawn(root);
    assert!(app.world().get_entity(root).is_err());
    for entity in owned_entities {
        assert!(
            app.world().get_entity(entity).is_err(),
            "owned entity {entity} survived its structure root"
        );
    }
}

#[test]
fn walls_block_and_the_doorway_does_not() {
    // Start inside the room, walk north (negative z) beside the
    // doorway: the wall at z -4.1 stops the body.
    let (mut app, player) = walking_app(&one_room(), Vec3::new(2.0, 0.0, 0.0), 0.0);
    walk(&mut app, Vec2::new(0.0, -1.0), 120);
    let stopped = feet(&app, player);
    assert!(
        stopped.z > -4.1 && stopped.z < -3.3,
        "the wall must stop the body just short of it, got {stopped}"
    );

    // Same walk through the doorway: the closed door itself stops
    // the body, not the wall, so it gets past the wall line only
    // once the door is open.
    let (mut app, player) = walking_app(&one_room(), Vec3::new(0.0, 0.0, 0.0), 0.0);
    for mut door in app
        .world_mut()
        .query::<&mut Door>()
        .iter_mut(app.world_mut())
    {
        door.open = true;
    }
    for (door, mut transform) in app
        .world_mut()
        .query::<(&Door, &mut Transform)>()
        .iter_mut(app.world_mut())
    {
        *transform = door.transform();
    }
    app.update();
    walk(&mut app, Vec2::new(0.0, -1.0), 120);
    let through = feet(&app, player);
    assert!(
        through.z < -5.0,
        "the body must pass through the open doorway, got {through}"
    );
}

#[test]
fn stairs_are_climbed_front_on() {
    // 50 ticks at 5 m/s is 4.17 m: from z 1.0 up the 3 m run onto
    // the landing (z -3.0 to -3.5). Further is walking off its far
    // edge, which is a fall, not a stairs defect.
    let (mut app, player) = walking_app(&stairs(), Vec3::new(0.0, 0.0, 1.0), 0.0);
    walk(&mut app, Vec2::new(0.0, -1.0), 50);
    let top = feet(&app, player);
    assert!(
        (top.y - 3.1).abs() < 0.05 && top.z < -3.0 && top.z > -3.5,
        "the body must reach the landing at the top of the stairs, got {top}"
    );
}

#[test]
fn stairs_block_from_the_side() {
    // Stand west of the run halfway up its length and walk east
    // into its side: the stair is solid, so the body stays off it.
    let (mut app, player) = walking_app(&stairs(), Vec3::new(-2.0, 0.0, -1.5), 0.0);
    walk(&mut app, Vec2::new(1.0, 0.0), 120);
    let side = feet(&app, player);
    assert!(
        side.x < -1.0 && side.y < 0.05,
        "the side of the stairs must block like a wall, got {side}"
    );
}
