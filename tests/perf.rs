//! Where the running game's time goes, read only (docs/performance.md):
//! the whole fixed step against its 16.7 ms budget, every part costliest
//! first, the counts per step, every span Bevy ran per frame, and Bevy's
//! own diagnostics. Each reading is added to logs/perf.txt. It changes
//! nothing in the game. A reading counts only in normal play: the test
//! fails when a test run is going, or when the game did not tick a whole
//! second's 60 ticks in the second it took (paused, or stepping).

use std::time::Duration;

use serde_json::json;

use crate::endpoint::{op, op_with};

/// The perf op publishes once every 60 ticks; past a second of play, the
/// reading read after it is one taken in play.
const ONE_PUBLISH: Duration = Duration::from_millis(1100);

#[test]
fn where_the_time_goes_in_play() {
    let before = op("status");
    std::thread::sleep(ONE_PUBLISH);
    let perf = op("perf");
    let diagnostics = op("diagnostics");
    let status = op("status");
    let run = op_with("tests", json!({}));
    let ticks = status["tick"].as_u64().unwrap_or(0).saturating_sub(before["tick"].as_u64().unwrap_or(0));
    // Another test running drives the game; this one, run by the queue,
    // is its own run's running test and reads play.
    let running = run["tests"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|t| t["state"] == "running" && t["name"] != "perf::where_the_time_goes_in_play");

    let mut rows = vec![format!(
        "reading at tick {}: {} ticks in {} ms; speed {:.2}; fps {:.1}; {} people; a test run going: {}",
        status["tick"],
        ticks,
        ONE_PUBLISH.as_millis(),
        status["speed"].as_f64().unwrap_or(0.0),
        status["fps"].as_f64().unwrap_or(0.0),
        status["people"],
        running
    )];
    let step = perf["step_ms"].as_f64().unwrap_or(0.0);
    let per_frame = perf["steps_per_frame"].as_f64().unwrap_or(0.0);
    let fixed_main = perf["frame"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|s| s["span"] == "schedule FixedMain")
        .and_then(|s| s["ms"].as_f64())
        .unwrap_or(0.0);
    let laps: f64 = perf["parts"].as_array().into_iter().flatten().filter(|p| p["in_step"] == true).filter_map(|p| p["ms"].as_f64()).sum();
    rows.push(format!(
        "whole step {step:.3} ms of {:.3} (worst {:.3}); {per_frame:.2} steps a frame; FixedMain {fixed_main:.3} ms a frame, {:.3} a step; the step's laps add up to {laps:.3}",
        perf["budget_ms"].as_f64().unwrap_or(0.0),
        perf["step_worst_ms"].as_f64().unwrap_or(0.0),
        if per_frame > 0.0 { fixed_main / per_frame } else { 0.0 }
    ));
    rows.push("parts of a step (ms, average and worst; * a lap of the step):".to_string());
    for part in perf["parts"].as_array().into_iter().flatten() {
        rows.push(format!(
            "  {} {:<46} {:>8.3} {:>9.3}",
            if part["in_step"] == true { "*" } else { " " },
            part["part"].as_str().unwrap_or("?"),
            part["ms"].as_f64().unwrap_or(0.0),
            part["worst_ms"].as_f64().unwrap_or(0.0)
        ));
    }
    rows.push("counts (a step):".to_string());
    for count in perf["counts"].as_array().into_iter().flatten() {
        rows.push(format!("  {:<48} {:>8.2}", count["what"].as_str().unwrap_or("?"), count["per_step"].as_f64().unwrap_or(0.0)));
    }
    let view = op("view");
    rows.push(format!("meshes {}, in view {}, cel shaded {}", view["meshes"], view["seen"], view["cel"]));
    rows.push("spans per frame (ms):".to_string());
    for span in perf["frame"].as_array().into_iter().flatten() {
        rows.push(format!("  {:<72} {:>8.3}", span["span"].as_str().unwrap_or("?"), span["ms"].as_f64().unwrap_or(0.0)));
    }
    rows.push("diagnostics:".to_string());
    let mut diag: Vec<(String, f64)> = diagnostics
        .as_object()
        .into_iter()
        .flatten()
        .map(|(k, v)| (k.clone(), v.as_f64().unwrap_or(0.0)))
        .collect();
    diag.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (path, value) in diag.iter().take(40) {
        rows.push(format!("  {path:<72} {value:>10.3}"));
    }
    let text = format!("{}\n\n", rows.join("\n"));
    let _ = std::fs::create_dir_all("../logs");
    let mut old = std::fs::read_to_string("../logs/perf.txt").unwrap_or_default();
    if !old.is_empty() && !old.ends_with('\n') {
        old.push('\n');
    }
    let _ = std::fs::write("../logs/perf.txt", format!("{old}{text}"));
    eprintln!("{text}");

    assert!(!running, "a test run is going: the numbers are not play (run {})", run["run"]);
    assert!(ticks >= 60, "{ticks} ticks in {} ms: paused or stepping, the numbers are not play", ONE_PUBLISH.as_millis());
    assert!(perf["parts"].as_array().is_some_and(|p| !p.is_empty()), "the perf op reports no parts: {perf}");
    // Todo 9m: the step's laps add up to the whole step, and the whole
    // step is what Bevy's FixedMain spends per step.
    assert!((laps - step).abs() <= step * 0.05, "the step's laps add up to {laps:.3} ms, the whole step is {step:.3}");
    let main_per_step = if per_frame > 0.0 { fixed_main / per_frame } else { 0.0 };
    assert!((main_per_step - step).abs() <= step * 0.05, "FixedMain is {main_per_step:.3} ms a step ({fixed_main:.3} a frame, {per_frame:.2} steps a frame), the whole step {step:.3}");
}

