//! The small world's 20 monuments of varying sizes stand (operator,
//! 2026-09-26), read from the running game's `world` op.

use serde_json::{Value as Json, json};

const URL: &str = "http://127.0.0.1:15703/topside";
/// How many monuments the small world has (content `SMALL_SITES`).
const MONUMENTS: usize = 20;

fn world() -> Option<Json> {
    let resp = ureq::post(URL).send_json(json!({"op": "world", "args": {}})).ok()?;
    resp.into_body().read_json().ok()
}

#[test]
fn the_small_world_has_its_twenty_monuments() {
    let world = world().unwrap_or_else(|| panic!("no topside game answering on {URL}; start it with scripts/build.ps1 run"));
    let standing = world["result"]["monuments"].as_array().cloned().unwrap_or_default();
    let lines: Vec<String> = standing
        .iter()
        .map(|m| format!("{} at {}: {} buildings", m["kind"].as_str().unwrap_or("?"), m["at"], m["buildings"]))
        .collect();
    eprintln!("world op, tick {}: {} monuments\n{}", world["result"]["tick"], standing.len(), lines.join("\n"));
    assert_eq!(standing.len(), MONUMENTS, "the small world's monuments standing:\n{}", lines.join("\n"));
}
