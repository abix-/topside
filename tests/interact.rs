//! Every interaction enters through modforge's one interact call;
//! these drive the production `interact_on_press` system with a
//! forced look target and check the engine side of each result.

use std::time::Duration;

use bevy::{prelude::*, time::TimeUpdateStrategy};
use modforge::hud::{InteractKind, OpenPanel};
use modforge::item::ItemStack;
use modforge::structure::Aabb;
use topside::{
    AppState, build_app,
    hud::HudRes,
    interact::{InteractInput, Interactable, LookingAt, WorldItem},
    player::{INVENTORY_SLOTS, Player, PlayerInventory},
    structure::Door,
};

fn stack(item: &str, count: u32) -> ItemStack {
    ItemStack {
        item: item.to_string(),
        count,
        quality: None,
    }
}

fn playing_app() -> (App, Entity) {
    let mut app = build_app(true);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / 60.0,
    )));
    let player = app
        .world_mut()
        .spawn((
            Transform::from_xyz(0.0, 1.7, 0.0),
            Player {
                yaw: 0.0,
                pitch: 0.0,
            },
        ))
        .id();
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();
    (app, player)
}

/// Press E at `target` for one fixed tick. `update_looking_at` runs
/// before `interact_on_press` in the same chain, so the look target
/// is forced by placing the interactable where the ray hits.
fn press_e(app: &mut App, target: Entity, kind: InteractKind) {
    let mut looking_at = app.world_mut().resource_mut::<LookingAt>();
    looking_at.entity = Some(target);
    looking_at.kind = Some(kind);
    app.world_mut().resource_mut::<InteractInput>().0 = true;
    app.update();
}

fn in_front(app: &mut App, kind: InteractKind, bundle: impl Bundle) -> Entity {
    let position = Vec3::new(0.0, 1.7, -1.0);
    app.world_mut()
        .spawn((
            Interactable {
                kind,
                aabb: Aabb::from_center_size(position, Vec3::splat(0.4)),
            },
            Transform::from_translation(position),
            bundle,
        ))
        .id()
}

#[test]
fn pickup_moves_the_stack_into_the_player_inventory() {
    let (mut app, player) = playing_app();
    let item = in_front(
        &mut app,
        InteractKind::Pickup,
        WorldItem {
            stack: stack("scrap", 3),
        },
    );
    press_e(&mut app, item, InteractKind::Pickup);
    assert!(app.world().get_entity(item).is_err(), "picked up item is gone");
    assert_eq!(
        app.world()
            .get::<PlayerInventory>(player)
            .unwrap()
            .0
            .count_of("scrap"),
        3
    );
}

#[test]
fn full_inventory_leaves_the_item_in_the_world() {
    let (mut app, player) = playing_app();
    {
        let mut inventory = app.world_mut().get_mut::<PlayerInventory>(player).unwrap();
        for slot in 0..INVENTORY_SLOTS {
            inventory.0.slots[slot] = Some(stack("cloth", 20));
        }
    }
    let item = in_front(
        &mut app,
        InteractKind::Pickup,
        WorldItem {
            stack: stack("scrap", 1),
        },
    );
    press_e(&mut app, item, InteractKind::Pickup);
    assert!(
        app.world().get_entity(item).is_ok(),
        "a full inventory must not consume the item"
    );
    assert_eq!(
        app.world()
            .get::<PlayerInventory>(player)
            .unwrap()
            .0
            .count_of("scrap"),
        0
    );
}

#[test]
fn door_toggles_open_and_closed() {
    let (mut app, _) = playing_app();
    let door = Door {
        open: false,
        origin: Vec3::ZERO,
        closed_center: Vec3::new(0.0, 1.3, -1.0),
        hinge: Vec3::new(-0.5, 1.3, -1.0),
        closed_rotation: Quat::IDENTITY,
        collider: Aabb::from_center_size(Vec3::new(0.0, 1.3, -1.0), Vec3::new(1.0, 2.6, 0.1)),
    };
    let transform = door.transform();
    let door = app.world_mut().spawn((door, transform)).id();
    press_e(&mut app, door, InteractKind::Door { open: false });
    assert!(app.world().get::<Door>(door).unwrap().open);
    press_e(&mut app, door, InteractKind::Door { open: true });
    assert!(!app.world().get::<Door>(door).unwrap().open);
}

#[test]
fn container_opens_the_inventory_panel() {
    let (mut app, _) = playing_app();
    let crate_ = in_front(&mut app, InteractKind::Container, ());
    press_e(&mut app, crate_, InteractKind::Container);
    assert_eq!(
        app.world().resource::<HudRes>().0.open_panel,
        OpenPanel::Inventory
    );
}
