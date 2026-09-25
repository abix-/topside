//! Loot: items lie in the world through one `spawn_item`, E picks one
//! up or opens a box through modforge's `interact`, the dead leave a
//! box with everything they carried, and the note is read where it
//! lies.

mod common;

use bevy::prelude::*;
use common::{act, game};
use modforge::hud::OpenPanel;
use topside::actor::{Inventory, Npc};
use topside::combat::Health;
use topside::input::Action;
use topside::interact::{Container, HudRes, OpenContainer, WorldItem};
use topside::terrain::WorldRes;

fn put(app: &mut App, who: Entity, at: Vec2) {
    let mut t = app.world_mut().get_mut::<Transform>(who).unwrap();
    t.translation = at.extend(t.translation.z);
}

fn items_named(app: &mut App, name: &str) -> Vec<Entity> {
    let world = app.world_mut();
    world
        .query::<(Entity, &WorldItem)>()
        .iter(world)
        .filter(|(_, w)| w.stack.item == name)
        .map(|(e, _)| e)
        .collect()
}

#[test]
fn e_picks_the_pipe_up_off_the_bunker_floor() {
    let (mut app, player) = game();
    let bunker = app.world().resource::<WorldRes>().0.def.bunker;
    assert_eq!(items_named(&mut app, "pipe").len(), 1, "the pipe lies in the bunker");
    // Beside the pipe, nearer it than anything else.
    put(&mut app, player, bunker + Vec2::new(1.0, 3.2));
    act(&mut app, player, &[Action::Use]);
    assert!(items_named(&mut app, "pipe").is_empty(), "the pipe left the floor");
    assert_eq!(app.world().get::<Inventory>(player).unwrap().0.count_of("pipe"), 1);
}

#[test]
fn e_reads_the_note_where_it_lies() {
    let (mut app, player) = game();
    let bunker = app.world().resource::<WorldRes>().0.def.bunker;
    // Beside the bench the note lies on, nearer it than the vest.
    put(&mut app, player, bunker + Vec2::new(2.6, 2.9));
    act(&mut app, player, &[Action::Use]);
    let hud = &app.world().resource::<HudRes>().0;
    assert_eq!(hud.open_panel, OpenPanel::Note);
    assert!(hud.reading.as_ref().is_some_and(|n| !n.text.is_empty()), "the words show");
    assert_eq!(items_named(&mut app, "note").len(), 1, "a note is read, not taken");
}

#[test]
fn the_dead_leave_a_box_with_what_they_carried_and_e_opens_it() {
    let (mut app, player) = game();
    let world = app.world_mut();
    let npc = world
        .query_filtered::<Entity, With<Npc>>()
        .iter(world)
        .next()
        .expect("an NPC stands somewhere");
    let fell_at = app.world().get::<Transform>(npc).unwrap().translation.truncate();
    app.world_mut().get_mut::<Health>(npc).unwrap().0.current = 0.0;
    app.update();
    assert!(app.world().get_entity(npc).is_err(), "the body is gone");
    let world = app.world_mut();
    let (dropped, weapons) = world
        .query::<(Entity, &Transform, &Container)>()
        .iter(world)
        .find(|(_, t, _)| t.translation.truncate().distance(fell_at) < 0.01)
        .map(|(e, _, c)| (e, c.0.count_of("pipe") + c.0.count_of("hatchet")))
        .expect("a box where it fell");
    assert_eq!(weapons, 1, "the box holds its weapon");
    // Right at the box, so nothing else is nearer.
    put(&mut app, player, fell_at + Vec2::new(0.1, 0.0));
    act(&mut app, player, &[Action::Use]);
    assert_eq!(app.world().resource::<OpenContainer>().0, Some(dropped), "E opens it");
}
