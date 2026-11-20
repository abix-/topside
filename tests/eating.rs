//! NPCs eat, read from each person's memory in the running game.

use serde_json::json;

use crate::endpoint::{op_with, pause, step};
use crate::opening::{roxanne, within};

/// From the game's start (tap_0_you_wake), Roxanne made hungry with a can
/// of food in her hands (the `needs` and `give` ops) eats it within a few
/// game seconds: her memory holds the eating.
#[test]
fn npcs_eat() {
    let _p = pause();
    crate::opening::you_wake();
    let her = roxanne();
    op_with("give", json!({"id": her, "item": "canned food", "count": 1, "clear": true}));
    op_with("needs", json!({"id": her, "hunger": 10}));
    step(1);
    // The people op counts what each did from their memory.
    let her_now = || op_with("people", json!({"ids": [her]})).as_array().and_then(|p| p.first().cloned()).unwrap_or_default();
    let eaten = within(10, || her_now()["did"]["ate"].as_u64().unwrap_or(0) > 0);
    let n = her_now();
    assert!(eaten, "Roxanne, hungry with canned food, did not eat within ten game seconds: hunger {}, doing {}, did {}", n["hunger"], n["doing"], n["did"]);
}
