//! Joining (todo 11af, operator 2026-10-02), the system alone: a person
//! moves to another bunker, their home, store and standing moving with
//! them. The player asks in talk ("You can stay with us.", the-tap.md
//! pivot point 6, draft) and they say yes if they feel well enough toward
//! the player; they leave on their own once they feel better toward
//! another group than their own. Feeling is raised the one way the game
//! has, by gifts (the talk path's Offer). From the opening's first save,
//! paused, moved only by `step`.

use serde_json::{Value as Json, json};

use crate::endpoint::{op, op_with, pause, people, step};
use crate::opening::{SECOND, apart, choices, choose, player, player_spot, spot_of, within, woke};

/// A living stranger, of no faction.
fn a_stranger() -> u64 {
    people().into_iter().find(|p| p["dead"] != true && p["faction"].is_null()).and_then(|p| p["id"].as_u64()).expect("a living stranger of no faction")
}

fn person(id: u64) -> Json {
    people().into_iter().find(|p| p["id"].as_u64() == Some(id)).unwrap_or(Json::Null)
}

/// `giver` gives `to` a water `times` times through the talk path's Offer.
fn gifts(giver: u64, to: u64, times: u32) {
    for _ in 0..times {
        op_with("give", json!({"id": giver, "item": "water", "count": 1, "clear": true}));
        step(SECOND);
        op_with("talk", json!({"id": giver, "to": to, "said": {"Offer": {"slot": 0}}}));
        step(SECOND);
    }
}

/// The player walks to `id` and asks them to stay with the player's
/// bunker.
fn ask_to_join(id: u64) {
    op_with("command", json!({"do": "move_to", "x": spot_of(id).0, "y": spot_of(id).2, "z": spot_of(id).1}));
    assert!(within(900, || apart(player_spot(), spot_of(id)).0 < 2.5), "setup: the player never reached {id}");
    op_with("use", json!({"id": player(), "person": id}));
    assert!(within(30, || choices().iter().any(|c| c == "You can stay with us.")), "the player cannot ask {id} to stay: {:?}", choices());
    choose(&["You can stay with us."]);
}

fn bunker_count(faction: &str) -> u64 {
    op("world")["bunkers"][faction]["alive"].as_u64().unwrap_or(0)
}

/// Feeling well toward the player, a stranger asked to stay says yes: their
/// faction becomes the player's, their home the player's bunker, and the
/// bunker counts one more.
#[test]
fn asked_by_someone_they_like_a_stranger_joins() {
    let _p = pause();
    woke();
    let stranger = a_stranger();
    let count = bunker_count("player");
    // The player's bunker as a home: Roxanne's, who lives there.
    let home_of = |id: u64| op_with("npc", json!({"id": id}))["home"].clone();
    let home = home_of(crate::opening::roxanne());
    gifts(player(), stranger, 3);
    ask_to_join(stranger);
    assert!(within(10, || person(stranger)["faction"] == "player"), "the stranger did not join: faction {}", person(stranger)["faction"]);
    assert_eq!(home_of(stranger), home, "the stranger's home is not the player's bunker");
    assert_eq!(bunker_count("player"), count + 1, "the player's bunker does not count one more");
}

/// Feeling nothing for the player, a stranger asked to stay says no.
#[test]
fn asked_by_someone_they_do_not_know_a_stranger_stays_out() {
    let _p = pause();
    woke();
    let stranger = a_stranger();
    ask_to_join(stranger);
    step(10 * SECOND);
    assert!(person(stranger)["faction"].is_null(), "a stranger who feels nothing for the player joined: {}", person(stranger)["faction"]);
}

/// Joined, then feeling better toward Mara's people than the player's, they
/// leave on their own for Mara's bunker.
#[test]
fn feeling_better_toward_another_group_they_leave_for_it() {
    let _p = pause();
    woke();
    let stranger = a_stranger();
    gifts(player(), stranger, 3);
    ask_to_join(stranger);
    assert!(within(10, || person(stranger)["faction"] == "player"), "setup: the stranger did not join");
    let mara = op("world")["cast"].as_array().and_then(|c| c.iter().find(|p| p["part"] == "Mara")).and_then(|p| p["id"].as_u64()).expect("Mara's part");
    let maras = person(mara)["faction"].as_str().unwrap_or_default().to_string();
    let at = spot_of(stranger);
    op_with("command", json!({"id": mara, "do": "move_to", "x": at.0, "y": at.2, "z": at.1}));
    assert!(within(900, || apart(spot_of(mara), spot_of(stranger)).0 < 2.5), "setup: Mara never reached the stranger");
    gifts(mara, stranger, 8);
    assert!(within(600, || person(stranger)["faction"] == maras.as_str()), "feeling better toward Mara's people, the stranger never left for them: {}", person(stranger)["faction"]);
}
