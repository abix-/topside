//! The control plane of the running topside game: `status` answers and
//! the game is ticking, by the speed it reports itself.

use serde_json::{Value as Json, json};

use crate::endpoint::url;

fn status() -> Option<Json> {
    let resp = ureq::post(url()).send_json(json!({"op": "status", "args": {}})).ok()?;
    resp.into_body().read_json().ok()
}

#[test]
fn status_op_reports_a_ticking_simulation() {
    crate::opening::you_wake();
    let status = status().unwrap_or_else(|| panic!("no topside game answering on {}; start it with scripts/build.ps1 run", url()));
    assert_eq!(status["ok"], true, "status op must succeed: {status}");
    let state = &status["state"];
    assert!(state["tick"].as_u64().is_some_and(|t| t > 0), "the simulation has ticked: {status}");
    assert!(state["player"].is_array(), "the player's position: {status}");
    assert!(state["hunger"].is_number() && state["thirst"].is_number(), "hunger and thirst: {status}");
    let speed = state["speed"].as_f64().unwrap_or(0.0);
    assert!(speed > 0.0, "the game is ticking (speed {speed}): {status}");
}
