//! Explicit renderer diagnostics. Run alone through scripts/build.ps1 queue.
//! The session checkpoint is separate from the episode's committed saves.

use std::time::{Duration, Instant};

use serde_json::json;

use crate::endpoint::{loaded, op, op_with};

const SESSION: &str = "renderer_comparison_session";

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

#[test]
#[ignore = "explicit diagnostic: preserves the current session before a restart"]
fn preserve_session() {
    let path = root().join("saves").join(format!("{SESSION}.save"));
    assert!(
        !path.exists(),
        "a session checkpoint already exists at {}; preserve it before capturing another",
        path.display()
    );
    let saved = op_with("save", json!({"name": SESSION}));
    assert!(saved["tick"].is_u64(), "saving answered no tick: {saved}");
    assert!(path.is_file(), "the session checkpoint was not written");
    eprintln!("preserved session at tick {}, seed {}", saved["tick"], saved["seed"]);
}

#[test]
#[ignore = "explicit diagnostic: restores the preserved session"]
fn restore_session() {
    let restored = op_with("load", json!({"name": SESSION}));
    loaded();
    assert!(restored["tick"].is_u64(), "loading answered no tick: {restored}");
    eprintln!("restored session at tick {}, seed {}", restored["tick"], restored["seed"]);
}

#[test]
#[ignore = "explicit diagnostic: records renderer timings during normal play"]
fn frame_times() {
    loaded();
    let log = std::fs::read_to_string(root().join("logs/topside.err.log")).expect("the game startup log");
    let adapter = log
        .lines()
        .find(|line| line.contains("AdapterInfo {"))
        .expect("the log identifies the renderer");
    let backend = if adapter.contains("backend: Vulkan") {
        "vulkan"
    } else if adapter.contains("backend: Dx12") {
        "dx12"
    } else {
        "other"
    };
    let started = Instant::now();
    let mut samples = Vec::new();
    for _ in 0..5 {
        let before = op("status");
        let sample_start = Instant::now();
        std::thread::sleep(Duration::from_secs(1));
        let after = op("status");
        let diagnostics = op("diagnostics");
        samples.push(json!({"elapsed_ms": sample_start.elapsed().as_secs_f64() * 1000.0, "before": before, "after": after, "diagnostics": diagnostics}));
    }
    let report = json!({"adapter": adapter, "backend": backend, "elapsed_ms": started.elapsed().as_secs_f64() * 1000.0, "samples": samples, "perf": op("perf"), "view": op("view")});
    let path = root().join("logs").join(format!("renderer-{backend}.json"));
    std::fs::write(&path, serde_json::to_string_pretty(&report).expect("serialize the renderer report")).expect("write the renderer report");
    for sample in &samples {
        eprintln!(
            "{backend}: fps {}, frame {} ms, {} ticks in {:.0} ms",
            sample["diagnostics"]["fps"],
            sample["diagnostics"]["frame_time"],
            sample["after"]["tick"]
                .as_u64()
                .unwrap_or(0)
                .saturating_sub(sample["before"]["tick"].as_u64().unwrap_or(0)),
            sample["elapsed_ms"].as_f64().unwrap_or(0.0)
        );
    }
    eprintln!("renderer report: {}", path.display());
    assert!(
        samples
            .iter()
            .all(|s| s["after"]["tick"].as_u64().unwrap_or(0) > s["before"]["tick"].as_u64().unwrap_or(0)),
        "the simulation did not advance during every sample"
    );
    assert!(
        samples.iter().all(|s| s["diagnostics"]["frame_time"].as_f64().is_some_and(|ms| ms > 0.0)),
        "the renderer reported no frame times"
    );
}
