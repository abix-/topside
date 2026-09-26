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
    op_with(name, json!({}))
}

fn op_with(name: &str, args: Json) -> Option<Json> {
    let resp = ureq::post(URL).send_json(json!({"op": name, "args": args})).ok()?;
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
        "{:>6} {:>6} {:>5} {:>5} {:>7}  {:<40} {:>5} {:>5} {:>7} {:>5}  bunkers (food/water, alive, starving)",
        "real s", "people", "fps", "speed", "step ms", "costliest parts of a step (ms)", "ate", "taken", "stocked", "died"
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
        let parts: Vec<String> = perf["result"]["parts"]
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
            "{:>6.0} {:>6} {:>5.0} {:>5.2} {:>7.2}  {:<40} {:>5} {:>5} {:>7} {:>5}  {}",
            watch.elapsed().as_secs_f64(),
            status["state"]["people"],
            status["state"]["fps"].as_f64().unwrap_or(0.0),
            speed,
            perf["result"]["step_ms"].as_f64().unwrap_or(0.0),
            parts.join(", "),
            w["ate"],
            w["taken"],
            w["stocked"],
            w["died"],
            bunkers.join(" | ")
        ));
        let _ = std::fs::write("logs/game.txt", format!("{}\n", rows.join("\n")));
    }
    // At the end, every part and count the game reports.
    if let Some(perf) = op("perf") {
        let p = &perf["result"];
        rows.push(String::new());
        rows.push(format!("whole step {:.2} ms of {:.2}, {} people", p["step_ms"].as_f64().unwrap_or(0.0), p["budget_ms"].as_f64().unwrap_or(0.0), p["people"]));
        for part in p["parts"].as_array().into_iter().flatten() {
            rows.push(format!("  {:<32} {:>6.3} ms", part["part"].as_str().unwrap_or("?"), part["ms"].as_f64().unwrap_or(0.0)));
        }
        for count in p["counts"].as_array().into_iter().flatten() {
            rows.push(format!("  {:<32} {:>8.2} a step", count["what"].as_str().unwrap_or("?"), count["per_step"].as_f64().unwrap_or(0.0)));
        }
        let _ = std::fs::write("logs/game.txt", format!("{}\n", rows.join("\n")));
    }
    // Every person at the end, from their memory (life.md "What a person
    // did"): needs, what they are doing, how many of each thing they did,
    // who killed them last. A topside person who died is gone; their
    // killer's "killed" counts them.
    if let Some(people) = op("people") {
        const KINDS: [&str; 9] = ["ate", "slept", "took", "put in", "hit", "was hit", "killed", "died", "respawned"];
        rows.push(String::new());
        rows.push(format!(
            "{:>5} {:<14} {:<17} {:>4} {:>4} {:>4} {:>4} {:>4}  {:<20} {}  killed by / last did",
            "id", "def", "faction", "dead", "hp", "hung", "thir", "rest", "doing",
            KINDS.map(|k| format!("{k:>6}")).join("")
        ));
        let mut list: Vec<Json> = people["result"].as_array().cloned().unwrap_or_default();
        list.sort_by_key(|p| p["id"].as_u64().unwrap_or(0));
        for p in list {
            let n = |k: &str| p["did"][k].as_u64().unwrap_or(0);
            let doing: String = p["doing"].as_str().unwrap_or("?").chars().take(20).collect();
            let killed_by = match &p["killed_by"] {
                Json::Null => String::new(),
                k => format!("{} by {} / ", k["of"].as_str().unwrap_or("?"), k["by"]),
            };
            rows.push(format!(
                "{:>5} {:<14} {:<17} {:>4} {:>4.0} {:>4.0} {:>4.0} {:>4.0}  {:<20} {}  {}{}",
                p["id"],
                p["def"].as_str().unwrap_or("?"),
                p["faction"].as_str().unwrap_or("-"),
                if p["dead"].as_bool().unwrap_or(false) { "yes" } else { "" },
                p["health"].as_f64().unwrap_or(0.0),
                p["hunger"].as_f64().unwrap_or(0.0),
                p["thirst"].as_f64().unwrap_or(0.0),
                p["rest"].as_f64().unwrap_or(0.0),
                doing,
                KINDS.map(|k| format!("{:>6}", n(k))).join(""),
                killed_by,
                p["last_did"]["did"].as_str().unwrap_or("")
            ));
        }
        let _ = std::fs::write("logs/game.txt", format!("{}\n", rows.join("\n")));
    }
    // And the last searches that found no way.
    if let Some(ways) = op("ways") {
        rows.push(String::new());
        rows.push("searches that found no way (newest last):".to_string());
        for w in ways["result"].as_array().into_iter().flatten() {
            rows.push(format!(
                "  {} doing {} from {} to {} ({:.0} m): start blocked {}, goal blocked {}, {} tiles searched",
                w["who"].as_str().unwrap_or("?"),
                w["doing"].as_str().unwrap_or("?"),
                w["from"],
                w["to"],
                w["distance"].as_f64().unwrap_or(0.0),
                w["from_blocked"],
                w["to_blocked"],
                w["tiles_searched"]
            ));
        }
        // The last three of them in full, their chunk drawn from the walk
        // grid (the `npc` op).
        let ids: Vec<u64> = ways["result"]
            .as_array()
            .into_iter()
            .flatten()
            .rev()
            .filter_map(|w| w["id"].as_u64())
            .take(3)
            .collect();
        for id in ids {
            let Some(npc) = op_with("npc", json!({"id": id})) else {
                continue;
            };
            let n = &npc["result"];
            rows.push(String::new());
            rows.push(format!(
                "npc {id}: {} ({}) at {} doing {}; chunk {} tile {}: {} doorways, {} reachable from here",
                n["def"].as_str().unwrap_or("?"),
                n["faction"],
                n["at"],
                n["doing"].as_str().unwrap_or("?"),
                n["chunk"]["chunk"],
                n["chunk"]["tile"],
                n["chunk"]["doorways"],
                n["chunk"]["doorways_reachable"]
            ));
            rows.push(format!("  spawned at {}, standing on {}", n["spawned_at"], n["standing_on"]));
            for line in n["chunk"]["tiles"].as_array().into_iter().flatten() {
                rows.push(format!("    {}", line.as_str().unwrap_or("")));
            }
            let g = &n["goal_chunk"];
            if !g.is_null() {
                rows.push(format!(
                    "  the goal's chunk {} tile {}: {} doorways, {} reachable from the goal",
                    g["chunk"], g["tile"], g["doorways"], g["doorways_reachable"]
                ));
                for line in g["tiles"].as_array().into_iter().flatten() {
                    rows.push(format!("    {}", line.as_str().unwrap_or("")));
                }
            }
        }
        let _ = std::fs::write("logs/game.txt", format!("{}\n", rows.join("\n")));
    }
    eprintln!("{}", rows.join("\n"));
    assert!(
        slowest >= KEEPS_UP,
        "the game fell behind real time (slowest {slowest:.2} of real time):\n{}",
        rows.join("\n")
    );
}
