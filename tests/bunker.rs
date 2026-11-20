//! The bunker you start in (design.md "The bunker you start in"): two
//! rooms, the surface room and the player's room below it, joined by
//! stairs under a hatch, read from the running game.

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

/// The bunker is two rooms and nothing else (operator, 2026-10-03): the
/// surface room on the ground with the door to topside, the water store,
/// the bench and Roxanne; the player's room below it with the player, the
/// note and the food; the hatch lying shut in the surface room's floor,
/// over the room below. Read from the world op just after waking.
/// Checked in the opening's one play as the player wakes
/// (opening::play_to_pivot_1).
pub(crate) fn the_bunker_is_two_rooms_the_surface_room_and_the_players_room_below() {
    let w = op("world");
    let rooms = w["starting_bunker"].as_array().cloned().unwrap_or_default();
    eprintln!("just woke at tick {}; the starting bunker: {}", w["tick"], serde_json::Value::Array(rooms.clone()));
    // Who of the player's bunker is where, and what they did (who went
    // through the front door).
    for p in crate::endpoint::people().into_iter().filter(|p| p["faction"] == "player") {
        let id = p["id"].as_u64().unwrap_or(0);
        let them = op_with("npc", serde_json::json!({"id": id}));
        eprintln!("{} ({id}) at {} feet {} doing {} about {}; did {}", p["name"], them["at"], them["feet"], them["doing"], them["about"], them["did"]);
        // What their memory holds of the things it knows: seen and checked
        // when, and who told them.
        eprintln!("  knows {}", them["knows"]);
    }
    assert_eq!(rooms.len(), 2, "the starting bunker is not two rooms");
    let surface = crate::opening::bunker_room("surface");
    let below = crate::opening::bunker_room("below");
    assert!(surface["level"].as_f64().is_some_and(|l| l.abs() < 0.01), "the surface room is not on the ground: {surface}");
    let (up, down) = (surface["level"].as_f64().unwrap_or(0.0), below["level"].as_f64().unwrap_or(0.0));
    assert!((up - down - 3.0).abs() < 0.01, "the player's room is not one level (3 m) below the surface room: {up} and {down}");
    let holds = |room: &serde_json::Value, key: &str, what: &str| room[key].as_array().is_some_and(|t| t.iter().any(|t| t == what));
    assert!(holds(&surface, "things", "storage box"), "no water store in the surface room: {surface}");
    assert!(holds(&surface, "things", "Station(CraftingBench)"), "no bench in the surface room: {surface}");
    assert!(holds(&surface, "people", "Roxanne"), "Roxanne does not live in the surface room: {surface}");
    let doors = |room: &serde_json::Value, hatch: bool| room["doors"].as_array().map_or(0, |d| d.iter().filter(|d| (d["hatch"] == true) == hatch).count());
    assert_eq!(doors(&surface, false), 1, "the surface room has not the one door to topside: {surface}");
    assert_eq!(doors(&surface, true), 1, "no hatch lies in the surface room's floor: {surface}");
    let hatch = surface["doors"].as_array().and_then(|d| d.iter().find(|d| d["hatch"] == true).cloned()).unwrap_or_default();
    assert!(hatch["open"] == false, "the player's hatch is not shut: {hatch}");
    let over_below = crate::opening::in_room(&below, (hatch["at"][0].as_f64().unwrap_or(f64::NAN), hatch["at"][1].as_f64().unwrap_or(f64::NAN), below["floor"].as_f64().unwrap_or(0.0)));
    assert!(over_below, "the hatch {hatch} does not lie over the player's room {below}");
    assert!(holds(&below, "people", "the player"), "the player did not wake in the room below: {below}");
    assert!(holds(&below, "things", "note"), "no note in the player's room: {below}");
    assert!(holds(&below, "things", "canned food"), "no food in the player's room: {below}");
    assert_eq!(doors(&below, false) + doors(&below, true), 0, "the player's room has a door of its own besides the hatch: {below}");
    // Roxanne found the tap, so she has been there: her memory holds it
    // checked, or her brain walks her out to look at it (episode.rs
    // place_the_tap; the operator's playtest of 2026-10-03).
    let tap = op("world")["things"].as_array().and_then(|t| t.iter().find(|t| t["name"] == "tap").cloned()).expect("The Tap's tap in the world");
    let her = crate::opening::roxanne();
    let knows = op_with("npc", serde_json::json!({"id": her}))["knows"].clone();
    let of_tap = knows.as_array().and_then(|k| k.iter().find(|k| k["key"] == tap["key"]).cloned()).expect("Roxanne knows the tap");
    assert!(of_tap["checked_at"].is_u64(), "Roxanne's memory of the tap is not checked, so her brain will go to look at it: {of_tap}");
}

