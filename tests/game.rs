//! The actual game, tested (design.md "The founding idea": the world
//! keeps running if the player hides and starves in the bunker). The
//! test launches the real topside.exe on a fixed seed at normal speed,
//! leaves the player in the bunker, and watches it for two minutes
//! through the control plane. Every five seconds a line goes to
//! logs/game.txt: frames a second, how fast the game runs against real
//! time, the costliest parts of a step, and what the people have done.
//! It passes when the game kept up with real time the whole way.

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value as Json, json};

const URL: &str = "http://127.0.0.1:15703/topside";
/// How long to watch the game, in real time.
const WATCH: Duration = Duration::from_secs(120);
/// Keeping up: the game's clock runs at least this share of real time.
const KEEPS_UP: f64 = 0.95;

fn op(name: &str) -> Option<Json> {
    let resp = ureq::post(URL).send_json(json!({"op": name, "args": {}})).ok()?;
    resp.into_body().read_json().ok()
}

/// The game, closed when the test ends however it ends.
struct Game(Child);

impl Drop for Game {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn the_game_keeps_up_with_real_time_and_its_people_live() {
    assert!(
        op("status").is_none(),
        "a topside game is already running on 127.0.0.1:15703; close it first"
    );
    let _ = std::fs::create_dir_all("logs");
    let err = std::fs::File::create("logs/game_test.err.log").expect("the game's log");
    let _game = Game(
        Command::new(env!("CARGO_BIN_EXE_topside"))
            .args(["--seed", "1"])
            .stdout(Stdio::null())
            .stderr(err)
            .spawn()
            .expect("the game starts"),
    );
    let started = Instant::now();
    while op("status").is_none() {
        assert!(started.elapsed() < Duration::from_secs(120), "the game's control plane never answered");
        std::thread::sleep(Duration::from_millis(250));
    }

    let mut rows = vec![format!(
        "{:>6} {:>5} {:>5}  {:<40} {:>5} {:>5} {:>7} {:>5}  bunkers (food/water, alive, starving)",
        "real s", "fps", "speed", "costliest parts of a step (ms)", "ate", "taken", "stocked", "died"
    )];
    let watch = Instant::now();
    let (mut last_tick, mut last_at) = (0, Instant::now());
    let mut slowest = f64::MAX;
    while watch.elapsed() < WATCH {
        std::thread::sleep(Duration::from_secs(5));
        let (Some(status), Some(world), Some(perf)) = (op("status"), op("world"), op("perf")) else {
            panic!("the game stopped answering; its log is logs/game_test.err.log\n{}", rows.join("\n"));
        };
        let tick = status["state"]["tick"].as_u64().unwrap_or(0);
        let speed = tick.saturating_sub(last_tick) as f64 / 60.0 / last_at.elapsed().as_secs_f64();
        // The first sample includes the start; the rest measure keeping up.
        if last_tick > 0 {
            slowest = slowest.min(speed);
        }
        (last_tick, last_at) = (tick, Instant::now());
        let parts: Vec<String> = perf["result"]
            .as_array()
            .map(|p| {
                p.iter()
                    .take(3)
                    .map(|x| format!("{} {:.1}", x["part"].as_str().unwrap_or("?"), x["ms"].as_f64().unwrap_or(0.0)))
                    .collect()
            })
            .unwrap_or_default();
        let w = &world["result"];
        let bunkers: Vec<String> = w["bunkers"]
            .as_object()
            .map(|b| {
                b.values()
                    .map(|v| format!("{}/{} {} {}", v["food"], v["water"], v["alive"], v["starving"]))
                    .collect()
            })
            .unwrap_or_default();
        rows.push(format!(
            "{:>6.0} {:>5.0} {:>5.2}  {:<40} {:>5} {:>5} {:>7} {:>5}  {}",
            watch.elapsed().as_secs_f64(),
            status["state"]["fps"].as_f64().unwrap_or(0.0),
            speed,
            parts.join(", "),
            w["ate"],
            w["taken"],
            w["stocked"],
            w["died"],
            bunkers.join(" | ")
        ));
        let _ = std::fs::write("logs/game.txt", format!("{}\n", rows.join("\n")));
    }
    eprintln!("{}", rows.join("\n"));
    assert!(
        slowest >= KEEPS_UP,
        "the game fell behind real time (slowest {slowest:.2} of real time):\n{}",
        rows.join("\n")
    );
}
