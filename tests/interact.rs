//! Every interaction enters through modforge's one interact call;
//! these drive the production `interact_on_press` system with a
//! forced look target and check the engine side of each result.

use std::time::Duration;

use bevy::{ecs::system::SystemState, prelude::*, time::TimeUpdateStrategy};
use modforge::hud::{
    Holder, Holders, InteractKind, OpenPanel, SlotRef, drop_slot, end_drag, hotbar_slot,
    slot_label, start_drag, toggle_panel, use_slot,
};
use modforge::item::{ItemStack, create};
use modforge::structure::Aabb;
use modforge::survival::FULL;
use topside::{
    AppState,
    actor::{Hotbar, INVENTORY_SLOTS, Inventory},
    build_app,
    content::ItemRegistryRes,
    hud::HudRes,
    interact::{InteractInput, Interactable, LookingAt, WorldItem},
    player::Player,
    structure::Door,
    survival::{Rates, Survival},
};

/// A base-quality stack of a registered item, through the one
/// creation function.
fn stack(app: &App, item: &str, count: u32) -> ItemStack {
    let registry = app.world().resource::<ItemRegistryRes>();
    let def = registry
        .0
        .def(item)
        .unwrap_or_else(|| panic!("test item '{item}' is not registered"));
    create(def, count, &[], 0.0, 0)
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
    let scrap = stack(&app, "scrap", 3);
    let item = in_front(&mut app, InteractKind::Pickup, WorldItem { stack: scrap });
    press_e(&mut app, item, InteractKind::Pickup);
    assert!(app.world().get_entity(item).is_err(), "picked up item is gone");
    assert_eq!(
        app.world()
            .get::<Inventory>(player)
            .unwrap()
            .0
            .count_of("scrap"),
        3
    );
}

