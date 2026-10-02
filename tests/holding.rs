//! Holding a place (todo 11z, operator 2026-10-02), the system alone: a
//! person told to hold a place stays at it, and turns on anyone they stand
//! hostile to who comes for it within 8 m: one warning line, then a fight if
//! the intruder stays. Started here by the `hold` command (todo 11z: "a
//! `hold` command or an episode asking"). From the opening's first save,
//! paused, moved only by `step`.

use serde_json::json;

use crate::endpoint::{from_save, op_with, pause, people, step};
use crate::opening::{SECOND, apart, spot_of, within, woke};

/// Two living people far from the player's bunker: the holder, and one who
/// will come for the holder's place.
fn two_people() -> (u64, u64) {
    let far: Vec<u64> = people()
        .into_iter()
        .filter(|p| p["dead"] != true && p["faction"] != "player")
        .filter_map(|p| p["id"].as_u64())
        .collect();
    assert!(far.len() >= 2, "fewer than two people outside the player's bunker");
    (far[0], far[1])
}

/// Told to hold where they stand, a person is still within 8 m of it two
/// game minutes later.
#[test]
fn told_to_hold_a_place_they_stay() {
    let _p = pause();
    woke();
    let (holder, _) = two_people();
    let place = spot_of(holder);
    op_with("command", json!({"id": holder, "do": "hold", "x": place.0, "y": place.2, "z": place.1}));
    step(SECOND);
    for _ in 0..120 {
        step(SECOND);
        assert!(apart(spot_of(holder), place).0 <= 8.0, "the holder left the place: {:?} from {place:?}", spot_of(holder));
    }
}

/// Someone the holder stands hostile to by faction (a raider, against a
/// bunker person: the brain's one rule, modforge relationship::stands)
/// comes within 8 m: the holder says one warning line to them, and, when
/// they stay, fights them.
#[test]
fn a_hostile_who_comes_is_warned_once_then_fought() {
    let _p = pause();
    // Just before the raider comes within 8 m of the held place (tests/
    // saves.rs holding_raider_coming): the holder is the bunker person
    // commanded to hold, the raider the one walking for it.
    from_save("holding_raider_coming");
    let everyone = people();
    let alive = |def: &str| everyone.iter().filter(|p| p["dead"] != true && p["def"] == def && p["faction"] != "player").filter_map(|p| p["id"].as_u64()).next();
    let holder = alive("bunker person").expect("a living person of another bunker");
    let intruder = alive("raider").expect("a living raider");
    let place = spot_of(holder);
    assert!(within(60, || apart(spot_of(intruder), place).0 < 8.0), "the raider never came within 8 m of the held place");
    let warned = || heard_from_to(holder, intruder);
    assert!(within(10, || warned() >= 1), "the holder never warned the intruder");
    let health = |id: u64| op_with("npc", json!({"id": id}))["health"].as_f64().unwrap_or(0.0);
    let before = health(intruder);
    assert!(within(30, || health(intruder) < before), "the intruder stayed and the holder never fought them");
    assert_eq!(warned(), 1, "the holder warned more than once");
}

/// How many lines `by` said to `to`, from `by`'s memory (the npc op's did
/// list: "said to <id>: <words>", modforge memory.rs Did::words).
fn heard_from_to(by: u64, to: u64) -> usize {
    let said_to = format!("said to {to}:");
    op_with("npc", json!({"id": by}))["did"].as_array().map_or(0, |d| d.iter().filter(|d| d["did"].as_str().is_some_and(|w| w.starts_with(&said_to))).count())
}
