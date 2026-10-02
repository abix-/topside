//! The running game's control plane, for every test: one op, its result.

use serde_json::{Value as Json, json};

pub const URL: &str = "http://127.0.0.1:15703/topside";

/// The `result` of op `name` with `args`; a test fails when no game
/// answers, or with the op's own error when the op answers one.
pub fn op_with(name: &str, args: Json) -> Json {
    let answer: Option<Json> = ureq::post(URL)
        .send_json(json!({"op": name, "args": args}))
        .ok()
        .and_then(|r| r.into_body().read_json().ok());
    let answer = answer.unwrap_or_else(|| panic!("no topside game answering on {URL}; start it with scripts/build.ps1 run"));
    if answer["ok"] == false {
        panic!("the {name} op failed: {}", answer["error"]);
    }
    answer["result"].clone()
}

/// The `result` of op `name`.
pub fn op(name: &str) -> Json {
    op_with(name, json!({}))
}

/// Start a test from its own save (todo 9k, authority.md "Running
/// tests"): `saves/<name>.save` loaded; if there is none, or the game
/// refuses it (made by another version of the game), the start is reached
/// once by playing (`reach`) and saved under that name for the next run.
/// Answers whether it played (true) or loaded (false).
pub fn start_from(name: &str, reach: impl FnOnce()) -> bool {
    let answer: Option<Json> = ureq::post(URL)
        .send_json(json!({"op": "load", "args": {"name": name}}))
        .ok()
        .and_then(|r| r.into_body().read_json().ok());
    let answer = answer.unwrap_or_else(|| panic!("no topside game answering on {URL}; start it with scripts/build.ps1 run"));
    if answer["ok"] == true {
        return false;
    }
    eprintln!("{name}: not loaded ({}); reaching the start by playing and saving it", answer["error"]);
    reach();
    let saved = op_with("save", json!({"name": name}));
    assert!(saved["tick"].is_u64(), "{name}: the start was reached but not saved: {saved}");
    true
}

/// Load save `name` (authority.md "Running tests": a test loads a save made
/// just before its event). A save the play-through makes (tests/saves.rs)
/// is never made here by playing: missing or refused, the test fails at
/// once, saying how to make it.
pub fn from_save(name: &str) {
    let answer: Option<Json> = ureq::post(URL)
        .send_json(json!({"op": "load", "args": {"name": name}}))
        .ok()
        .and_then(|r| r.into_body().read_json().ok());
    let answer = answer.unwrap_or_else(|| panic!("no topside game answering on {URL}; start it with scripts/build.ps1 run"));
    assert_eq!(answer["ok"], true, "save {name} not loaded ({}): make it with `pwsh -NoProfile -File scripts/build.ps1 saves`", answer["error"]);
}

/// For the play-through: play a game minute at a time, saving before each,
/// until `happened`; then the last save before it is kept as `name`, the
/// moment just before the event. At most `minutes` game minutes.
pub fn save_just_before(name: &str, minutes: u64, happened: impl Fn() -> bool) {
    // The rolling save, overwritten each minute; never committed.
    let held = "play_through_minute".to_string();
    for _ in 0..minutes {
        op_with("save", json!({"name": held}));
        step(60 * 60);
        if happened() {
            op_with("load", json!({"name": held}));
            op_with("save", json!({"name": name}));
            return;
        }
    }
    panic!("{name}: the event never happened within {minutes} game minutes");
}

/// The game paused for a test (the `pause` op), and let run again when the
/// test ends, passed or failed.
pub struct Paused;

impl Drop for Paused {
    fn drop(&mut self) {
        op_with("pause", json!({"on": false}));
    }
}

/// Pause the game until the answer drops.
pub fn pause() -> Paused {
    let paused = op_with("pause", json!({"on": true}));
    assert_eq!(paused["paused"], true, "the game did not pause: {paused}");
    Paused
}

/// Play `ticks` game ticks of a paused game (the `step` op): the moment
/// after them.
pub fn step(ticks: u64) -> Json {
    op_with("step", json!({"ticks": ticks}))
}

