//! People come back from the dead with half health, full food, and full
//! water (operator, 2026-09-26): from each person's memory in the
//! running game's `npc` op, nobody who respawned starves or dies of
//! thirst within half a game day, which full food and water outlast.

use serde_json::{Value as Json, json};

use crate::endpoint::url;
/// Half a game day in ticks (a day is 1200 s at 60 ticks a second).
const HALF_DAY: u64 = 600 * 60;

fn op(name: &str, args: Json) -> Option<Json> {
    let resp = ureq::post(url()).send_json(json!({"op": name, "args": args})).ok()?;
    resp.into_body().read_json().ok()
}

#[test]
fn the_respawned_do_not_starve_straight_away() {
    let people = op("people", json!({})).unwrap_or_else(|| panic!("no topside game answering on {}; start it with scripts/build.ps1 run", url()));
    let ids: Vec<u64> = people["result"].as_array().into_iter().flatten().filter_map(|p| p["id"].as_u64()).collect();
    let (mut respawns, mut starved) = (0, Vec::new());
    for id in ids {
        let Some(npc) = op("npc", json!({"id": id})) else { continue };
        // Oldest first: (tick, words).
        let did: Vec<(u64, String)> = npc["result"]["did"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|d| (d["tick"].as_u64().unwrap_or(0), d["did"].as_str().unwrap_or("").to_string()))
            .collect();
        for (i, (at, words)) in did.iter().enumerate() {
            if words != "respawned" {
                continue;
            }
            respawns += 1;
            if let Some((died, how)) = did[i + 1..].iter().find(|(_, w)| w.starts_with("died")) {
                if (how.contains("starvation") || how.contains("dehydration")) && died - at < HALF_DAY {
                    starved.push(format!("npc {id}: respawned at {at}, {how} at {died} ({} ticks later)", died - at));
                }
            }
        }
    }
    eprintln!("{respawns} respawns read; {} starved or died of thirst within half a day\n{}", starved.len(), starved.join("\n"));
    assert!(respawns > 0, "nobody in the running game has respawned yet");
    assert!(starved.is_empty(), "the respawned came back without food or water:\n{}", starved.join("\n"));
}
