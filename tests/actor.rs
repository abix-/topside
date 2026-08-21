//! The one NPC spawn path: every NPC enters through `spawn_actor`
//! with exactly what the player has, a slot, and an identity; the AI
//! then drives it through the same action queue as the keyboard.

use std::time::Duration;

use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*, time::TimeUpdateStrategy};
use modforge::hud::HOTBAR_SLOTS;
use modforge::item::EquipSlot;
use topside::{
    AppState, SimTick, build_app,
    actor::{EYE_HEIGHT, Equipment, Facing, Hotbar, Inventory, Npc, SpawnActor, spawn_actor},
    cel::CelMaterial,
    combat::{Firing, Health, Protection},
    content::{ActorRegistryRes, ItemRegistryRes},
    hud::HudRes,
    interact::{Container, WorldItem},
    actor_map::{Actor, ActorMapRes},
    input::{Action, Actions, ThisTick},
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
    try_spawn(app, def, feet, yaw, None).expect("a free row and a registered faction")
}

/// Through the one path, with a faction override as a placed actor
/// would carry.
fn try_spawn(app: &mut App, def: &str, feet: Vec3, yaw: f32, faction: Option<&str>) -> Option<Entity> {
    let def = app
        .world()
        .resource::<ActorRegistryRes>()
        .0
        .def(def)
        .expect("registered actor")
        .clone();
    let mut state: SystemState<SpawnActor> = SystemState::new(app.world_mut());
    let mut params = state.get_mut(app.world_mut()).expect("spawn params");
    let entity = spawn_actor(&mut params, &def, feet, yaw, faction);
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
    assert_eq!(npc.faction.as_deref(), Some("raiders"));
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
    let weapon = world.get::<Hotbar>(a).unwrap().0.held_stack().unwrap();
    assert_eq!(weapon.item, "pipe", "armed from the def, held from the bar");

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

/// Whether `npc` has anything to do this tick (the AI only pushes
/// when it sees a hostile).
fn acting(app: &App, npc: Entity) -> bool {
    !app.world().get::<ThisTick>(npc).unwrap().0.is_empty()
}

#[test]
fn targets_come_from_the_faction_table() {
    let (mut app, _player) = playing_app();
    // A raider 5 m ahead of the player sees a hostile: it acts.
    let raider = spawn(&mut app, "scavenger", Vec3::new(0.0, 0.0, -5.0), 0.0);
    // A neutral wanderer beside it sees no one.
    let neutral = spawn(&mut app, "wanderer", Vec3::new(6.0, 0.0, -5.0), 0.0);
    for _ in 0..3 {
        app.update();
    }
    assert!(acting(&app, raider), "a raider targets the player");
    assert!(!acting(&app, neutral), "a neutral targets no one");

    // Nor is the neutral a target: alone with a raider, far from the
    // player, the raider stands still.
    let (mut app, player) = playing_app();
    app.world_mut().get_mut::<Transform>(player).unwrap().translation = Vec3::new(500.0, EYE_HEIGHT, 0.0);
    let raider = spawn(&mut app, "scavenger", Vec3::new(0.0, 0.0, -5.0), 0.0);
    let _neutral = spawn(&mut app, "wanderer", Vec3::new(0.0, 0.0, -10.0), 0.0);
    for _ in 0..3 {
        app.update();
    }
    assert!(!acting(&app, raider), "a raider leaves a neutral alone");

    // Two hostile factions target each other: a warlord's lookout
    // (a placed actor carries its placer's faction) against a raider.
    let (mut app, player) = playing_app();
    app.world_mut().get_mut::<Transform>(player).unwrap().translation = Vec3::new(500.0, EYE_HEIGHT, 0.0);
    let raider = spawn(&mut app, "scavenger", Vec3::new(0.0, 0.0, -5.0), 0.0);
    let warlord = try_spawn(&mut app, "lookout", Vec3::new(0.0, 0.0, -12.0), 0.0, Some("warlords"))
        .expect("warlords is registered");
    assert_eq!(
        app.world().get::<Npc>(warlord).unwrap().faction.as_deref(),
        Some("warlords"),
        "the placed faction wins over the def's"
    );
    for _ in 0..3 {
        app.update();
    }
    assert!(acting(&app, raider) && acting(&app, warlord), "hostile factions fight each other");

    // An unregistered faction never enters the world.
    assert!(try_spawn(&mut app, "scavenger", Vec3::ZERO, 0.0, Some("nobody")).is_none());
}

/// Push an action onto an actor's queue for the next fixed tick.
fn act(app: &mut App, actor: Entity, action: Action) {
    let next = app.world().resource::<SimTick>().0 + 1;
    app.world_mut()
        .get_mut::<Actions>(actor)
        .unwrap()
        .0
        .push(next, action);
}

fn give(app: &mut App, actor: Entity, item: &str, count: u32) -> usize {
    let stack = {
        let items = app.world().resource::<ItemRegistryRes>();
        let def = items.0.def(item).expect("registered");
        (modforge::item::create(def, count, &[], 0.0, 0), def.max_stack)
    };
    let mut hotbar = app.world_mut().get_mut::<Hotbar>(actor).unwrap();
    let slot = (0..HOTBAR_SLOTS).find(|i| hotbar.0.slots.slots[*i].is_none()).unwrap();
    hotbar.0.slots.add(stack.0, stack.1);
    slot
}

/// An actor's health; one that died and despawned reads as zero.
fn health(app: &App, actor: Entity) -> f32 {
    app.world()
        .get::<Health>(actor)
        .map_or(0.0, |h| h.0.current)
}

#[test]
fn the_scavenger_hits_the_player_and_the_player_shoots_back() {
    let (mut app, player) = playing_app();
    // In reach, facing each other.
    let npc = spawn(&mut app, "scavenger", Vec3::new(0.0, 0.0, -1.5), std::f32::consts::PI);
    app.update();
    // The hunter sees the player in reach and swings its pipe. A level
    // swing from eye height lands on the head: 20 doubled, no armor.
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(health(&app, player), 60.0, "one pipe swing landed on the player's head");

    // The player equips a shotgun with shells from the hotbar and
    // fires once: six pellets at 12, falloff none this close, most
    // land on the chest or head of a body 1.5 m away.
    let gun = give(&mut app, player, "pipe shotgun", 1);
    give(&mut app, player, "shotgun shell", 2);
    act(&mut app, player, Action::Hotbar { index: gun });
    app.update();
    assert_eq!(
        app.world().get::<Hotbar>(player).unwrap().0.held_stack().map(|s| s.item.as_str()),
        Some("pipe shotgun"),
        "the gun is in hand and stays in the bar"
    );
    let before = health(&app, npc);
    act(&mut app, player, Action::Attack);
    app.update();
    let after = health(&app, npc);
    assert!(after < before, "pellets hurt: {before} -> {after}");
    assert!(
        before - after >= 12.0 * 3.0,
        "at least half the pellets land at 1.5 m: {before} -> {after}"
    );
    assert_eq!(
        app.world().get::<Hotbar>(player).unwrap().0.slots.count_of("shotgun shell"),
        1,
        "one shell spent"
    );
    // Fire until the shells are gone, then a click on empty changes
    // nothing.
    for _ in 0..120 {
        act(&mut app, player, Action::Attack);
        app.update();
    }
    assert_eq!(app.world().get::<Hotbar>(player).unwrap().0.slots.count_of("shotgun shell"), 0);
    assert!(
        app.world().get_entity(npc).is_err(),
        "two shells of pellets at 1.5 m finish a 60 health scavenger, and it is gone"
    );
}

#[test]
fn the_hud_names_the_actor_under_the_crosshair_and_its_health() {
    let (mut app, player) = playing_app();
    let npc = spawn(&mut app, "scavenger", Vec3::new(0.0, 0.0, -5.0), 0.0);
    app.update();
    let target = app.world().resource::<HudRes>().0.target.clone();
    assert_eq!(target, Some(("scavenger".to_string(), 60.0, 60.0)));

    // Shoot it: the line follows its health down.
    let gun = give(&mut app, player, "pipe shotgun", 1);
    give(&mut app, player, "shotgun shell", 1);
    act(&mut app, player, Action::Hotbar { index: gun });
    app.update();
    act(&mut app, player, Action::Attack);
    app.update();
    let after = app.world().resource::<HudRes>().0.target.clone().unwrap();
    assert_eq!(after.1, health(&app, npc));
    assert!(after.1 < 60.0, "the line shows the hit: {after:?}");

    // Look away: no target line.
    app.world_mut().get_mut::<Transform>(player).unwrap().rotation =
        Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    app.update();
    assert_eq!(app.world().resource::<HudRes>().0.target, None);
}

#[test]
fn a_dead_scavenger_leaves_a_box_with_its_pipe_and_its_row() {
    let (mut app, player) = playing_app();
    let npc = spawn(&mut app, "scavenger", Vec3::new(0.0, 0.0, -1.5), std::f32::consts::PI);
    let id = app.world().get::<Actor>(npc).unwrap().0;
    let feet = Vec3::new(0.0, 0.0, -1.5);
    // The scavenger carries a can besides its pipe.
    give(&mut app, npc, "canned food", 2);
    // Swing the pipe until it drops: 60 health, head hits of 40.
    let pipe = give(&mut app, player, "pipe", 1);
    act(&mut app, player, Action::Hotbar { index: pipe });
    for _ in 0..120 {
        act(&mut app, player, Action::Attack);
        app.update();
        if app.world().get_entity(npc).is_err() {
            break;
        }
    }
    let world = app.world_mut();
    assert!(world.get_entity(npc).is_err(), "the body is gone");
    let map = &world.resource::<ActorMapRes>().0;
    assert_eq!(map.handle_of(id), None, "it left the map");
    assert_eq!(map.live_count(), 0);
    assert_eq!(map.row_count(), 1, "the row is freed, the high-water mark stays");

    // One storage box where it stood, holding the pipe and the cans.
    let boxes: Vec<(Vec3, u32, u32)> = world
        .query::<(&Transform, &WorldItem, &Container)>()
        .iter(world)
        .filter(|(_, w, _)| w.stack.item == "storage box")
        .map(|(t, _, c)| (t.translation, c.0.count_of("pipe"), c.0.count_of("canned food")))
        .collect();
    assert_eq!(boxes.len(), 1, "one death box");
    let (at, pipes, cans) = boxes[0];
    assert!((at - feet).with_y(0.0).length() < 0.01, "where it stood: {at}");
    assert_eq!((pipes, cans), (1, 2));
}

#[test]
fn a_vest_guards_the_chest_and_only_the_chest() {
    let (mut app, player) = playing_app();
    let npc = spawn(&mut app, "scavenger", Vec3::new(0.0, 0.0, -1.5), std::f32::consts::PI);
    // Standing level, its swing from eye height would land on the
    // player's head; lower the scavenger so the swing lands on the
    // chest.
    app.world_mut().get_mut::<Transform>(npc).unwrap().translation.y -= 0.6;
    // Worn through the same use path as eating and equipping; the
    // vest is on before the first swing lands in the same tick.
    let vest = give(&mut app, player, "road sign vest", 1);
    act(&mut app, player, Action::Hotbar { index: vest });
    for _ in 0..4 {
        app.update();
    }
    assert!(
        app.world().get::<Equipment>(player).unwrap().0.get(EquipSlot::Chest).is_some(),
        "the vest is worn on the chest"
    );
    let chest_hit = 100.0 - health(&app, player);
    assert!(chest_hit > 0.0 && chest_hit < 20.0, "armor took part of the 20: {chest_hit}");
}