/// The chunks made, stepping from `opening_woke`: after each of ten steps
/// of 60 ticks, the moment's `chunks_made` (how many, and a hash of which)
/// and `next_thing_id` (a chunk's things take their ids in the order chunks
/// are made), and the chunks lap. A change to how chunks are asked for
/// must give the same list (docs/performance.md). Added to logs/perf.txt.
#[test]
fn the_chunks_made_stepping_from_woke() {
    let _paused = crate::endpoint::pause();
    crate::opening::woke();
    chunks_made_stepping("opening_woke");
}

/// The same on a game launched with `--world big` (no edge: chunks are made
/// as people walk into them, one person at a time), from the save
/// `chunks_big_world`: made from whatever the big world is at the first
/// time, loaded every time after, so two builds step the same world.
/// Stepped twice from the same save, the same chunks are made, in the same
/// order, at every check (authority.md rule 5). Only on the big world: on a
/// world with an edge (the world op's chunks `missing` is a number) it
/// cannot play what it tests, so it fails, and nothing is saved as
/// chunks_big_world.
#[test]
fn the_chunks_made_stepping_in_the_big_world() {
    let missing = op("world")["chunks"]["missing"].clone();
    assert!(missing.is_null(), "this test needs a game launched with --world big (the running world has an edge: {missing} of its chunks not made)");
    let _paused = crate::endpoint::pause();
    crate::endpoint::start_from("chunks_big_world", || {});
    let first = chunks_made_stepping("chunks_big_world");
    assert!(!crate::endpoint::start_from("chunks_big_world", || panic!("chunks_big_world was saved but does not load")));
    let second = chunks_made_stepping("chunks_big_world, again");
    assert_eq!(first, second, "the same save stepped the same ticks made other chunks the second time");
}

/// After each of ten steps of 60 ticks, the chunks made and the next
/// thing's id, and the chunks lap, into logs/perf.txt; none of the chunks
/// round a living person or the player is left not made at any check.
/// The checks, for comparing.
fn chunks_made_stepping(from: &str) -> Vec<String> {
    let mut rows = vec![format!("chunks made stepping from {from}:")];
    let mut checks = Vec::new();
    for _ in 0..10 {
        let at = crate::endpoint::step(60);
        let not_made = op("world")["chunks"]["not_made_round_people"].clone();
        let check = format!("tick {} chunks_made {} next_thing_id {}", at["tick"], at["chunks_made"], at["next_thing_id"]);
        rows.push(format!("  {check} not made round people {not_made}"));
        assert_eq!(not_made, json!(0), "at tick {}, chunks round people are not made: {not_made}", at["tick"]);
        checks.push(check);
    }
    let perf = op("perf");
    let lap = |name: &str| perf["parts"].as_array().into_iter().flatten().find(|p| p["part"] == name).and_then(|p| p["ms"].as_f64()).unwrap_or(f64::NAN);
    rows.push(format!("  the chunks lap {:.3} ms a step, the whole step {:.3}", lap("chunks"), perf["step_ms"].as_f64().unwrap_or(0.0)));
    let text = format!("{}\n\n", rows.join("\n"));
    let _ = std::fs::create_dir_all("../logs");
    let mut old = std::fs::read_to_string("../logs/perf.txt").unwrap_or_default();
    if !old.is_empty() && !old.ends_with('\n') {
        old.push('\n');
    }
    let _ = std::fs::write("../logs/perf.txt", format!("{old}{text}"));
    eprintln!("{text}");
    checks
}

