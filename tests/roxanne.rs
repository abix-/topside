//! Roxanne, the one person in the player's bunker besides them, in the
//! parts The Tap's opening is made of, each tested alone over the
//! `command` op (design.md "The bunker you start in"): she walks the
//! bunker's levels, she knocks on the player's door, she leads the player.

use std::time::Duration;

use serde_json::{Value as Json, json};

use crate::endpoint::{op, op_with, people};

/// Up to `secs` real seconds for `done`, a quarter second apart.
fn within(secs: u64, done: impl Fn() -> bool) -> bool {
    for _ in 0..secs * 4 {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    done()
}

/// Roxanne as the `people` op reports her.
fn roxanne() -> Json {
    people().into_iter().find(|p| p["name"] == "Roxanne" && p["dead"] != true).expect("Roxanne, alive, in the people op")
}

/// Where a person stands: the ground point and their feet's height.
fn spot(p: &Json) -> (f64, f64, f64) {
    (p["at"][0].as_f64().unwrap_or(f64::NAN), p["at"][1].as_f64().unwrap_or(f64::NAN), p["feet"].as_f64().unwrap_or(f64::NAN))
}

/// Whether person `id` comes within 1.5 m of `to` on its level within
/// `secs` real seconds, and the nearest they came. Read from the `npc` op,
/// answered live: a command ends on arrival and the person's brain walks
/// them on at once, faster than the once-a-second `people` op shows.
fn comes_to(id: u64, to: (f64, f64, f64), secs: u64) -> (bool, f64) {
    let start = std::time::Instant::now();
    let mut nearest = f64::INFINITY;
    while start.elapsed() < Duration::from_secs(secs) {
        let p = op_with("npc", json!({"id": id}));
        let (x, z, feet) = (p["at"][0].as_f64().unwrap_or(f64::NAN), p["at"][1].as_f64().unwrap_or(f64::NAN), p["feet"].as_f64().unwrap_or(f64::NAN));
        if (feet - to.2).abs() < 0.5 {
            nearest = nearest.min(((x - to.0).powi(2) + (z - to.1).powi(2)).sqrt());
        }
        if nearest <= 1.5 {
            return (true, nearest);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    (false, nearest)
}

/// The player in their room, woken there by dying (the respawn), its door
/// shut first: the bunker's door on the deepest level nearest where the
/// player wakes, used (the `use` op) if it stands open. Returns where the
/// player wakes.
fn the_player_shut_in_their_room() -> (f64, f64, f64) {
    crate::endpoint::player_alive();
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    let wake = |_: ()| {
        op_with("kill", json!({"id": player}));
        within(3, || op("player")["dead"] == true);
        crate::endpoint::player_alive();
        within(3, || (op("status")["player_feet"].as_f64().unwrap_or(0.0) + 6.0).abs() < 0.3);
        let s = op("status");
        (s["player"][0].as_f64().unwrap_or(0.0), s["player"][1].as_f64().unwrap_or(0.0), s["player_feet"].as_f64().unwrap_or(0.0))
    };
    let room = wake(());
    let doors = op("world")["bunker_doors"].as_array().cloned().unwrap_or_default();
    let door = doors
        .iter()
        .filter(|d| d["floor"].as_f64().is_some_and(|f| (f - room.2).abs() < 0.5))
        .min_by(|a, b| {
            let far = |d: &Json| ((d["at"][0].as_f64().unwrap_or(0.0) - room.0).powi(2) + (d["at"][1].as_f64().unwrap_or(0.0) - room.1).powi(2)).sqrt();
            far(a).total_cmp(&far(b))
        })
        .cloned()
        .expect("a door on the player's level");
    eprintln!("the player's room door: {door}");
    if door["open"] == true {
        op_with("use", json!({"id": player, "at": door["at"], "height": door["floor"]}));
        std::thread::sleep(Duration::from_millis(1500));
        let now = op("world")["bunker_doors"].as_array().and_then(|d| d.iter().find(|d| d["at"] == door["at"] && d["floor"] == door["floor"]).cloned());
        eprintln!("used it: now {now:?}");
        return wake(());
    }
    room
}

/// Roxanne knocks on the player's shut door: the player hears her knock
/// and is in a conversation with her, the talk box open, through the door.
#[test]
fn knocks_on_the_players_door_and_the_talk_box_opens() {
    let room = the_player_shut_in_their_room();
    let her = roxanne();
    let id = her["id"].as_u64().expect("Roxanne's id");
    let heard_before = op("player")["heard_knocking"].as_array().map_or(0, |h| h.len());
    eprintln!("Roxanne ({id}) at {:?}; the player in their room at {room:?}, heard {heard_before} knocks before", spot(&her));
    op_with("command", json!({"id": id, "do": "knock", "who": op("player")["id"]}));
    let heard = within(30, || {
        op("player")["heard_knocking"].as_array().is_some_and(|h| h.len() > heard_before && h.last().is_some_and(|k| k["by"].as_u64() == Some(id)))
    });
    let opened = within(5, || op("hud")["talk"]["to"].as_u64() == Some(id));
    crate::talk_ui::shot("roxanne_knocks");
    let talk = op("hud")["talk"].clone();
    eprintln!("heard: {heard}, Roxanne at {:?}, the talk box: {talk}", spot(&roxanne()));
    if !heard {
        let her = op_with("npc", json!({"id": id}));
        let did: Vec<Json> = her["did"].as_array().map(|d| d.iter().rev().take(5).cloned().collect()).unwrap_or_default();
        eprintln!("her way: {}, acting: {}, last did: {did:?}", her["way"], her["this_tick"]);
    }
    op_with("key", json!({"key": "Escape"}));
    assert!(heard, "the player never heard Roxanne knock");
    assert!(opened, "the talk box never opened with Roxanne after her knock: {talk}");
}

/// Roxanne walks from where she is down to the player's room, 6 m down
/// through its shut door, and back up to where she stood, each by a
/// command on the navigation mesh. The player is stood topside first, so
/// no one is in her way.
#[test]
fn walks_down_to_the_players_room_and_back() {
    crate::endpoint::player_alive();
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    // Where the player wakes: in their room.
    op_with("kill", json!({"id": player}));
    within(3, || op("player")["dead"] == true);
    crate::endpoint::player_alive();
    within(3, || (op("status")["player_feet"].as_f64().unwrap_or(0.0) + 6.0).abs() < 0.3);
    let s = op("status");
    let room = (s["player"][0].as_f64().unwrap_or(0.0), s["player"][1].as_f64().unwrap_or(0.0), s["player_feet"].as_f64().unwrap_or(0.0));
    // Out of her way: topside, south of the front door.
    op_with("use", json!({"id": player, "at": [0.0, 12.0]}));
    within(3, || op("status")["player_feet"].as_f64().is_some_and(|f| f.abs() < 0.1));

    let her = roxanne();
    let id = her["id"].as_u64().expect("Roxanne's id");
    let from = spot(&op_with("npc", json!({"id": id})));
    eprintln!("Roxanne ({id}) at {from:?}; the player's room at {room:?}");
    op_with("command", json!({"id": id, "do": "move_to", "x": room.0, "y": room.2, "z": room.1}));
    let (down, nearest) = comes_to(id, room, 30);
    crate::talk_ui::shot("roxanne_down");
    eprintln!("down: {down}, nearest {nearest:.2} m; Roxanne now at {:?}, doing {}", spot(&roxanne()), roxanne()["doing"]);
    if !down {
        let her = op_with("npc", json!({"id": id}));
        eprintln!("her way: {}, acting: {}", her["way"], her["this_tick"]);
    }
    assert!(down, "Roxanne never reached the player's room at {room:?}");

    op_with("command", json!({"id": id, "do": "move_to", "x": from.0, "y": from.2, "z": from.1}));
    let (back, nearest) = comes_to(id, from, 30);
    eprintln!("back: {back}, nearest {nearest:.2} m; Roxanne now at {:?}", spot(&roxanne()));
    if !back {
        let her = op_with("npc", json!({"id": id}));
        eprintln!("her way: {}, acting: {}", her["way"], her["this_tick"]);
    }
    assert!(back, "Roxanne never walked back to {from:?}");
}
