//! The actual game, tested as it runs (design.md "The founding idea":
//! the world keeps running if the player hides and starves in the
//! bunker). The test attaches to the running topside.exe on the
//! control plane, never starts one and never waits, and writes to
//! logs/game.txt what the game reports now: how fast it runs against
//! real time (the game measures that itself), frames a second, the
//! costliest parts of a step, the people, and the searches that found
//! no way. It passes when the game is keeping up.

use serde_json::{Value as Json, json};

const URL: &str = "http://127.0.0.1:15703/topside";
/// Keeping up: the game's clock runs at least this share of real time.
const KEEPS_UP: f64 = 0.95;

fn op(name: &str) -> Option<Json> {
    op_with(name, json!({}))
}

fn op_with(name: &str, args: Json) -> Option<Json> {
    let resp = ureq::post(URL).send_json(json!({"op": name, "args": args})).ok()?;
    resp.into_body().read_json().ok()
}

/// What a fixed step costs now, part by part, costliest first, and the
/// frame rate: read only, the game left as it is.
#[test]
fn what_each_step_costs_now() {
    let (Some(perf), Some(status)) = (op("perf"), op("status")) else {
        panic!("no game answering");
    };
    let p = &perf["result"];
    eprintln!("whole step {:.2} ms of {:.2}, {} people; {} frames a second", p["step_ms"].as_f64().unwrap_or(0.0), p["budget_ms"].as_f64().unwrap_or(0.0), p["people"], status["result"]["fps"]);
    for part in p["parts"].as_array().into_iter().flatten().take(15) {
        eprintln!("  {:<40} {:>7.3} ms", part["part"].as_str().unwrap_or("?"), part["ms"].as_f64().unwrap_or(0.0));
    }
    // Bevy's own diagnostics: the whole frame, drawing included.
    eprintln!("diagnostics: {}", op("diagnostics").map_or(Json::Null, |d| d["result"].clone()));
    // Every span Bevy ran, ms per frame, the costliest first (profile.rs).
    let frame = p["frame"].as_array().cloned().unwrap_or_default();
    for span in frame.iter().take(25) {
        eprintln!("  frame {:<70} {:>7.3} ms", span["span"].as_str().unwrap_or("?"), span["ms"].as_f64().unwrap_or(0.0));
    }
    assert!(!frame.is_empty(), "the perf op reports no spans per frame");
}

#[test]
fn the_game_keeps_up_with_real_time_and_its_people_live() {
    let (Some(status), Some(world), Some(perf)) = (op("status"), op("world"), op("perf")) else {
        panic!("no topside game answering on {URL}; start it with scripts/build.ps1 run");
    };
    let _ = std::fs::create_dir_all("../logs");
    let mut rows = vec![format!(
        "{:>6} {:>6} {:>5} {:>5} {:>7}  {:<40} {:>5} {:>5} {:>7} {:>5}  bunkers (food/water, alive, starving)",
        "tick", "people", "fps", "speed", "step ms", "costliest parts of a step (ms)", "ate", "taken", "stocked", "died"
    )];
    let speed = status["state"]["speed"].as_f64().unwrap_or(0.0);
    {
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
            "{:>6} {:>6} {:>5.0} {:>5.2} {:>7.2}  {:<40} {:>5} {:>5} {:>7} {:>5}  {}",
            status["state"]["tick"].as_u64().unwrap_or(0),
            status["state"]["people"],
            status["state"]["fps"].as_f64().unwrap_or(0.0),
            speed,
            perf["result"]["step_ms"].as_f64().unwrap_or(0.0),
            parts.join(", "),
            w["did"]["ate"],
            w["did"]["took"],
            w["did"]["put in"],
            w["did"]["died"],
            bunkers.join(" | ")
        ));
        let _ = std::fs::write("../logs/game.txt", format!("{}\n", rows.join("\n")));
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
        let _ = std::fs::write("../logs/game.txt", format!("{}\n", rows.join("\n")));
    }
    // Every person at the end, from their memory (life.md "What a person
    // did"): needs, what they are doing, how many of each thing they did,
    // who killed them last. A topside person who died is gone; their
    // killer's "killed" counts them.
    if let Some(people) = op("people") {
        const KINDS: [&str; 9] = ["ate", "slept", "took", "put in", "hit", "was hit", "killed", "died", "respawned"];
        rows.push(String::new());
        rows.push(format!(
            "{:>5} {:<14} {:<17} {:>4} {:>4} {:>4} {:>4} {:>4}  {:<20} {}  {:>8} {:>7}  {:<24} killed by / last did",
            "id", "def", "faction", "dead", "hp", "hung", "thir", "rest", "doing",
            KINDS.map(|k| format!("{k:>6}")).join(""),
            "alive s", "learned", "lives (s each)"
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
            let lives: Vec<String> = p["lives"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|s| format!("{:.0}", s.as_f64().unwrap_or(0.0)))
                .collect();
            rows.push(format!(
                "{:>5} {:<14} {:<17} {:>4} {:>4.0} {:>4.0} {:>4.0} {:>4.0}  {:<20} {}  {:>8.0} {:>7}  {:<24} {}{}",
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
                p["alive_for"].as_f64().unwrap_or(0.0),
                p["learned"],
                lives.join(" "),
                killed_by,
                p["last_did"]["did"].as_str().unwrap_or("")
            ));
        }
        let _ = std::fs::write("../logs/game.txt", format!("{}\n", rows.join("\n")));
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
        let _ = std::fs::write("../logs/game.txt", format!("{}\n", rows.join("\n")));
    }
    eprintln!("{}", rows.join("\n"));
    assert!(
        speed >= KEEPS_UP,
        "the game fell behind real time ({speed:.2} of real time):\n{}",
        rows.join("\n")
    );
}
