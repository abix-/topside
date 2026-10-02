//! The bunker you start in (design.md, todo 13a and 15a): underground,
//! the entrance on the ground and two levels below it, read from the
//! running game.

use crate::endpoint::{op, op_with};

fn status() -> (f64, f64, f64) {
    let s = op("status");
    (s["player"][0].as_f64().unwrap_or(0.0), s["player"][1].as_f64().unwrap_or(0.0), s["player_feet"].as_f64().unwrap_or(0.0))
}

/// Up to `secs` real seconds for `done`, a quarter second apart.
fn within(secs: u64, done: impl Fn() -> bool) -> bool {
    for _ in 0..secs * 4 {
        if done() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    done()
}

/// The player wakes in their room, the deepest level, 6 m down, where the
/// note lies; walked to it (a command) and E pressed, it is read there:
/// things reached at their height (authority.md "Presentation").
#[test]
fn the_player_wakes_in_their_room_and_reads_the_note_there() {
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    op_with("kill", serde_json::json!({"id": player}));
    within(3, || op("player")["dead"] == true);
    crate::endpoint::player_alive();
    let woke = within(3, || (status().2 + 6.0).abs() < 0.3);
    let (x, y, feet) = status();
    eprintln!("woke at ({x:.1}, {y:.1}), feet {feet:.2}");
    assert!(woke, "the player woke with their feet at {feet:.2}, not 6 m down");
    // The note, from the bunker point the player wakes at.
    let note = (x - 1.4, y + 3.6);
    op_with("command", serde_json::json!({"do": "move_to", "x": note.0, "y": note.1}));
    within(10, || {
        let (px, py, _) = status();
        ((px - note.0).powi(2) + (py - note.1).powi(2)).sqrt() < 1.6
    });
    op_with("key", serde_json::json!({"key": "E"}));
    let read = within(3, || op("hud")["reading"] == true);
    crate::talk_ui::shot("bunker_note");
    let (px, py, pf) = status();
    eprintln!("by the note at ({px:.1}, {py:.1}), feet {pf:.2}; reading: {read}");
    op_with("key", serde_json::json!({"key": "Escape"}));
    assert!(read, "E by the note in the player's room did not open it");
}

/// The stairwell (design.md "The bunker you start in"), from the
/// entrance's door landing facing down the stairs, in
/// logs/bunker_stairwell.png. Stood there by the `use` op, then turned by
/// a short move: commands walk the ground's level only until todo 13a2.
#[test]
fn the_stairwell_seen_from_the_entrance_landing() {
    crate::endpoint::player_alive();
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    // The bunker point (0, 0): a point in the stairwell's west lane, north
    // of the door landing; the `use` op stands the player a metre off it,
    // looking at it.
    let landing = (4.3, -2.6);
    op_with("use", serde_json::json!({"id": player, "at": [landing.0, landing.1]}));
    let there = within(3, || {
        let (px, py, pf) = status();
        pf.abs() < 0.1 && ((px - landing.0).powi(2) + (py - landing.1).powi(2)).sqrt() < 2.0
    });
    std::thread::sleep(std::time::Duration::from_millis(500));
    crate::talk_ui::shot("bunker_stairwell");
    let (px, py, pf) = status();
    eprintln!("on the landing at ({px:.1}, {py:.1}), feet {pf:.2}");
    assert!(there && pf.abs() < 0.3, "the player never stood on the entrance's door landing");
}

/// The entrance room's one doorway into the stairwell, from inside the
/// room facing east, in logs/bunker_doorway.png.
#[test]
fn the_entrance_has_one_doorway_into_the_stairwell() {
    crate::endpoint::player_alive();
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    op_with("use", serde_json::json!({"id": player, "at": [-1.5, 0.0]}));
    within(3, || status().2.abs() < 0.1 && status().0 < 1.0);
    op_with("command", serde_json::json!({"do": "move_to", "x": 2.0, "y": 0.0}));
    let there = within(5, || status().0 > 0.3);
    std::thread::sleep(std::time::Duration::from_millis(500));
    crate::talk_ui::shot("bunker_doorway");
    let (px, py, pf) = status();
    eprintln!("in the entrance at ({px:.1}, {py:.1}), feet {pf:.2}");
    assert!(there && pf.abs() < 0.3, "the player never stood in the entrance facing the stairwell");
}

/// The basic controls in the player's room (design.md "The bunker you
/// start in"): each shown until done; E pressed, its line goes and the
/// rest stay.
#[test]
fn the_controls_show_in_the_room_and_each_goes_once_done() {
    crate::endpoint::player_alive();
    let feet = status().2;
    let shown = op("hud")["hints"].as_array().cloned().unwrap_or_default();
    eprintln!("feet {feet:.2}; the controls shown: {shown:?}");
    if feet > -0.5 {
        assert!(shown.is_empty(), "controls shown topside: {shown:?}");
        eprintln!("the player is topside: none shown, as it should be; the rest not played this run");
        return;
    }
    op_with("key", serde_json::json!({"key": "E"}));
    let gone = within(3, || op("hud")["hints"].as_array().is_some_and(|h| !h.iter().any(|t| t == "E  use")));
    let after = op("hud")["hints"].clone();
    crate::talk_ui::shot("bunker_controls");
    eprintln!("after E: {after}");
    assert!(gone, "E pressed and its line still shown: {after}");
}

/// The player stood in the entrance by the stair tower, east of the room,
/// looking at it: the way down seen, in logs/bunker_stairs.png.
#[test]
fn the_way_down_is_seen_from_the_entrance() {
    crate::endpoint::player_alive();
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    op_with("use", serde_json::json!({"id": player, "at": [4.5, 0.0]}));
    std::thread::sleep(std::time::Duration::from_millis(500));
    crate::talk_ui::shot("bunker_stairs");
    let s = op("status");
    eprintln!("the player at {} feet {}", s["player"], s["player_feet"]);
    assert!(s["player"][0].as_f64().is_some_and(|x| x.abs() < 8.0), "the player is not in the bunker");
}

#[test]
fn every_bunker_goes_two_levels_below_the_ground_open_over_the_way_down() {
    let w = op("world");
    let below = w["below"].as_array().cloned().unwrap_or_default();
    eprintln!("buildings that go below the ground: {}", serde_json::Value::Array(below.clone()));
    let bunkers = w["bunkers"].as_array().map_or(0, |b| b.len()).max(1);
    let deep = below.iter().filter(|b| b["bottom"].as_f64().is_some_and(|y| (y + 6.0).abs() < 0.01)).count();
    assert!(deep >= bunkers, "{deep} buildings go two levels (6 m) down; there are {bunkers} bunkers");
    assert!(
        below.iter().all(|b| b["open"].as_u64().is_some_and(|n| n > 0) && b["tiles"].as_u64().is_some_and(|n| n > 0)),
        "a building below the ground with no tiles below, or no ground open over its way down"
    );
}