/// Everyone the `people` op lists.
pub fn people() -> Vec<Json> {
    op("people").as_array().cloned().unwrap_or_default()
}

/// Why the player stopped short of `to` (x, height, z), printed.
pub fn why_stopped(to: (f64, f64, f64)) {
    let s = op("status");
    let at = (s["player"][0].as_f64().unwrap_or(0.0), s["player"][1].as_f64().unwrap_or(0.0), s["player_feet"].as_f64().unwrap_or(0.0));
    why_stopped_at("the player", &op("player"), at, to);
}

/// Why person `id` stopped short of `to` (x, height, z), printed.
pub fn why_they_stopped(id: u64, to: (f64, f64, f64)) {
    let p = op_with("npc", json!({"id": id}));
    let at = (p["at"][0].as_f64().unwrap_or(0.0), p["at"][1].as_f64().unwrap_or(0.0), p["feet"].as_f64().unwrap_or(0.0));
    why_stopped_at(&format!("person {id}"), &p, at, to);
}

/// Someone (`me`, as the player or npc op reports them) at `at` (x, z,
/// feet) stopped short of `to`: their walk on the mesh, their commands left
/// and what they do this tick, the mesh's way from where they stand, and
/// who stands near.
fn why_stopped_at(who: &str, me: &Json, at: (f64, f64, f64), to: (f64, f64, f64)) {
    let (px, pz, pf) = at;
    eprintln!("{who} at ({px:.2}, {pz:.2}), feet {pf:.2}; way {}, commands {}, acting {}, the talk box {}", me["way"], me["commands"], me["this_tick"], op("hud")["talk"]);
    let s = op("status");
    eprintln!("  the game runs at {} of real time, {} frames a second; the player looks at {}", s["speed"], s["fps"], op("hud")["prompt"]);
    op_with("nav_path", json!({"from": [px, pf, pz], "to": [to.0, to.1, to.2]}));
    std::thread::sleep(std::time::Duration::from_millis(300));
    eprintln!("the mesh's way from there: {}", op("nav_path"));
    for p in people() {
        let (x, z) = (p["at"][0].as_f64().unwrap_or(1e9), p["at"][1].as_f64().unwrap_or(1e9));
        if ((x - px).powi(2) + (z - pz).powi(2)).sqrt() < 20.0 {
            eprintln!("  near: {} ({}) at {} feet {}, doing {}", p["name"], p["id"], p["at"], p["feet"], p["doing"]);
        }
    }
    // The doors near (doors are left out of the mesh; an open one stands
    // out from its doorway).
    for d in op("world")["bunker_doors"].as_array().cloned().unwrap_or_default() {
        let (x, z) = (d["at"][0].as_f64().unwrap_or(1e9), d["at"][1].as_f64().unwrap_or(1e9));
        if ((x - px).powi(2) + (z - pz).powi(2)).sqrt() < 4.0 && (d["floor"].as_f64().unwrap_or(1e9) - pf).abs() < 1.5 {
            eprintln!("  door: {d}");
        }
    }
}

/// The player alive: if dead, Enter, as the player respawns (the `key`
/// op), at most 3 real seconds; who killed them is printed. The `player`
/// op publishes once a game second.
pub fn player_alive() {
    let p = op("player");
    if p["dead"] != true {
        return;
    }
    eprintln!("the player was dead, killed by {}; respawning", p["killed_by"]);
    if let Some(by) = p["killed_by"]["by"].as_u64() {
        let k = op_with("npc", json!({"id": by}));
        eprintln!("the killer {by}: {} of {}, at {}, doing {}, feelings {}", k["def"], k["faction"], k["at"], k["doing"], k["feelings"]);
    }
    op_with("key", json!({"key": "Enter"}));
    for _ in 0..12 {
        std::thread::sleep(std::time::Duration::from_millis(250));
        if op("player")["dead"] == false {
            return;
        }
    }
    panic!("Enter did not respawn the player");
}

/// Whether a person belongs to a bunker.
pub fn is_bunker_person(p: &Json) -> bool {
    p["faction"].as_str().is_some_and(|f| f.starts_with("bunker"))
}
