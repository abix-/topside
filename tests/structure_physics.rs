use std::time::Duration;

use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*, time::TimeUpdateStrategy};
use topside::{
    AppState, SimTick,
    actor::{Facing, Hotbar},
    build_app,
    cel::CelMaterial,
    content::ItemRegistryRes,
    input::{Action, Actions},
    player::Player,
    structure::{
        CONCRETE_FLOOR, CONCRETE_WALL, Door, Opening, Pieces, Rebake, RoomSpec, Side, StairSpec,
        StructureBatch, StructureDef, StructureRoot, spawn_structure,
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
            Transform::from_translation(feet + Vec3::Y * EYE_HEIGHT)
                .with_rotation(Quat::from_euler(EulerRot::YXZ, yaw, 0.0, 0.0)),
            Player,
            Facing { yaw, pitch: 0.0 },
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

/// Push an action onto the player's queue for the next fixed tick,
/// exactly as the keyboard would.
fn act(app: &mut App, player: Entity, action: Action) {
    let next = app.world().resource::<SimTick>().0 + 1;
    app.world_mut()
        .get_mut::<Actions>(player)
        .unwrap()
        .0
        .push(next, action);
}

fn walk(app: &mut App, player: Entity, input: Vec2, ticks: usize) {
    for _ in 0..ticks {
        act(app, player, Action::Move { x: input.x, y: input.y });
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
        Some(&RigidBody::Static),
        "the root is the building's one static body"
    );
    let owned_entities: Vec<Entity> = app
        .world()
        .get::<Children>(root)
        .expect("structure root owns spawned entities")
        .iter()
        .collect();

    // Batched like Rust's buildings (performance.md): one compound
    // collider on the root holding every piece's box, one merged
    // mesh per material, no entity per piece.
    let compound = app
        .world()
        .get::<Collider>(root)
        .expect("the root carries the building's one collider");
    let pieces = compound
        .shape()
        .as_compound()
        .expect("the root collider is a compound of every solid")
        .shapes()
        .len();
    // Three whole walls, the two segments beside the doorway, the
    // floor, and the ceiling.
    assert_eq!(pieces, 7, "every solid's box is in the compound");
    assert_eq!(
        app.world()
            .get::<ColliderOf>(root)
            .expect("the root collider is attached to its own body")
            .body,
        root
    );
    let batches: Vec<Entity> = owned_entities
        .iter()
        .copied()
        .filter(|e| app.world().get::<StructureBatch>(*e).is_some())
        .collect();
    assert!(
        (1..=4).contains(&batches.len()),
        "one merged mesh per material, got {}",
        batches.len()
    );
    for entity in &batches {
        assert!(app.world().get::<Mesh3d>(*entity).is_some());
        assert!(app.world().get::<Collider>(*entity).is_none());
    }

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
fn flying_goes_through_the_wall_and_off_again_stops_at_it() {
    let (mut app, player) = walking_app(&one_room(), Vec3::new(2.0, 0.0, 0.0), 0.0);
    act(&mut app, player, Action::Fly);
    walk(&mut app, player, Vec2::new(0.0, 1.0), 30);
    let through = feet(&app, player);
    assert!(through.z < -8.0, "flying passes the wall at z -4.1, got {through}");
    // Off again: back inside, the wall stops the body as before.
    act(&mut app, player, Action::Fly);
    app.update();
    app.world_mut().get_mut::<Transform>(player).unwrap().translation = Vec3::new(2.0, EYE_HEIGHT, 0.0);
    walk(&mut app, player, Vec2::new(0.0, 1.0), 120);
    let stopped = feet(&app, player);
    assert!(stopped.z > -4.1 && stopped.z < -3.3, "walking again, got {stopped}");
}

#[test]
fn walls_block_and_the_doorway_does_not() {
    // Start inside the room, walk north (negative z) beside the
    // doorway: the wall at z -4.1 stops the body.
    let (mut app, player) = walking_app(&one_room(), Vec3::new(2.0, 0.0, 0.0), 0.0);
    walk(&mut app, player, Vec2::new(0.0, 1.0),120);
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
    walk(&mut app, player, Vec2::new(0.0, 1.0),120);
    let through = feet(&app, player);
    assert!(
        through.z < -5.0,
        "the body must pass through the open doorway, got {through}"
    );
}

/// Swinging the pipe at a wall: the fire system casts from the eye,
/// the hit resolves to a piece through the one damage function, the
/// timer spaces the swings, a dead piece rebakes the building
/// without it, and the body walks through where the wall was.
#[test]
fn swinging_the_pipe_breaks_a_wall_piece_and_leaves_a_gap() {
    // Start inside the room 1.5 m from the east wall (x 3.0 to 3.2),
    // facing it, within the pipe's 2 m reach.
    let (mut app, player) = walking_app(&one_room(), Vec3::new(1.5, 0.0, 0.0), -std::f32::consts::FRAC_PI_2);
    let root = app
        .world_mut()
        .query_filtered::<Entity, With<StructureRoot>>()
        .single(app.world())
        .unwrap();
    let pieces_before = app.world().get::<Pieces>(root).unwrap().0.len();
    let boxes_before = app
        .world()
        .get::<Collider>(root)
        .unwrap()
        .shape()
        .as_compound()
        .unwrap()
        .shapes()
        .len();
    assert_eq!(pieces_before, boxes_before, "one box in the compound per piece");

    // A pipe in hand, the trigger held.
    {
        let world = app.world_mut();
        let pipe = {
            let items = world.resource::<ItemRegistryRes>();
            modforge::item::create(items.0.def("pipe").unwrap(), 1, &[], 0.0, 0)
        };
        let mut hotbar = world.get_mut::<Hotbar>(player).unwrap();
        hotbar.0.slots.slots[0] = Some(pipe);
        hotbar.0.held = Some(0);
    }
    // One second of swings at 0.6 s apart: the first lands at once,
    // the second at 0.6 s. Two hits of 20 off a 100 health wall.
    for _ in 0..60 {
        act(&mut app, player, Action::Attack);
        app.update();
    }
    let wall_health = |app: &App| {
        app.world()
            .get::<Pieces>(root)
            .unwrap()
            .0
            .iter()
            .find(|p| p.center.x > 2.5)
            .map(|p| p.health)
    };
    assert_eq!(wall_health(&app), Some(60.0), "two swings in the first second");
    assert_eq!(app.world().get::<Pieces>(root).unwrap().0.len(), pieces_before);

    // Keep swinging until the wall dies (three more hits) and the
    // building rebakes without it.
    for _ in 0..150 {
        act(&mut app, player, Action::Attack);
        app.update();
    }
    assert_eq!(wall_health(&app), None, "the wall piece is gone");
    assert!(app.world().get::<Rebake>(root).is_none(), "rebake ran");
    let boxes_after = app
        .world()
        .get::<Collider>(root)
        .unwrap()
        .shape()
        .as_compound()
        .unwrap()
        .shapes()
        .len();
    assert_eq!(boxes_after, boxes_before - 1, "the compound lost the dead piece");

    // Walk east through the gap where the wall was.
    app.update();
    walk(&mut app, player, Vec2::new(0.0, 1.0),120);
    let out = feet(&app, player);
    assert!(out.x > 4.0, "the body walks through where the wall was, got {out}");
}

#[test]
fn stairs_are_climbed_front_on() {
    // 50 ticks at 5 m/s is 4.17 m: from z 1.0 up the 3 m run onto
    // the landing (z -3.0 to -3.5). Further is walking off its far
    // edge, which is a fall, not a stairs defect.
    let (mut app, player) = walking_app(&stairs(), Vec3::new(0.0, 0.0, 1.0), 0.0);
    walk(&mut app, player, Vec2::new(0.0, 1.0),50);
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
    walk(&mut app, player, Vec2::new(1.0, 0.0), 120);
    let side = feet(&app, player);
    assert!(
        side.x < -1.0 && side.y < 0.05,
        "the side of the stairs must block like a wall, got {side}"
    );
}
