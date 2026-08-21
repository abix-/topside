//! The one NPC spawn path: every NPC enters through `spawn_actor`
//! with exactly what the player has, a slot, and an identity; the AI
//! then drives it through the same action queue as the keyboard.

use std::time::Duration;

use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*, time::TimeUpdateStrategy};
use modforge::item::EquipSlot;
use topside::{
    AppState, build_app,
    actor::{EYE_HEIGHT, Equipment, Facing, Hotbar, Inventory, Npc, SpawnActor, spawn_actor},
    cel::CelMaterial,
    combat::{Firing, Health, Protection},
    content::ActorRegistryRes,
    actor_map::{Actor, ActorMapRes},
    input::{Actions, ThisTick},
    player::Player,
    survival::Survival,
};

fn playing_app() -> (App, Entity) {
    let mut app = build_app(true);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / 60.0,
    )));
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<CelMaterial>>();
    app.world_mut()
        .spawn((RigidBody::Static, Collider::half_space(Vec3::Y)));
    let player = app
        .world_mut()
        .spawn((Transform::from_xyz(0.0, EYE_HEIGHT, 0.0), Player))
        .id();
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();
    (app, player)
}

fn spawn(app: &mut App, def: &str, feet: Vec3, yaw: f32) -> Entity {
    let def = app
        .world()
        .resource::<ActorRegistryRes>()
        .0
        .def(def)
        .expect("registered actor")
        .clone();
    let mut state: SystemState<SpawnActor> = SystemState::new(app.world_mut());
    let mut params = state.get_mut(app.world_mut()).expect("spawn params");
    let entity = spawn_actor(&mut params, &def, feet, yaw).expect("a free slot");
    state.apply(app.world_mut());
    entity
}

#[test]
fn spawn_actor_sets_identity_faction_position_and_the_shared_parts() {
    let (mut app, _) = playing_app();
    let a = spawn(&mut app, "scavenger", Vec3::new(5.0, 0.0, -5.0), 1.0);
    let b = spawn(&mut app, "raider", Vec3::new(-3.0, 0.0, 2.0), 0.0);
    let world = app.world_mut();

    let npc = world.get::<Npc>(a).unwrap();
    assert_eq!(npc.def, "scavenger");
    assert_eq!(npc.faction, "raiders");
    // Identity: one ActorId each, registered in the one map.
    let id_a = world.get::<Actor>(a).unwrap().0;
    let id_b = world.get::<Actor>(b).unwrap().0;
    assert_ne!(id_a, id_b, "each actor owns its id");
    let map = &world.resource::<ActorMapRes>().0;
    assert_eq!(map.handle_of(id_a), Some(a));
    assert_eq!(map.id_of(b), Some(id_b));
    assert_eq!(map.live_count(), 2);
    assert_eq!(map.row_count(), 2);

    let at = world.get::<Transform>(a).unwrap().translation;
    assert_eq!(at, Vec3::new(5.0, EYE_HEIGHT, -5.0), "eyes above the feet");
    assert_eq!(world.get::<Facing>(a).unwrap().yaw, 1.0);

    // Exactly what the player carries.
    assert!(world.get::<Inventory>(a).is_some());
    assert!(world.get::<Hotbar>(a).is_some());
    assert!(world.get::<Survival>(a).is_some());
    assert!(world.get::<Firing>(a).is_some());
    assert!(world.get::<Actions>(a).is_some());
    assert!(world.get::<ThisTick>(a).is_some());
    assert!(world.get::<Protection>(a).is_some());
    assert_eq!(world.get::<Health>(a).unwrap().0.max, 60.0, "health from the def");
    let weapon = world.get::<Equipment>(a).unwrap().0.get(EquipSlot::Weapon).unwrap();
    assert_eq!(weapon.item, "pipe", "armed from the def");

    // A body the world can hit hangs under the root.
    let children: Vec<Entity> = world.get::<Children>(a).unwrap().iter().collect();
    assert_eq!(children.len(), 1);
    assert!(world.get::<Collider>(children[0]).is_some());
}

#[test]
fn a_hunter_walks_at_the_player_through_the_same_action_queue() {
    let (mut app, player) = playing_app();
    // Ten metres in front of the player, facing away from it.
    let npc = spawn(&mut app, "scavenger", Vec3::new(0.0, 0.0, -10.0), 0.0);
    let before = app.world().get::<Transform>(npc).unwrap().translation;
    for _ in 0..60 {
        app.update();
    }
    let world = app.world();
    let after = world.get::<Transform>(npc).unwrap().translation;
    let this_tick = world.get::<ThisTick>(npc).unwrap();
    assert!(!this_tick.0.is_empty(), "the AI pushed actions that drained");
    assert!(
        after.z > before.z + 1.0,
        "the hunter turned and walked toward the player: {before} -> {after}"
    );
    assert!(
        (world.get::<Facing>(npc).unwrap().yaw.abs() - std::f32::consts::PI).abs() < 0.2,
        "it turned to face the player"
    );
    // The player never moved: its own queue was empty.
    assert_eq!(world.get::<Transform>(player).unwrap().translation, Vec3::new(0.0, EYE_HEIGHT, 0.0));
}
