use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};
use topside::{
    cel::CelMaterial,
    structure::{
        Door, Opening, RoomSpec, Side, StaticColliders, StructureDef, StructureRoot,
        StructureSolid, Walkables, spawn_structure,
    },
};

type SpawnStructureParams<'w, 's> = (
    Commands<'w, 's>,
    ResMut<'w, Assets<Mesh>>,
    ResMut<'w, Assets<CelMaterial>>,
    ResMut<'w, StaticColliders>,
    ResMut<'w, Walkables>,
);

#[test]
fn structure_root_owns_avian_geometry_and_collision() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PhysicsPlugins::default());
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<CelMaterial>>();
    app.init_resource::<StaticColliders>();
    app.init_resource::<Walkables>();
    app.finish();
    app.cleanup();

    let def = StructureDef {
        name: "physics ownership".to_string(),
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
    };

    let world = app.world_mut();
    let mut state: SystemState<SpawnStructureParams<'_, '_>> = SystemState::new(world);
    let root;
    {
        let (mut commands, mut meshes, mut materials, mut colliders, mut walkables) = state
            .get_mut(world)
            .expect("structure spawn system state is valid");
        root = spawn_structure(
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut colliders,
            &mut walkables,
            &def,
            Vec3::new(10.0, 0.0, -4.0),
        )
        .expect("structure def is valid");
    }
    state.apply(world);
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
        assert!(app.world().get::<Collider>(entity).is_some());
        assert_eq!(
            app.world()
                .get::<ColliderOf>(entity)
                .expect("solid collider is attached to a rigid body")
                .body,
            root
        );
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
