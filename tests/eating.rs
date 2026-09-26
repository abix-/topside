//! NPCs eat, read from each person's memory in the running game's
//! `people` op.

use serde_json::{Value as Json, json};

const URL: &str = "http://127.0.0.1:15703/topside";

fn people() -> Option<Json> {
    let resp = ureq::post(URL).send_json(json!({"op": "people", "args": {}})).ok()?;
    resp.into_body().read_json().ok()
}

#[test]
fn npcs_eat() {
    let people = people().unwrap_or_else(|| panic!("no topside game answering on {URL}; start it with scripts/build.ps1 run"));
    let list = people["result"].as_array().cloned().unwrap_or_default();
    let ate: Vec<String> = list
        .iter()
        .filter_map(|p| {
            let n = p["did"]["ate"].as_u64().unwrap_or(0);
            (n > 0).then(|| format!("{} {} ({}): {n}", p["id"], p["def"].as_str().unwrap_or("?"), p["faction"].as_str().unwrap_or("-")))
        })
        .collect();
    let hungriest = list.iter().filter_map(|p| p["hunger"].as_f64()).fold(f64::MAX, f64::min);
    let tick = ureq::post(URL)
        .send_json(json!({"op": "status", "args": {}}))
        .ok()
        .and_then(|r| r.into_body().read_json::<Json>().ok())
        .map(|s| s["state"]["tick"].clone())
        .unwrap_or(Json::Null);
    eprintln!(
        "people op at tick {tick}: {} people, {} ate, the hungriest at {hungriest:.0} hunger\n{}",
        list.len(),
        ate.len(),
        ate.join("\n")
    );
    assert!(!ate.is_empty(), "no NPC in the running game has eaten ({} people)", list.len());
}
