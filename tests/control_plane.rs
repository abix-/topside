//! Drives the control plane of a RUNNING topside game. Skips
//! cleanly when no game is up, so `cargo test` stays green in CI.

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
    assert_eq!(first["op"], "status");
    let first_tick = first["state"]["tick"].as_u64().expect("state.tick is a number");
    assert!(first_tick > 0, "simulation must have ticked at least once");
    assert!(
        first["state"]["hunger"].is_number() && first["state"]["thirst"].is_number(),
        "state must carry the player's survival stats: {first}"
    );

    std::thread::sleep(std::time::Duration::from_millis(300));
    let second = status().expect("game answered once, must answer again");
    let second_tick = second["state"]["tick"].as_u64().expect("state.tick is a number");
    assert!(
        second_tick > first_tick,
        "simulation must advance between queries ({first_tick} -> {second_tick})"
    );
}

#[test]
fn unknown_op_is_rejected_with_an_error() {
    let Some(resp) = ureq::post("http://127.0.0.1:15703/topside")
        .send_json(json!({"op": "no-such-op", "args": {}}))
        .ok()
        .and_then(|r| r.into_body().read_json::<Json>().ok())
    else {
        eprintln!("skipping: no topside game listening on 127.0.0.1:15703");
        return;
    };
    assert_eq!(resp["ok"], false);
    assert!(
        resp["error"].as_str().unwrap_or_default().contains("unknown op"),
        "error must name the unknown op: {resp}"
    );
}
