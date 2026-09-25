//! The player's death (design.md "Death and difficulty": everyone is
//! equal): everything the player carried drops in a death box where
//! they fell, exactly like an NPC's, and the world keeps running. For
//! now the player then wakes in the bunker with nothing.

mod common;

use bevy::prelude::*;
use common::{act, game, out_in_the_open, position};
use modforge::item::create;
use topside::actor::{Dead, Hotbar, Inventory, Npc};
use topside::input::Action;
use topside::combat::Health;
use topside::content::ItemRegistryRes;
use topside::interact::Container;
use topside::terrain::WorldRes;

#[test]
fn the_player_drops_a_death_box_like_anyone_and_wakes_with_nothing() {
    let (mut app, player) = game();
    out_in_the_open(&mut app, player);
    let fell_at = position(&app, player);
    let (pipe, cans) = {
        let items = app.world().resource::<ItemRegistryRes>();
        (
            create(items.0.def("pipe").unwrap(), 1, &[], 0.0, 0),
            create(items.0.def("canned food").unwrap(), 3, &[], 0.0, 1),
        )
    };
    let _ = app.world_mut().get_mut::<Inventory>(player).unwrap().0.add(cans, 10);
    {
        let mut hotbar = app.world_mut().get_mut::<Hotbar>(player).unwrap();
        hotbar.0.slots.slots[0] = Some(pipe);
        hotbar.0.held = Some(0);
    }
    let world = app.world_mut();
    let people_before = world.query_filtered::<(), With<Npc>>().iter(world).count();

    app.world_mut().get_mut::<Health>(player).unwrap().0.current = 0.0;
    app.update();

    // The box where the player fell holds everything they carried.
    let world = app.world_mut();
    let (cans_in_box, pipes_in_box) = world
        .query::<(&Transform, &Container)>()
        .iter(world)
        .find(|(t, _)| t.translation.truncate().distance(fell_at) < 1.0)
        .map(|(_, c)| (c.0.count_of("canned food"), c.0.count_of("pipe")))
        .expect("a death box where the player fell");
    assert_eq!((cans_in_box, pipes_in_box), (3, 1));

    // Dead: the death message shows until the player chooses to go
    // again; the world keeps running.
    assert!(app.world().get::<Dead>(player).is_some(), "dead, waiting to respawn");
    for _ in 0..30 {
        app.update();
    }
    assert!(app.world().get::<Dead>(player).is_some(), "still waiting");

    // Respawn: in the bunker, whole, carrying nothing.
    act(&mut app, player, &[Action::Respawn]);
    assert!(app.world().get::<Dead>(player).is_none(), "back in the world");
    let bunker = app.world().resource::<WorldRes>().0.def.bunker;
    assert!(position(&app, player).distance(bunker) < 1.0, "respawned in the bunker");
    let health = &app.world().get::<Health>(player).unwrap().0;
    assert_eq!(health.current, health.max);
    assert_eq!(app.world().get::<Inventory>(player).unwrap().0.count_of("canned food"), 0);
    assert!(app.world().get::<Hotbar>(player).unwrap().0.held_stack().is_none(), "empty hands");
    let world = app.world_mut();
    let people_after = world.query_filtered::<(), With<Npc>>().iter(world).count();
    assert_eq!(people_after, people_before, "the world's people are all still there");
}
