//! Newcomers (todo 11i, operator 2026-10-02), the system alone: word of a
//! place reaches the edge when someone who knows it tells people of other
//! factions; a group of 2 to 4 then rolls at the edge and walks there, at
//! most one group a game hour. Word is told here through the talk path's
//! Tell (the talk op), by Roxanne, who knows the tap from the start, to
//! Mara, of another bunker. From the opening's first save, paused, moved
//! only by `step`.

use serde_json::json;

use crate::endpoint::{op, op_with, pause, people, step};
use crate::opening::{SECOND, apart, dist, roxanne, spot_of, the_tap, within, woke};

fn ids() -> Vec<u64> {
    people().into_iter().filter_map(|p| p["id"].as_u64()).collect()
}

/// Roxanne tells Mara of the tap: word of it goes past the bunkers.
fn tell_mara_of_the_tap() {
    let w = op("world");
    let mara = w["cast"].as_array().and_then(|c| c.iter().find(|p| p["part"] == "Mara")).and_then(|p| p["id"].as_u64()).expect("Mara's part");
    let tap = w["things"].as_array().and_then(|t| t.iter().find(|t| t["name"] == "tap")).and_then(|t| t["key"].as_u64()).expect("the tap's key");
    let at = spot_of(mara);
    op_with("command", json!({"id": roxanne(), "do": "move_to", "x": at.0, "y": at.2, "z": at.1}));
    assert!(within(900, || apart(spot_of(roxanne()), spot_of(mara)).0 < 2.5), "setup: Roxanne never reached Mara");
    op_with("talk", json!({"id": roxanne(), "to": mara, "said": {"Tell": {"key": tap}}}));
    step(SECOND);
}

/// Word told, 2 to 4 people who were not in the world roll at its edge
/// (farther from the world's middle than any monument, less 50 m) within a
/// game hour, and walk toward the place.
#[test]
fn word_of_a_place_brings_a_group_from_the_edge() {
    let _p = pause();
    woke();
    let before = ids();
    tell_mara_of_the_tap();
    let new = || ids().into_iter().filter(|id| !before.contains(id)).collect::<Vec<u64>>();
    assert!(within(3600, || new().len() >= 2), "no newcomers within a game hour of word told");
    let group = new();
    assert!(group.len() <= 4, "{} newcomers in one group, more than 4", group.len());
    let rim = op("world")["monuments"].as_array().into_iter().flatten().map(|m| dist((m["at"][0].as_f64().unwrap_or(0.0), m["at"][1].as_f64().unwrap_or(0.0)), (0.0, 0.0))).fold(0.0, f64::max);
    for id in &group {
        let at = spot_of(*id);
        assert!(dist((at.0, at.1), (0.0, 0.0)) >= rim - 50.0, "newcomer {id} rolled at {at:?}, not at the edge (monuments reach {rim:.0} m out)");
    }
    let tap = the_tap();
    let start: Vec<f64> = group.iter().map(|id| apart(spot_of(*id), tap).0).collect();
    step(30 * SECOND);
    for (id, d) in group.iter().zip(&start) {
        assert!(apart(spot_of(*id), tap).0 < d - 5.0, "newcomer {id} is not walking toward the tap");
    }
}

/// At most one group a game hour: word told again right after a group
/// came brings no second group within the rest of that hour.
#[test]
fn at_most_one_group_a_game_hour() {
    let _p = pause();
    woke();
    let before = ids();
    tell_mara_of_the_tap();
    let new = |since: &[u64]| ids().into_iter().filter(|id| !since.contains(id)).count();
    let tick = || op("status")["tick"].as_u64().unwrap_or(0);
    assert!(within(3600, || new(&before) >= 2), "setup: no first group came");
    // Seen a game second at a time, so the group rolled within a second
    // before this tick.
    let came_at = tick();
    let after_first = ids();
    tell_mara_of_the_tap();
    // Until 59 game minutes after the group came: inside its hour.
    let until = came_at + 59 * 60 * SECOND;
    while tick() < until {
        step((until - tick()).min(600 * SECOND));
        assert_eq!(new(&after_first), 0, "a second group came within the game hour of the first (tick {})", tick());
    }
}
