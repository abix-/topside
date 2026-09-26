//! Bunker people haul food home: what each person's memory says they
//! put into a box (their bunker's store) in the running game's `people`
//! op, and every bunker's store from the `world` op.

use serde_json::{Value as Json, json};

const URL: &str = "http://127.0.0.1:15703/topside";

fn op(name: &str) -> Option<Json> {
    op_with(name, json!({}))
}

fn op_with(name: &str, args: Json) -> Option<Json> {
    let resp = ureq::post(URL).send_json(json!({"op": name, "args": args})).ok()?;
    resp.into_body().read_json().ok()
}

/// One bunker person in a few lines, from the `npc` op: where, what they
/// are doing, needs, what they carry, what they know and what it held,
/// the last things they did, and their best learned choices.
fn person(id: u64) -> String {
    let Some(npc) = op_with("npc", json!({"id": id})) else {
        return format!("npc {id}: no answer");
    };
    let n = &npc["result"];
    let knows: Vec<String> = n["knows"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|k| format!("{} at {} held {}", k["kind"].as_str().unwrap_or("?"), k["at"], k["held"]))
        .collect();
    let did: Vec<String> = n["did"]
        .as_array()
        .into_iter()
        .flatten()
        .rev()
        .take(5)
        .map(|d| format!("{} {}", d["tick"], d["did"].as_str().unwrap_or("?")))
        .collect();
    let learned: Vec<String> = n["learned"]
        .as_array()
        .into_iter()
        .flatten()
        .take(3)
        .map(|l| format!("{} {:.2} in {}", l["choice"].as_str().unwrap_or("?"), l["value"].as_f64().unwrap_or(0.0), l["situation"].as_str().unwrap_or("?")))
        .collect();
    format!(
        "npc {id} ({}) at {} home {} dead {}\n  doing {}\n  needs {}\n  carrying {}\n  way {}\n  knows {}\n  did (newest first) {}\n  learned {}",
        n["faction"].as_str().unwrap_or("-"),
        n["at"],
        n["home"],
        n["dead"],
        n["doing"].as_str().unwrap_or("?"),
        n["needs"],
        n["carrying"],
        n["way"],
        knows.join("; "),
        did.join("; "),
        learned.join("; ")
    )
}

#[test]
fn bunker_people_haul_food_home() {
    let no_game = || panic!("no topside game answering on {URL}; start it with scripts/build.ps1 run");
    let people = op("people").unwrap_or_else(no_game);
    let world = op("world").unwrap_or_else(no_game);
    let list = people["result"].as_array().cloned().unwrap_or_default();
    let hauled: Vec<String> = list
        .iter()
        .filter(|p| p["faction"].as_str().is_some_and(|f| f.starts_with("bunker")))
        .filter_map(|p| {
            let n = p["did"]["put in"].as_u64().unwrap_or(0);
            (n > 0).then(|| format!("{} ({}): put in {n} times, last {}", p["id"], p["faction"].as_str().unwrap_or("-"), p["last_did"]["did"]))
        })
        .collect();
    eprintln!(
        "at tick {}: {} bunker people hauled home; stores {}\n{}",
        world["result"]["tick"],
        hauled.len(),
        world["result"]["bunkers"],
        hauled.join("\n")
    );
    // Every bunker hauls home and none starves (a bunker's store runs
    // short within its first day: 10 cans and 10 bottles for five).
    let bunkers = world["result"]["bunkers"].as_object().cloned().unwrap_or_default();
    let failing: Vec<String> = bunkers
        .iter()
        .filter(|(_, b)| b["stocked"].as_u64().unwrap_or(0) == 0 || b["starving"].as_u64().unwrap_or(0) > 0)
        .map(|(name, b)| format!("{name}: {b}"))
        .collect();
    if !failing.is_empty() {
        // Why not: every person of those bunkers, as the game sees them.
        let ids: Vec<u64> = list
            .iter()
            .filter(|p| p["faction"].as_str().is_some_and(|f| failing.iter().any(|b| b.starts_with(&format!("{f}:")))))
            .filter_map(|p| p["id"].as_u64())
            .collect();
        for id in ids {
            eprintln!("{}", person(id));
        }
    }
    assert!(failing.is_empty(), "bunkers that never hauled food home or are starving:\n{}", failing.join("\n"));
}
