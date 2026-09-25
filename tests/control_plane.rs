//! Drives the control plane of a RUNNING topside game. Skips cleanly
//! when no game is up.

use serde_json::{Value as Json, json};

fn status() -> Option<Json> {
    let resp = ureq::post("http://127.0.0.1:15703/topside")
        .send_json(json!({"op": "status", "args": {}}))
        .ok()?;
    resp.into_body().read_json().ok()
}

#[test]
fn status_op_reports_a_ticking_simulation() {
    let Some(first) = status() else {
        eprintln!("skipping: no topside game listening on 127.0.0.1:15703");
        return;
    };
    assert_eq!(first["ok"], true, "status op must succeed: {first}");
    let first_tick = first["state"]["tick"].as_u64().expect("state.tick is a number");
    assert!(first_tick > 0, "the simulation has ticked");
    assert!(first["state"]["player"].is_array(), "the player's position: {first}");
    assert!(first["state"]["hunger"].is_number() && first["state"]["thirst"].is_number());
    std::thread::sleep(std::time::Duration::from_millis(300));
    let second = status().expect("the game answered once, it answers again");
    let second_tick = second["state"]["tick"].as_u64().unwrap();
    assert!(second_tick > first_tick, "the simulation advances ({first_tick} -> {second_tick})");
}
