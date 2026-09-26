//! The starting bunker: its walls hold the player in, its closed door
//! too, and Use opens the door to let the player out.

use crate::common;

use common::{alone, game, hold, position};
use topside::input::Action;

const S: Action = Action::Move { x: 0.0, y: -1.0 };
const D: Action = Action::Move { x: 1.0, y: 0.0 };

#[test]
fn the_bunker_walls_hold_the_player_in() {
    let (mut app, player) = game();
    alone(&mut app);
    // The room is 6 m wide inside, centred on the bunker point.
    hold(&mut app, player, D, 90);
    let x = position(&app, player).x;
    assert!(x < 3.0 && x > 2.4, "the east wall stops the player: x {x}");
}

#[test]
fn the_door_holds_until_used_then_lets_the_player_out() {
    let (mut app, player) = game();
    alone(&mut app);
    // The door is north in the bunker's plan, down the screen.
    hold(&mut app, player, S, 90);
    let at_door = position(&app, player).y;
    assert!(at_door > -4.0, "the closed door stops the player: y {at_door}");
    hold(&mut app, player, Action::Use, 1);
    hold(&mut app, player, S, 90);
    let outside = position(&app, player).y;
    assert!(outside < -5.0, "through the open door and out: y {outside}");
}