/// What a stepped tick costs, as a test pays it: the `opening_woke` save
/// loaded, the game paused, 60 ticks stepped ten times, each step timed
/// by the real clock from asking to the answer, and the median of the
/// last five. Added to logs/perf.txt.
#[test]
fn what_a_stepped_tick_costs() {
    let _paused = crate::endpoint::pause();
    crate::opening::woke();
    let mut rows = vec!["stepping from opening_woke:".to_string()];
    let mut all = Vec::new();
    for _ in 0..10 {
        let started = std::time::Instant::now();
        let at = crate::endpoint::step(60);
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        all.push(ms);
        rows.push(format!("  60 ticks to tick {} in {ms:.0} ms: {:.2} ms a tick", at["tick"], ms / 60.0));
    }
    // The first steps after a load pay for the load's settling (chunk
    // meshes landing); the median of the last five is the step's own cost.
    let mut settled = all[all.len() - 5..].to_vec();
    settled.sort_by(f64::total_cmp);
    let median = settled[settled.len() / 2];
    rows.push(format!("  settled median (last 5): 60 ticks in {median:.0} ms, {:.2} ms a tick", median / 60.0));
    let worst = all.iter().copied().fold(0.0, f64::max);
    // Stepped on, untimed, until the load's own work (its world readied,
    // its chunks made) has faded from the spans' averages (each frame keeps
    // 29/30 of the last: 30 more calls are hundreds of frames).
    for _ in 0..30 {
        crate::endpoint::step(60);
    }
    let perf = op("perf");
    rows.push(format!("  the perf op's whole step {:.3} ms, its laps (average over about the last 30 steps):", perf["step_ms"].as_f64().unwrap_or(0.0)));
    for part in perf["parts"].as_array().into_iter().flatten().filter(|p| p["in_step"] == true) {
        rows.push(format!("    {:<44} {:>8.3}", part["part"].as_str().unwrap_or("?"), part["ms"].as_f64().unwrap_or(0.0)));
    }
    // Every span, also as ms a step: its share of FixedMain's span times
    // the whole step, so the paused frames between step calls (which run
    // no step and count as nothing) cancel out. Spans outside the fixed
    // step (drawing, answering) read as what a call pays besides its ticks.
    let spans: Vec<(String, f64)> = perf["frame"].as_array().into_iter().flatten().map(|s| (s["span"].as_str().unwrap_or("?").to_string(), s["ms"].as_f64().unwrap_or(0.0))).collect();
    let fixed_main = spans.iter().find(|(name, _)| name == "schedule FixedMain").map_or(0.0, |(_, ms)| *ms);
    let step_ms = perf["step_ms"].as_f64().unwrap_or(0.0);
    rows.push(format!("  every span, the costliest first: ms a frame, and ms a step (its share of FixedMain {fixed_main:.3} times the whole step {step_ms:.3}):"));
    for (name, ms) in &spans {
        let a_step = if fixed_main > 0.0 { ms / fixed_main * step_ms } else { 0.0 };
        rows.push(format!("    {:<90} {:>8.3} {:>8.3}", name.chars().take(90).collect::<String>(), ms, a_step));
    }
    let text = format!("{}\n\n", rows.join("\n"));
    let _ = std::fs::create_dir_all("../logs");
    let mut old = std::fs::read_to_string("../logs/perf.txt").unwrap_or_default();
    if !old.is_empty() && !old.ends_with('\n') {
        old.push('\n');
    }
    let _ = std::fs::write("../logs/perf.txt", format!("{old}{text}"));
    eprintln!("{text}");
    assert!(worst > 0.0, "no step was timed");
}
