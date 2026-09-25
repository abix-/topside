//! Living people (life.md): a hungry person goes to the box it saw food
//! in and eats; a person struck from behind turns on the striker it
//! never saw.

mod common;

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use common::{alone, game};
use modforge::brain::CombatState;
use modforge::item::{Inventory as Items, create};
use topside::actor::{ActorCombat, Memory, SpawnActor, Survival, spawn_actor};
use topside::combat::Bullet;
use topside::content::{ActorRegistryRes, ItemRegistryRes};
use topside::interact::{Container, spawn_item};
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
    for _ in 0..180 {
        app.update();
    }
    // The brain keeps eating until fed (hunger past 90): from 20 that is
    // two cans.
    assert_eq!(app.world().get::<Container>(found).unwrap().0.count_of("canned food"), 1, "two cans eaten");
    assert!(app.world().get::<Survival>(npc).unwrap().0.hunger >= 90.0, "fed");
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
    for _ in 0..5 {
        app.update();
    }
    assert!(
        matches!(app.world().get::<ActorCombat>(npc).unwrap().0, CombatState::Fighting { .. }),
        "it turned on whoever hit it"
    );
}
