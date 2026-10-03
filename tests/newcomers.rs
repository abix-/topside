//! Newcomers (todo 11i, operator 2026-10-02), the system alone: word of a
//! place reaches the edge when someone who knows it tells people of other
//! factions; a group of 2 to 4 then rolls at the edge and walks there, at
//! most one group a game hour. Word is told by Roxanne, who knows the tap,
//! to Mara, of another bunker (the play-through's save newcomers_word_told,
//! just before the group rolls). Paused, moved only by `step`.

use crate::endpoint::{op, pause, people, step};
use crate::opening::{SECOND, apart, dist, spot_of, the_tap, within};
use crate::saves::newest;

/// The newcomers since `last`: people whose ActorId is past it (ids are
/// issued in order).
fn since(last: u64) -> Vec<u64> {
    people().into_iter().filter_map(|p| p["id"].as_u64()).filter(|id| *id > last).collect()
}

/// From just before, 2 to 4 people who were not in the world roll at its
/// edge (within 50 m of the world's border; in a world with no border,
/// farther from its middle than any monument, less 50 m) and walk toward
/// the place.
#[test]
fn word_of_a_place_brings_a_group_from_the_edge() {
    let _p = pause();
    crate::saves::newcomers_word_told();
    let last = newest();
    assert!(within(120, || since(last).len() >= 2), "no newcomers came");
    let group = since(last);
    assert!(group.len() <= 4, "{} newcomers in one group, more than 4", group.len());
    let w = op("world");
    let rim = w["monuments"].as_array().into_iter().flatten().map(|m| dist((m["at"][0].as_f64().unwrap_or(0.0), m["at"][1].as_f64().unwrap_or(0.0)), (0.0, 0.0))).fold(0.0, f64::max);
    // How far inside the world's border a point is: the nearest of its
    // four sides.
    let to_border = |at: (f64, f64)| {
        let b = &w["bounds"];
        let n = |i: usize, j: usize| b[i][j].as_f64().unwrap_or(f64::NAN);
        (at.0 - n(0, 0)).min(n(1, 0) - at.0).min(at.1 - n(0, 1)).min(n(1, 1) - at.1)
    };
    for id in &group {
        let at = spot_of(*id);
        let at_the_edge = if w["bounds"].is_null() { dist((at.0, at.1), (0.0, 0.0)) >= rim - 50.0 } else { to_border((at.0, at.1)) <= 50.0 };
        assert!(at_the_edge, "newcomer {id} rolled at {at:?}, not at the edge (the world's bounds {}, monuments reach {rim:.0} m out)", w["bounds"]);
    }
    let tap = the_tap();
    let start: Vec<f64> = group.iter().map(|id| apart(spot_of(*id), tap).0).collect();
    step(30 * SECOND);
    for (id, d) in group.iter().zip(&start) {
        let walking = apart(spot_of(*id), tap).0 < d - 5.0;
        if !walking {
            let n = crate::endpoint::op_with("npc", serde_json::json!({"id": id}));
            eprintln!(
                "newcomer {id} from {d:.0} m to {:.0} m of the tap {tap:?}: at {} feet {} doing {} about {} way {} needs {} acting {}",
                apart(spot_of(*id), tap).0, n["at"], n["feet"], n["doing"], n["about"], n["way"], n["needs"], n["this_tick"]
            );
            crate::endpoint::why_they_stopped(*id, (tap.0, tap.2 + 0.1, tap.1));
            eprintln!("their chunk: {}", n["chunk"]);
        }
        assert!(walking, "newcomer {id} is not walking toward the tap");
    }
}

/// At most one group a game hour, read where the edge keeps it: after a
/// group rolls, the edge's next group may not come before a game hour from
/// then (the world op's `edge.next_group_at`, a tick), and word told again
/// at once brings no group until a game minute before it may: the save
/// points newcomers_first_group, then newcomers_before_next_group (word
/// told again, grown to that minute), so no test plays the hour itself.
#[test]
fn at_most_one_group_a_game_hour() {
    let _p = pause();
    crate::saves::newcomers_first_group();
    let came_at = op("status")["tick"].as_u64().unwrap_or(0);
    let next = op("world")["edge"]["next_group_at"].as_u64().expect("the world op reports when the edge's next group may come");
    assert!(next >= came_at + 59 * 60 * SECOND, "the edge may send the next group at tick {next}, before an hour after the first (tick {came_at})");
    let after_first = newest();
    crate::saves::newcomers_before_next_group();
    let now = op("status")["tick"].as_u64().unwrap_or(0);
    assert!(now + 60 * SECOND >= next && now < next, "the save point is not a game minute before the next group may come: tick {now}, next {next}");
    assert!(since(after_first).is_empty(), "a second group came before an hour after the first: {:?}", since(after_first));
}
