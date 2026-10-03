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
    let running = run.is_object() && run["done"] == false;

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
    rows.push(format!(
        "whole step {:.3} ms of {:.3}",
        perf["step_ms"].as_f64().unwrap_or(0.0),
        perf["budget_ms"].as_f64().unwrap_or(0.0)
    ));
    rows.push("parts of a step (ms):".to_string());
    for part in perf["parts"].as_array().into_iter().flatten() {
        rows.push(format!("  {:<48} {:>8.3}", part["part"].as_str().unwrap_or("?"), part["ms"].as_f64().unwrap_or(0.0)));
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
    let perf = op("perf");
    rows.push(format!("  the perf op's whole step {:.3} ms", perf["step_ms"].as_f64().unwrap_or(0.0)));
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
