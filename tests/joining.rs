//! Joining (todo 11af, operator 2026-10-02), the system alone: a person
//! moves to another bunker, their home, store and standing moving with
//! them. The player asks in talk ("You can stay with us.", the-tap.md pivot
//! point 6, draft) and they say yes if they feel well enough toward the
//! player; they leave on their own once they feel better toward another
//! group than their own. Feeling is raised the one way the game has, by
//! gifts (the talk path's Offer). From the play-through's saves (tests/
//! saves.rs), paused, moved only by `step`.

use serde_json::json;

use crate::endpoint::{op, op_with, pause};
use crate::opening::{SECOND, player, roxanne, within};
use crate::saves::{ask_to_join, faction_of, gifts, part, the_stranger};

fn bunker_count(faction: &str) -> u64 {
    op("world")["bunkers"][faction]["alive"].as_u64().unwrap_or(0)
}

/// Feeling well toward the player, a stranger asked to stay says yes: their
/// faction becomes the player's, their home the player's bunker (Roxanne's
/// home), and the bunker counts one more.
#[test]
fn asked_by_someone_they_like_a_stranger_joins() {
    let _p = pause();
    crate::saves::beside_a_stranger();
    let stranger = the_stranger();
    let count = bunker_count("player");
    let home_of = |id: u64| op_with("npc", json!({"id": id}))["home"].clone();
    let home = home_of(roxanne());
    gifts(player(), stranger, 3);
    ask_to_join(stranger);
    assert!(within(10, || faction_of(stranger) == "player"), "the stranger did not join: faction {:?}", faction_of(stranger));
    assert_eq!(home_of(stranger), home, "the stranger's home is not the player's bunker");
    assert_eq!(bunker_count("player"), count + 1, "the player's bunker does not count one more");
}

/// Feeling nothing for the player, a stranger asked to stay says no.
#[test]
fn asked_by_someone_they_do_not_know_a_stranger_stays_out() {
    let _p = pause();
    crate::saves::beside_a_stranger();
    let stranger = the_stranger();
    ask_to_join(stranger);
    crate::endpoint::step(10 * SECOND);
    assert_eq!(faction_of(stranger), "", "a stranger who feels nothing for the player joined: {:?}", faction_of(stranger));
}

/// Joined, then feeling better toward Mara's people than the player's, they
/// leave on their own for Mara's bunker.
#[test]
fn feeling_better_toward_another_group_they_leave_for_it() {
    let _p = pause();
    crate::saves::joined_with_mara_near();
    let mara = part("Mara");
    let stranger = crate::endpoint::people()
        .into_iter()
        .filter(|p| p["faction"] == "player" && p["dead"] != true)
        .filter_map(|p| p["id"].as_u64())
        .find(|id| *id != roxanne())
        .expect("the stranger who joined");
    let maras = faction_of(mara);
    gifts(mara, stranger, 8);
    assert!(within(120, || faction_of(stranger) == maras), "feeling better toward Mara's people, the stranger never left for them: {:?}", faction_of(stranger));
}
