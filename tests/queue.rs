//! The runner of the test queue (authority.md "Running tests"): queues the
//! tests named in TOPSIDE_QUEUE on the running game (the `tests` op), then
//! runs them one at a time, each by this same test program with `--exact`,
//! and reports each result to the game. It fails when any test failed, so
//! one command proves a goal. TOPSIDE_QUEUE is a list separated by spaces,
//! each a test file (every test in it: "the_tap" is tests/the_tap.rs) or
//! one test's full name. Ignored, so a whole run never starts it.

use std::process::Command;
use std::time::{Duration, Instant};

use serde_json::json;

use crate::endpoint::op_with;

/// The most of a test's output reported to the game: its end.
const OUTPUT_CHARS: usize = 4000;

/// The most any one test may take (operator, 2026-09-27: past 2 minutes a
/// test is worthless).
const LIMIT: Duration = Duration::from_secs(120);

/// This test's own name, never queued by itself.
const ME: &str = "queue::run_the_queue";

/// Every test name in this program (libtest `--list`).
fn every_test() -> Vec<String> {
    let me = std::env::current_exe().expect("the test program's own path");
    let out = Command::new(me).args(["--list"]).output().expect("the test program lists its tests");
    String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| l.strip_suffix(": test")).map(String::from).collect()
}

/// Test `name` run alone by this program (`me`), its output in a file so
/// nothing waits on a full pipe; stopped and failed past `LIMIT`. Whether
/// it passed, and its output.
fn run_one(me: &std::path::Path, name: &str) -> (bool, String) {
    let file = std::env::temp_dir().join(format!("topside-queue-{}.txt", std::process::id()));
    let out = std::fs::File::create(&file).expect("the output file");
    let err = out.try_clone().expect("the output file, twice");
    let mut child = Command::new(me).args(["--exact", name, "--include-ignored", "--nocapture"]).stdout(out).stderr(err).spawn().expect("the test program runs one test");
    let started = Instant::now();
    let passed = loop {
        if let Some(status) = child.try_wait().expect("the test's process") {
            break status.success();
        }
        if started.elapsed() > LIMIT {
            let _ = child.kill();
            let _ = child.wait();
            break false;
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    let mut text = String::from_utf8_lossy(&std::fs::read(&file).unwrap_or_default()).into_owned();
    if !passed && started.elapsed() > LIMIT {
        text.push_str(&format!("\nstopped: past {} s, the most any test may take", LIMIT.as_secs()));
    }
    (passed, text)
}

#[test]
#[ignore]
fn run_the_queue() {
    let wanted = std::env::var("TOPSIDE_QUEUE").expect("TOPSIDE_QUEUE names the tests to queue, separated by spaces");
    let parts: Vec<&str> = wanted.split_whitespace().collect();
    let names: Vec<String> = every_test().into_iter().filter(|n| n != ME && parts.iter().any(|p| n == p || n.starts_with(&format!("{p}::")))).collect();
    assert!(!names.is_empty(), "no test matches TOPSIDE_QUEUE={wanted}");
    let run = op_with("tests", json!({"queue": names}))["run"].as_u64().expect("the tests op answers the run's id");
    eprintln!("run {run}: {} tests queued", names.len());

    let me = std::env::current_exe().expect("the test program's own path");
    loop {
        let next = op_with("tests", json!({"take": run}));
        let Some(name) = next.as_str() else { break };
        eprintln!("running {name}");
        let (passed, mut text) = run_one(&me, name);
        // How far the test played from its save point (todo 9k: a few game
        // seconds): the save it loaded last, as `start_from` names it, and
        // the game seconds played since (the `status` op's `since_load`).
        let save = text.lines().filter_map(|l| l.strip_prefix("loaded the save ")).last().and_then(|l| l.split(':').next()).map(str::to_string);
        let played = crate::endpoint::op("status")["since_load"].as_u64().unwrap_or(0) as f64 / 60.0;
        let from = match &save {
            Some(save) => format!("from its save {save}: {played:.1} game seconds played after it"),
            None => format!("from no save: {played:.1} game seconds played since the last load"),
        };
        text.push_str(&format!("\n{from}\n"));
        let cut = text.char_indices().rev().nth(OUTPUT_CHARS).map_or(0, |(i, _)| i);
        eprintln!("  {}, {from}", if passed { "passed" } else { "FAILED" });
        if !passed {
            eprintln!("{}", &text[cut..]);
        }
        op_with("tests", json!({"report": run, "name": name, "passed": passed, "output": &text[cut..]}));
    }

    let status = op_with("tests", json!({"status": run}));
    for t in status["tests"].as_array().cloned().unwrap_or_default() {
        eprintln!("{:<8} {}", t["state"].as_str().unwrap_or("?"), t["name"].as_str().unwrap_or("?"));
    }
    eprintln!("run {run}: {} passed, {} failed", status["passed"], status["failed"]);
    assert_eq!(status["failed"], 0, "run {run}: some tests failed; the tests op's status holds each one's output");
    assert_eq!(status["done"], true, "run {run} has tests not finished");
}