/// The player wakes in their room, the room below the surface room, 3 m
/// down, where the note lies; walked to it (a command) and E pressed, it
/// is read there: things reached at their height (authority.md
/// "Presentation").
#[test]
fn the_player_wakes_in_their_room_and_reads_the_note_there() {
    crate::opening::you_wake();
    let woke = within(3, || (status().2 + 3.0).abs() < 0.3);
    let (x, y, feet) = status();
    eprintln!("woke at ({x:.1}, {y:.1}), feet {feet:.2}");
    assert!(woke, "the player woke with their feet at {feet:.2}, not 3 m down");
    // The note, from the bunker point the player wakes at.
    let note = (x - 1.4, y + 3.6);
    op_with("command", serde_json::json!({"do": "move_to", "x": note.0, "y": feet, "z": note.1}));
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

/// The bunker's front door, from inside the entrance: closed, then used,
/// turned a quarter turn on its hinge at one end of the doorway; in
/// logs/bunker_door_closed.png and logs/bunker_door_open.png.
#[test]
fn the_front_door_turns_on_its_hinge() {
    crate::opening::you_wake();
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    // The door block's middle, its 2 tiles in the south wall.
    let door = (0.0, 4.5);
    op_with("use", serde_json::json!({"id": player, "at": [door.0, door.1 - 2.5]}));
    within(3, || status().2.abs() < 0.1 && status().1 < 3.0);
    std::thread::sleep(std::time::Duration::from_millis(400));
    crate::talk_ui::shot("bunker_door_closed");
    op_with("use", serde_json::json!({"id": player, "at": [door.0, door.1]}));
    std::thread::sleep(std::time::Duration::from_millis(600));
    crate::talk_ui::shot("bunker_door_open");
    let (px, py, pf) = status();
    eprintln!("by the front door at ({px:.1}, {py:.1}), feet {pf:.2}");
    assert!(pf.abs() < 0.3 && (py - door.1).abs() < 2.0, "the player was not by the front door");
}

/// The basic controls in the player's room (design.md "The bunker you
/// start in"): each shown until done; E pressed, its line goes and the
/// rest stay.
#[test]
fn the_controls_show_in_the_room_and_each_goes_once_done() {
    // Paused, so the E press is taken by the one tick stepped and the hud's
    // copy read is published at that stop.
    let _paused = crate::endpoint::pause();
    crate::opening::you_wake();
    let feet = status().2;
    let shown = op("hud")["hints"].as_array().cloned().unwrap_or_default();
    eprintln!("feet {feet:.2}; the controls shown: {shown:?}");
    assert!(shown.iter().any(|t| t == "E  use"), "waking in their room, the controls are not shown: {shown:?}");
    op_with("key", serde_json::json!({"key": "E"}));
    crate::endpoint::step(1);
    let after = op("hud")["hints"].clone();
    crate::talk_ui::shot("bunker_controls");
    eprintln!("after E: {after}; the player did {}", op("player")["did"]);
    let gone = after.as_array().is_some_and(|h| !h.iter().any(|t| t == "E  use"));
    assert!(gone, "E pressed and its line still shown: {after}");
}

/// No ground is drawn within 5 cm of a floor tile, step or landing (the
/// operator's playtest of 2026-10-03: the ground showed through the floors
/// on the stairs), read from what the game draws (the `ground_on_floors`
/// op), in the player's bunker, from pivot point 1's save.
#[test]
fn no_ground_shows_through_a_floor_or_a_step() {
    let _paused = crate::endpoint::pause();
    crate::opening::choosing();
    let seen = op("ground_on_floors");
    let near = seen["near_the_ground"].as_u64().unwrap_or_else(|| panic!("the ground_on_floors op answered no count: {seen}"));
    assert!(near > 0, "no floor tile, step or landing drawn on a ground floor: nothing to test ({seen})");
    assert_eq!(seen["ground_on_floors"].as_u64(), Some(0), "of {near} floor tiles, steps and landings at the ground, these have the ground drawn within 5 cm of their top: {}", seen["first"]);
}

#[test]
fn every_bunker_goes_one_level_below_the_ground_open_over_the_hatch() {
    let _paused = crate::endpoint::pause();
    crate::opening::you_wake();
    let w = op("world");
    let below = w["below"].as_array().cloned().unwrap_or_default();
    eprintln!("buildings that go below the ground: {}", serde_json::Value::Array(below.clone()));
    let bunkers = w["bunkers"].as_array().map_or(0, |b| b.len()).max(1);
    let deep = below.iter().filter(|b| b["bottom"].as_f64().is_some_and(|y| (y + 3.0).abs() < 0.01)).count();
    assert!(deep >= bunkers, "{deep} buildings go one level (3 m) down; there are {bunkers} bunkers");
    assert!(
        below.iter().all(|b| b["open"].as_u64().is_some_and(|n| n > 0) && b["tiles"].as_u64().is_some_and(|n| n > 0)),
        "a building below the ground with no tiles below, or no ground open over its way down"
    );
}
