//! People come back from the dead with half health, full food, and full
//! water (operator, 2026-09-26), so nobody who respawned starves or dies
//! of thirst straight away. Read from the game's state the moment they are
//! back (authority.md "Running tests": a rule with a long span is read
//! from the state, never waited out).

use serde_json::json;

use crate::endpoint::{op, op_with, pause, step};

/// From the game's start (tap_0_you_wake), the player starving and
/// parched is killed (the `kill` op) and presses Enter to come back: back,
/// their food and water are full, their health half, and their memory
/// holds the respawn.
#[test]
fn the_respawned_do_not_starve_straight_away() {
    let _p = pause();
    crate::opening::you_wake();
    let me = op("player")["id"].as_u64().expect("the player's id");
    op_with("needs", json!({"id": me, "hunger": 3, "thirst": 3}));
    step(1);
    op_with("kill", json!({"id": me}));
    // The kill is taken on the next step and the death lands through the
    // one hit path: a tick at a time, at most a game second.
    let died = (0..crate::opening::SECOND).any(|_| {
        step(1);
        op("player")["dead"] == true
    });
    assert!(died, "the killed player is not dead: {}", op("player"));
    op_with("key", json!({"key": "Enter"}));
    step(crate::opening::SECOND);
    let (p, s) = (op("player"), op("status"));
    eprintln!("back: dead {}, health {}, hunger {}, thirst {}; did {}", p["dead"], s["health"], s["hunger"], s["thirst"], p["did"]);
    assert!(p["dead"] == false, "Enter pressed and the player did not come back: {p}");
    let f = |k: &str| s[k].as_f64().unwrap_or(0.0);
    // A game second after coming back: full less a second's drain.
    assert!(f("hunger") > 99.0 && f("thirst") > 99.0, "the respawned player came back without full food and water: hunger {}, thirst {}", f("hunger"), f("thirst"));
    assert!((f("health") - 50.0).abs() < 1.0, "the respawned player came back with health {}, not half", f("health"));
}
