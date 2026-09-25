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

/// The frame budget, readable from outside: every diagnostic Bevy
/// records, printed sorted by value so the heaviest render passes
/// read first. Run with --nocapture to see them.
#[test]
fn diagnostics_op_reports_frame_time_and_render_passes() {
    let Some(resp) = ureq::post("http://127.0.0.1:15703/topside")
        .send_json(json!({"op": "diagnostics", "args": {}}))
        .ok()
        .and_then(|r| r.into_body().read_json::<Json>().ok())
    else {
        eprintln!("skipping: no topside game listening on 127.0.0.1:15703");
        return;
    };
    assert_eq!(resp["ok"], true, "diagnostics op must succeed: {resp}");
    let Some(all) = resp["result"].as_object() else {
        panic!("diagnostics must be a map of path to value: {resp}");
    };
    assert!(all.contains_key("fps"), "fps is recorded: {resp}");
    assert!(all.contains_key("entity_count"), "entity count is recorded: {resp}");
    let mut rows: Vec<(&String, f64)> = all
        .iter()
        .filter_map(|(k, v)| v.as_f64().map(|v| (k, v)))
        .collect();
    rows.sort_by(|a, b| b.1.total_cmp(&a.1));
    eprintln!("--- diagnostics ({} entries) ---", rows.len());
    for (path, value) in rows {
        eprintln!("{value:>12.3}  {path}");
    }
}

fn people() -> Option<Vec<Json>> {
    let resp: Json = ureq::post("http://127.0.0.1:15703/topside")
        .send_json(json!({"op": "people", "args": {}}))
        .ok()?
        .into_body()
        .read_json()
        .ok()?;
    assert_eq!(resp["ok"], true, "people op must succeed: {resp}");
    resp["result"].as_array().cloned()
}

/// The people of the running world, as they are: who, where, how,
/// and what they did this tick. Asserts what is built (they exist
/// with factions, traits, and draining needs) and prints the rest so
/// the live game, not a feeling, says whether they live. Run with
/// --nocapture to see them.
#[test]
fn people_op_reports_every_npc_with_needs_that_drain() {
    let Some(first) = people() else {
        eprintln!("skipping: no topside game listening on 127.0.0.1:15703");
        return;
    };
    assert!(!first.is_empty(), "the world has people");
    for p in &first {
        assert!(p["id"].is_u64(), "{p}");
        assert!(p["def"].is_string(), "{p}");
        assert!(p["health"].as_f64().unwrap() <= p["max_health"].as_f64().unwrap());
        for need in ["hunger", "thirst", "rest", "safety"] {
            let v = p[need].as_f64().unwrap_or(-1.0);
            assert!((0.0..=100.0).contains(&v), "{need} of {p}");
        }
        assert_eq!(p["axes"].as_array().map(|a| a.len()), Some(7), "{p}");
    }
    let with_traits = first
        .iter()
        .filter(|p| p["axes"].as_array().unwrap().iter().any(|v| v.as_f64() != Some(0.0)))
        .count();
    assert!(with_traits > 0, "some people have traits");

    // Two seconds later: needs have fallen on everyone (they drain
    // every tick, asleep or not).
    std::thread::sleep(std::time::Duration::from_millis(2200));
    let second = people().expect("answered once, must answer again");
    let by_id = |list: &[Json], id: u64| list.iter().find(|p| p["id"] == id).cloned();
    let mut compared = 0;
    for p in &first {
        let id = p["id"].as_u64().unwrap();
        let Some(later) = by_id(&second, id) else { continue };
        assert!(later["hunger"].as_f64() <= p["hunger"].as_f64(), "hunger never rises alone");
        assert!(later["rest"].as_f64() <= p["rest"].as_f64(), "rest falls awake: {p} -> {later}");
        compared += 1;
    }
    assert!(compared > 0);

    // What they are doing, reported honestly: today the only thing
    // that moves a person is a hostile in sight.
    let acting: Vec<&Json> = second.iter().filter(|p| p["acting"] == true).collect();
    let moved = first
        .iter()
        .filter(|p| {
            by_id(&second, p["id"].as_u64().unwrap())
                .map(|l| l["at"] != p["at"])
                .unwrap_or(false)
        })
        .count();
    eprintln!(
        "--- people: {} total, {} with traits, {} acting this tick, {} moved in 2 s ---",
        second.len(),
        with_traits,
        acting.len(),
        moved
    );
    for p in second.iter().take(12) {
        eprintln!(
            "{:>4} {:<10} {:<9} hp {:>5.1}/{:<5.1} hunger {:>5.1} rest {:>5.1} safety {:>5.1} {}",
            p["id"],
            p["def"].as_str().unwrap_or(""),
            p["faction"].as_str().unwrap_or("neutral"),
            p["health"].as_f64().unwrap(),
            p["max_health"].as_f64().unwrap(),
            p["hunger"].as_f64().unwrap(),
            p["rest"].as_f64().unwrap(),
            p["safety"].as_f64().unwrap(),
            p["actions"]
        );
    }
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