#[test]
fn full_inventory_leaves_the_item_in_the_world() {
    let (mut app, player) = playing_app();
    let cloth = stack(&app, "cloth", 20);
    let scrap = stack(&app, "scrap", 1);
    {
        let mut inventory = app.world_mut().get_mut::<Inventory>(player).unwrap();
        for slot in 0..INVENTORY_SLOTS {
            inventory.0.slots[slot] = Some(cloth.clone());
        }
    }
    let item = in_front(&mut app, InteractKind::Pickup, WorldItem { stack: scrap });
    press_e(&mut app, item, InteractKind::Pickup);
    assert!(
        app.world().get_entity(item).is_ok(),
        "a full inventory must not consume the item"
    );
    assert_eq!(
        app.world()
            .get::<Inventory>(player)
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
fn an_unregistered_item_is_never_picked_up() {
    let (mut app, player) = playing_app();
    assert!(
        app.world()
            .resource::<ItemRegistryRes>()
            .0
            .def("bogus")
            .is_none(),
        "the registry must reject unknown ids"
    );
    let bogus = ItemStack {
        item: "bogus".to_string(),
        count: 1,
        quality: None,
    };
    let item = in_front(&mut app, InteractKind::Pickup, WorldItem { stack: bogus });
    press_e(&mut app, item, InteractKind::Pickup);
    assert!(app.world().get_entity(item).is_ok(), "the item stays in the world");
    assert!(
        app.world()
            .get::<Inventory>(player)
            .unwrap()
            .0
            .slots
            .iter()
            .all(Option::is_none),
        "nothing enters the inventory"
    );
}

#[test]
fn quality_rolls_come_from_the_def() {
    let (app, _) = playing_app();
    let registry = app.world().resource::<ItemRegistryRes>();
    let hatchet = registry.0.def("hatchet").unwrap();
    // Empty odds: always base quality.
    assert_eq!(create(hatchet, 1, &[], 0.0, 7).quality, None);
    // Certain odds (1000 per mille for the first tier): always tier
    // 0, sibling within the def's count, and the same roll for the
    // same time and salt.
    let first = create(hatchet, 1, &[1000], 12.5, 7);
    let again = create(hatchet, 1, &[1000], 12.5, 7);
    let quality = first.quality.expect("certain odds roll a tier");
    assert_eq!(quality.tier, 0);
    assert!((1..=hatchet.quality_siblings).contains(&quality.sibling));
    assert_eq!(first.quality, again.quality);
    assert_eq!(first.item, "hatchet");
}

#[test]
fn survival_stats_drain_every_tick_and_food_restores_them() {
    let (mut app, player) = playing_app();
    let rates = app.world().resource::<Rates>().0;
    // 60 fixed ticks is one second.
    for _ in 0..60 {
        app.update();
    }
    let after = app.world().get::<Survival>(player).unwrap().0;
    assert!(
        (after.hunger - (FULL - rates.hunger_per_sec)).abs() < 0.05,
        "hunger drains at the rate: {after:?}"
    );
    assert!(
        (after.thirst - (FULL - rates.thirst_per_sec)).abs() < 0.05,
        "thirst drains at the rate: {after:?}"
    );

    // Eat a can from inventory slot 0 through the one call the
    // inventory HUD and the hotbar will make: hunger comes back,
    // the stack shrinks by one.
    let can = stack(&app, "canned food", 2);
    let restores = app
        .world()
        .resource::<ItemRegistryRes>()
        .0
        .def("canned food")
        .unwrap()
        .food
        .unwrap()
        .hunger;
    {
        let mut inventory = app.world_mut().get_mut::<Inventory>(player).unwrap();
        inventory.0.slots[0] = Some(can);
    }
    {
        let world = app.world_mut();
        let mut state: SystemState<(
            Res<ItemRegistryRes>,
            Query<(&mut Inventory, &mut Survival)>,
        )> = SystemState::new(world);
        let (registry, mut actors) = state.get_mut(world).unwrap();
        let (mut inventory, mut survival) = actors.get_mut(player).unwrap();
        survival.0.hunger = 10.0;
        survival
            .0
            .eat_from_slot(&mut inventory.0, 0, &registry.0)
            .unwrap();
    }
    let survival = app.world().get::<Survival>(player).unwrap().0;
    assert_eq!(survival.hunger, 10.0 + restores);
    assert_eq!(
        app.world()
            .get::<Inventory>(player)
            .unwrap()
            .0
            .count_of("canned food"),
        1
    );
}

#[test]
fn drag_across_holders_hotbar_key_eats_and_o_drops() {
    // The slot observers and the digit keys call start_drag,
    // end_drag, use_slot, and drop_slot on the player's three
    // holders; this drives the same calls headless.
    let (mut app, player) = playing_app();
    let scrap = stack(&app, "scrap", 5);
    let can = stack(&app, "canned food", 2);
    {
        let mut inventory = app.world_mut().get_mut::<Inventory>(player).unwrap();
        inventory.0.slots[0] = Some(scrap);
        inventory.0.slots[1] = Some(can);
    }
    let world = app.world_mut();
    let mut state: SystemState<(
        ResMut<HudRes>,
        Res<ItemRegistryRes>,
        Query<(&mut Inventory, &mut Hotbar, &mut Survival)>,
    )> = SystemState::new(world);
    let (mut hud, registry, mut actors) = state.get_mut(world).unwrap();
    let (mut inventory, mut hotbar, mut survival) = actors.get_mut(player).unwrap();
    toggle_panel(&mut hud.0, OpenPanel::Inventory);
    let inv = |index| SlotRef {
        holder: Holder::Inventory,
        index,
    };
    let bar = |index| SlotRef {
        holder: Holder::Hotbar,
        index,
    };
    let mut holders = Holders {
        inventory: &mut inventory.0,
        hotbar: &mut hotbar.0,
    };

    // Within the inventory.
    start_drag(&mut hud.0, &holders, inv(0));
    assert_eq!(hud.0.dragging, Some(inv(0)));
    end_drag(&mut hud.0, &mut holders, inv(3), &registry.0);
    assert_eq!(hud.0.dragging, None);
    assert!(holders.inventory.slots[0].is_none());
    assert_eq!(slot_label(holders.inventory.slots[3].as_ref()), "scrap x5");

    // Inventory to the hotbar slot key 1 uses.
    let key_one = hotbar_slot(1).unwrap();
    start_drag(&mut hud.0, &holders, inv(1));
    end_drag(&mut hud.0, &mut holders, bar(key_one), &registry.0);
    assert!(holders.inventory.slots[1].is_none());
    assert_eq!(slot_label(holders.hotbar.slots[key_one].as_ref()), "canned food x2");

    survival.0.hunger = 10.0;
    use_slot(&mut survival.0, holders.hotbar, key_one, &registry.0).unwrap();
    assert_eq!(survival.0.hunger, 60.0);
    assert_eq!(slot_label(holders.hotbar.slots[key_one].as_ref()), "canned food");
    assert!(
        use_slot(&mut survival.0, holders.inventory, 3, &registry.0).is_err(),
        "scrap is not food"
    );

    let dropped = drop_slot(holders.inventory, 3).unwrap();
    assert_eq!(dropped.count, 5);
    assert!(holders.inventory.slots[3].is_none());
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
