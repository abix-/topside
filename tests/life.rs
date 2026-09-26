//! Living people (life.md): a hungry person goes to the box it saw food
//! in and eats; a person struck from behind turns on the striker it
//! never saw.

use crate::common;

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use common::{alone, game};
use modforge::brain::CombatState;
use modforge::item::{Inventory as Items, create};
use topside::actor::{ActorCombat, Home, Inventory, Memory, SpawnActor, Store, Survival, spawn_actor};
use topside::combat::Bullet;
use topside::content::{ActorRegistryRes, ItemRegistryRes, OTHER_BUNKERS};
use topside::interact::{Container, Door, spawn_item};
use topside::terrain::WorldRes;

/// A wanderer (no faction) standing on open ground east of the bunker,
/// facing east, and the bunker point.
fn wanderer(app: &mut App) -> (Entity, Vec2) {
    let at = app.world().resource::<WorldRes>().bunker + Vec2::new(25.0, 0.0);
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
    memory.0.checked(found.to_bits(), vec![("canned food".to_string(), 3)], 1);
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
    // A box 5 m east with three cans it saw, and it is hungry.
    let found = hungry_for(&mut app, npc, at + Vec2::new(5.0, 0.0));
    // It opens the box (Use) and eats one can at a time out of it
    // (UseSlot) until fed: it eats what it needs and carries nothing
    // off. From 20, fed past 90 is two cans.
    for _ in 0..600 {
        app.update();
    }
    assert_eq!(app.world().get::<Container>(found).unwrap().0.count_of("canned food"), 1, "two cans eaten");
    assert_eq!(app.world().get::<Inventory>(npc).unwrap().0.count_of("canned food"), 0, "none carried off");
    assert!(app.world().get::<Survival>(npc).unwrap().0.hunger >= 90.0, "fed");
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

/// Their bunker short of food (design.md "Taking loot"), a bunker person
/// goes out to a box it knows holds cans, takes all of them (needed, so
/// as much as it can carry), and brings them home into the store.
#[test]
fn a_bunker_person_brings_food_home_to_a_short_store() {
    let (mut app, _) = game();
    let world = app.world_mut();
    let (npc, home, store) = world
        .query::<(Entity, &Home, &Store)>()
        .iter(world)
        .map(|(e, h, s)| (e, h.0, s.0))
        .min_by_key(|(e, _, _)| *e)
        .expect("a bunker person");
    let bunker = *OTHER_BUNKERS.iter().min_by(|a, b| a.distance(home).total_cmp(&b.distance(home))).unwrap();
    let before = app.world().get::<Container>(store).unwrap().0.count_of("canned food");
    // A box of cans 20 m out past the door that it has looked into; it
    // is not hungry itself. Its bags hold scrap in all but one slot, so
    // the cans fill them and it heads home (the bunker still short, it
    // would otherwise go on to the next box it knows).
    let found = hungry_for(&mut app, npc, bunker + Vec2::new(0.0, -20.0));
    app.world_mut().get_mut::<Survival>(npc).unwrap().0.hunger = 100.0;
    let scrap = {
        let items = app.world().resource::<ItemRegistryRes>();
        create(items.0.def("scrap").unwrap(), 1, &[], 0.0, 0)
    };
    let mut bags = app.world_mut().get_mut::<Inventory>(npc).unwrap();
    let last = bags.0.slots.len() - 1;
    for slot in &mut bags.0.slots[..last] {
        *slot = Some(scrap.clone());
    }
    for _ in 0..1800 {
        app.update();
    }
    assert_eq!(app.world().get::<Container>(found).unwrap().0.count_of("canned food"), 0, "it took all three");
    let after = app.world().get::<Container>(store).unwrap().0.count_of("canned food");
    let doing = app.world().get::<topside::actor::ActorActivity>(npc).unwrap().0.clone();
    let at = app.world().get::<Transform>(npc).unwrap().translation.truncate();
    let carried = app.world().get::<Inventory>(npc).unwrap().0.count_of("canned food");
    let open = app.world().get::<topside::interact::OpenContainer>(npc).unwrap().0;
    let way = app.world().get::<topside::nav::NavPath>(npc).unwrap().waypoints.clone();
    let world = app.world_mut();
    let door: Vec<(Vec2, bool)> = world
        .query::<&Door>()
        .iter(world)
        .filter(|d| d.closed_center.distance(bunker) < 6.0)
        .map(|d| (d.closed_center, d.open))
        .collect();
    assert!(
        after >= before + 3,
        "and brought them home: {before} then {after}; doing {doing:?} at {at} ({} m from home), carrying {carried} cans, open {open:?}, the store {store:?}, the way {way:?}, the door {door:?}",
        at.distance(home)
    );
}

/// An empty store and no box known: a bunker person heads out to find
/// out (whichever way it rolls, one of eight boxes 40 m around comes into
/// sight), takes the cans, and brings them home.
#[test]
fn with_an_empty_store_and_nothing_known_they_go_out_and_find_food() {
    let (mut app, _) = game();
    let world = app.world_mut();
    let (npc, home, store) = world
        .query::<(Entity, &Home, &Store)>()
        .iter(world)
        .map(|(e, h, s)| (e, h.0, s.0))
        .min_by_key(|(e, _, _)| *e)
        .expect("a bunker person");
    let bunker = *OTHER_BUNKERS.iter().min_by(|a, b| a.distance(home).total_cmp(&b.distance(home))).unwrap();
    // The store empty, and every bunker person knows it.
    app.world_mut().get_mut::<Container>(store).unwrap().0 = Items::new(12);
    let world = app.world_mut();
    let mates: Vec<Entity> = world.query_filtered::<Entity, With<Store>>().iter(world).collect();
    for mate in mates {
        if app.world().get::<Store>(mate).unwrap().0 == store {
            app.world_mut().get_mut::<Memory>(mate).unwrap().0.checked(store.to_bits(), vec![], 1);
        }
    }
    // Eight boxes of three cans, 40 m around the bunker, none known.
    let mut boxes = Vec::new();
    for k in 0..8 {
        let angle = k as f32 * std::f32::consts::FRAC_PI_4;
        let at = bunker + Vec2::from_angle(angle) * 40.0;
        let found = hungry_for(&mut app, npc, at);
        app.world_mut().get_mut::<Memory>(npc).unwrap().0.gone(found.to_bits());
        boxes.push(found);
    }
    app.world_mut().get_mut::<Survival>(npc).unwrap().0.hunger = 100.0;
    for _ in 0..1800 {
        app.update();
    }
    let in_store = app.world().get::<Container>(store).unwrap().0.count_of("canned food");
    assert!(in_store >= 3, "it found food and brought it home: {in_store} cans in the store");
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
