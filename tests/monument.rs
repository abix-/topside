//! A rolled monument spawns through the one path: members under one
//! root via spawn_structure, loot spots as storage boxes via
//! spawn_item, gates as locked doors.

use std::time::Duration;

use bevy::{ecs::system::SystemState, prelude::*, time::TimeUpdateStrategy};
use modforge::hud::InteractKind;
use modforge::survival::Need;
use topside::{
    AppState, SimTick,
    actor::{Facing, Home, Memory, Npc},
    build_app,
    cel::CelMaterial,
    content::{ItemRegistryRes, MonumentRegistryRes},
    actor_map::ActorMapRes,
    input::{Action, Actions},
    interact::{Container, LookingAt, WorldItem},
    player::Player,
    structure::{Door, Locked, MonumentRoot, StructureRoot, spawn_monument},
};

type SpawnParams<'w, 's> = (
    Commands<'w, 's>,
    ResMut<'w, Assets<Mesh>>,
    ResMut<'w, Assets<CelMaterial>>,
    Res<'w, ItemRegistryRes>,
    Res<'w, MonumentRegistryRes>,
);

/// Roll `kind` with the first seed that produces a gate, spawn it
/// headless, and tick once so doors exist and gates lock.
fn spawn_gated(kind: &str) -> (App, Entity, usize) {
    let mut app = build_app(true);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / 60.0,
    )));
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<CelMaterial>>();
    app.world_mut().spawn((
        Transform::from_xyz(0.0, 1.7, 0.0),
        Player,
    ));
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();

    let world = app.world_mut();
    let mut state: SystemState<SpawnParams<'_, '_>> = SystemState::new(world);
    let (root, members);
    {
        let (mut commands, mut meshes, mut materials, items, monuments) =
            state.get_mut(world).unwrap();
        let rolled = (0..40u64)
            .map(|seed| monuments.monuments.roll(kind, &monuments.buildings, seed).unwrap())
            .find(|m| !m.gates.is_empty())
            .expect("a seed with a gated back room");
        members = rolled.members.len();
        root = spawn_monument(
            &mut commands,
            &mut meshes,
            &mut materials,
            &items.0,
            &rolled,
            Vec3::new(100.0, 0.0, 100.0),
            0.0,
        )
        .unwrap();
    }
    state.apply(world);
    app.update();
    app.update();
    (app, root, members)
}

#[test]
fn a_rolled_monument_spawns_its_members_boxes_and_gate() {
    let (mut app, root, members) = spawn_gated("launch site");
    let world = app.world_mut();

    assert!(world.get::<MonumentRoot>(root).is_some());
    let children: Vec<Entity> = world.get::<Children>(root).unwrap().iter().collect();
    let structures = children
        .iter()
        .filter(|e| world.get::<StructureRoot>(**e).is_some())
        .count();
    assert_eq!(structures, members, "one structure root per member");

    let boxes = world
        .query::<(&WorldItem, &Container)>()
        .iter(world)
        .filter(|(item, _)| item.stack.item == "storage box")
        .count();
    assert_eq!(boxes, members, "one loot box per member");
    let stocked = world
        .query::<(&WorldItem, &Container)>()
        .iter(world)
        .filter(|(item, c)| item.stack.item == "storage box" && c.0.count_of("scrap") > 0)
        .count();
    assert_eq!(stocked, members, "every loot box holds scrap");

    // Every NPC spot was filled through the one spawn path, under
    // the root.
    let npcs = children
        .iter()
        .filter(|e| world.get::<Npc>(**e).is_some())
        .count();
    assert_eq!(npcs, members, "one NPC per spot, parented to the root");
    assert_eq!(world.resource::<ActorMapRes>().0.live_count(), members);

    // Each of them knows its home from the start: the place (good
    // for rest and safety) and every box in it (good for hunger),
    // and nothing else (life.md "What a person knows").
    let npc = *children.iter().find(|e| world.get::<Npc>(**e).is_some()).unwrap();
    let home = world.get::<Home>(npc).expect("a person spawned at a site has a home");
    assert_eq!(home.root, root);
    let memory = &world.get::<Memory>(npc).unwrap().0;
    assert_eq!(memory.known.len(), 1 + members, "the place and one box per building");
    assert!(memory.knows(root.to_bits()));
    assert!(memory.good_for(Need::Rest).count() >= 1, "home is good for rest");
    assert_eq!(memory.good_for(Need::Hunger).count(), members, "every box is good for hunger");
    assert!(memory.unchecked_nearest(home.position).is_some(), "boxes not yet checked");

    let locked: Vec<(Entity, u32)> = world
        .query::<(Entity, &Door, &Locked)>()
        .iter(world)
        .map(|(e, _, l)| (e, l.0))
        .collect();
    assert_eq!(locked.len(), 1, "the back room door is the gate");
    let (gate, level) = locked[0];
    assert_eq!(level, 5, "a launch site gate is level 5");

    // Stand west of the gate looking east at it (gates sit on west
    // walls, so the door plane is x = const), then press E: the
    // look path reports a gate, modforge answers Locked, the door
    // stays shut.
    let center = world.get::<Door>(gate).unwrap().closed_center;
    let yaw = -std::f32::consts::FRAC_PI_2;
    let player = world
        .query_filtered::<Entity, With<Player>>()
        .single(world)
        .unwrap();
    let mut transform = world.get_mut::<Transform>(player).unwrap();
    transform.translation = center + Vec3::new(-1.5, 0.4, 0.0);
    transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, 0.0, 0.0);
    world.get_mut::<Facing>(player).unwrap().yaw = yaw;
    app.update();
    assert_eq!(
        app.world().resource::<LookingAt>().kind,
        Some(InteractKind::Gate { level }),
        "looking at the gate reports it locked"
    );
    let next = app.world().resource::<SimTick>().0 + 1;
    app.world_mut()
        .get_mut::<Actions>(player)
        .unwrap()
        .0
        .push(next, Action::Use);
    app.update();
    assert!(!app.world().get::<Door>(gate).unwrap().open);

    // Despawning the root takes the members with it.
    app.world_mut().despawn(root);
    assert!(
        app.world_mut()
            .query::<&StructureRoot>()
            .iter(app.world())
            .next()
            .is_none()
    );
}
