//! Living people (life.md): a hungry person goes to the box it saw food
//! in and eats; a person struck from behind turns on the striker it
//! never saw.

mod common;

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use common::{alone, game};
use modforge::brain::CombatState;
use modforge::item::{Inventory as Items, create};
use topside::actor::{ActorCombat, Home, Inventory, Memory, SpawnActor, Survival, spawn_actor};
use topside::combat::Bullet;
use topside::content::{ActorRegistryRes, ItemRegistryRes, OTHER_BUNKERS};
use topside::interact::{Container, Door, spawn_item};
use topside::terrain::WorldRes;

/// A wanderer (no faction) standing on open ground east of the bunker,
/// facing east, and the bunker point.
fn wanderer(app: &mut App) -> (Entity, Vec2) {
    let at = app.world().resource::<WorldRes>().0.def.bunker + Vec2::new(25.0, 0.0);
    let npc = app
        .world_mut()
        .run_system_once(move |mut spawn: SpawnActor, registry: Res<ActorRegistryRes>| {
            let def = registry.0.def("wanderer").unwrap().clone();
            spawn_actor(&mut spawn, &def, at, None).unwrap()
        })
        .unwrap();
    app.update();
    (npc, at)
}

/// A box of three cans at `box_at`, remembered by `npc` as holding
/// cans, and `npc` hungry. Returns the box.
fn hungry_for(app: &mut App, npc: Entity, box_at: Vec2) -> Entity {
    let found = app
        .world_mut()
        .run_system_once(move |mut commands: Commands, items: Res<ItemRegistryRes>| {
            let box_def = items.0.def("storage box").unwrap();
            let can = items.0.def("canned food").unwrap();
            let mut contents = Items::new(12);
            let _ = contents.add(create(can, 3, &[], 0.0, 0), 10);
            let e = spawn_item(&mut commands, box_def, create(box_def, 1, &[], 0.0, 0), box_at, [0.5, 0.4, 0.2]);
            commands.entity(e).insert(Container(contents));
            e
        })
        .unwrap();
    app.update();
    let mut memory = app.world_mut().get_mut::<Memory>(npc).unwrap();
    memory.0.see(found.to_bits(), "storage box", Vec3::new(box_at.x, 0.0, box_at.y), 1);
    memory.0.checked(found.to_bits(), vec!["canned food".to_string()], 1);
    app.world_mut().get_mut::<Survival>(npc).unwrap().0.hunger = 20.0;
    found
}

#[test]
fn a_hungry_person_walks_round_a_wall_to_the_food() {
    let (mut app, _) = game();
    alone(&mut app);
    let (npc, at) = wanderer(&mut app);
    // A wall 8 m long across the way, the box behind it.
    app.world_mut().spawn((
        avian2d::prelude::RigidBody::Static,
        avian2d::prelude::Collider::rectangle(0.5, 8.0),
        Transform::from_translation((at + Vec2::new(2.5, 0.0)).extend(0.0)),
    ));
    let found = hungry_for(&mut app, npc, at + Vec2::new(5.0, 0.0));
    for _ in 0..600 {
        app.update();
    }
    assert!(
        app.world().get::<Container>(found).unwrap().0.count_of("canned food") < 3,
        "it went round the wall and ate"
    );
}

#[test]
fn a_hungry_person_walks_to_the_box_it_saw_food_in_and_eats() {
    let (mut app, _) = game();
    alone(&mut app);
    let (npc, at) = wanderer(&mut app);
    // A box 5 m east with three cans.
    let box_at = at + Vec2::new(5.0, 0.0);
    let found = app
        .world_mut()
        .run_system_once(move |mut commands: Commands, items: Res<ItemRegistryRes>| {
            let box_def = items.0.def("storage box").unwrap();
            let can = items.0.def("canned food").unwrap();
            let mut contents = Items::new(12);
            let _ = contents.add(create(can, 3, &[], 0.0, 0), 10);
            let e = spawn_item(&mut commands, box_def, create(box_def, 1, &[], 0.0, 0), box_at, [0.5, 0.4, 0.2]);
            commands.entity(e).insert(Container(contents));
            e
        })
        .unwrap();
    app.update();
    // It saw the cans in there, and it is hungry.
    {
        let mut memory = app.world_mut().get_mut::<Memory>(npc).unwrap();
        memory.0.see(found.to_bits(), "storage box", Vec3::new(box_at.x, 0.0, box_at.y), 1);
        memory.0.checked(found.to_bits(), vec!["canned food".to_string()], 1);
    }
    app.world_mut().get_mut::<Survival>(npc).unwrap().0.hunger = 20.0;
    // It opens the box (Use), takes the cans (Transfer), and eats one
    // from its own bags (UseSlot): the player's own actions, one a
    // reaction apart.
    for _ in 0..600 {
        app.update();
    }
    assert_eq!(app.world().get::<Container>(found).unwrap().0.count_of("canned food"), 0, "the cans taken");
    let carried = app.world().get::<Inventory>(npc).unwrap().0.count_of("canned food");
    assert!((1..3).contains(&carried), "it carries the rest: {carried}");
    assert!(app.world().get::<Survival>(npc).unwrap().0.hunger > 50.0, "it ate");
}

/// A bunker person opens its bunker's door with the player's own Use
/// and goes out to food it knows of outside.
#[test]
fn a_bunker_person_opens_its_door_and_goes_out_for_food() {
    let (mut app, _) = game();
    let world = app.world_mut();
    let (npc, home) = world
        .query::<(Entity, &Home)>()
        .iter(world)
        .map(|(e, h)| (e, h.0))
        .min_by_key(|(e, _)| *e)
        .expect("a bunker person");
    let bunker = *OTHER_BUNKERS.iter().min_by(|a, b| a.distance(home).total_cmp(&b.distance(home))).unwrap();
    // It knows only a box of cans 15 m out past the door (the door is on
    // the bunker's -y side, as the player's).
    app.world_mut().get_mut::<Memory>(npc).unwrap().0 = Default::default();
    let found = hungry_for(&mut app, npc, bunker + Vec2::new(0.0, -15.0));
    for _ in 0..900 {
        app.update();
    }
    assert!(
        app.world().get::<Container>(found).unwrap().0.count_of("canned food") < 3,
        "it got out and took the cans"
    );
    let world = app.world_mut();
    let opened = world.query::<&Door>().iter(world).any(|d| d.open && d.closed_center.distance(bunker) < 6.0);
    assert!(opened, "through its door, opened");
}

#[test]
fn a_person_struck_from_behind_turns_on_the_striker() {
    let (mut app, player) = game();
    alone(&mut app);
    let (npc, at) = wanderer(&mut app);
    // A bullet from the player, from behind it (it faces east).
    app.world_mut().spawn((
        Bullet {
            shooter: player,
            damage: "shotgun pellet".to_string(),
            direction: Vec2::X,
            travelled: 0.0,
            range: 40.0,
        },
        Transform::from_translation((at + Vec2::new(-2.0, 0.0)).extend(1.5)),
    ));
    // Up to a second before it reacts: being hit earns no extra
    // reaction (life.md "How often a person thinks").
    for _ in 0..70 {
        app.update();
    }
    assert!(
        matches!(app.world().get::<ActorCombat>(npc).unwrap().0, CombatState::Fighting { .. }),
        "it turned on whoever hit it"
    );
}
