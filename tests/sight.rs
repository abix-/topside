//! Nobody sees behind walls (design.md "What 2D changes"): the
//! scavenger at the door neither fights nor shows while the player is
//! shut in the bunker, and does both once the player stands before it.

mod common;

use bevy::prelude::*;
use common::game;
use modforge::brain::CombatState;
use topside::actor::{ActorCombat, Npc};
use topside::content::DOOR_ENEMY;

#[test]
fn the_door_scavenger_sees_only_through_open_ground() {
    let (mut app, player) = game();
    let world = app.world_mut();
    let scavenger = world
        .query::<(Entity, &Npc)>()
        .iter(world)
        .find(|(_, n)| n.def == DOOR_ENEMY.0)
        .map(|(e, _)| e)
        .expect("the scavenger at the door");
    // Shut in the bunker, the door closed.
    for _ in 0..60 {
        app.update();
    }
    assert_eq!(app.world().get::<ActorCombat>(scavenger).unwrap().0, CombatState::None, "it cannot see in");
    assert_eq!(*app.world().get::<Visibility>(scavenger).unwrap(), Visibility::Hidden, "nor the player it");

    // Out in the open, 5 m in front of where it faces (it sees a cone
    // ahead, not all round).
    let at = app.world().get::<Transform>(scavenger).unwrap().translation.truncate();
    let ahead = app.world().get::<topside::actor::Facing>(scavenger).unwrap().direction();
    let mut t = app.world_mut().get_mut::<Transform>(player).unwrap();
    t.translation = (at + ahead * 5.0).extend(t.translation.z);
    for _ in 0..3 {
        app.update();
    }
    assert!(
        matches!(app.world().get::<ActorCombat>(scavenger).unwrap().0, CombatState::Fighting { .. }),
        "it sees the player and fights"
    );
    assert_eq!(*app.world().get::<Visibility>(scavenger).unwrap(), Visibility::Inherited, "and is seen");
}
