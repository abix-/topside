//! The running game's control plane, for every test: one op, its result.

use serde_json::{Value as Json, json};

pub const URL: &str = "http://127.0.0.1:15703/topside";

/// The `result` of op `name` with `args`; a test fails when no game answers.
pub fn op_with(name: &str, args: Json) -> Json {
    let answer: Option<Json> = ureq::post(URL)
        .send_json(json!({"op": name, "args": args}))
        .ok()
        .and_then(|r| r.into_body().read_json().ok());
    let answer = answer.unwrap_or_else(|| panic!("no topside game answering on {URL}; start it with scripts/build.ps1 run"));
    answer["result"].clone()
}

/// The `result` of op `name`.
pub fn op(name: &str) -> Json {
    op_with(name, json!({}))
}

/// Everyone the `people` op lists.
pub fn people() -> Vec<Json> {
    op("people").as_array().cloned().unwrap_or_default()
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
