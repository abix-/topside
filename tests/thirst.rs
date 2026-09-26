//! Why topside people die of thirst with wells and rivers in the world
//! (todo row 1). Every living topside person who is thirsty in the running
//! game, read with the `npc` op: what the brain has them doing, what it is
//! about, the water they know of, and their way. Fails while a thirsty
//! person is not seeing to their thirst.

use crate::endpoint::{is_bunker_person, op_with, people};
use serde_json::json;

/// Below this, a person is thirsty enough that drinking should come first
/// (the needs run from 100, met, down to 0).
const THIRSTY: f64 = 30.0;

/// Everyone who died of thirst: the last things their memory says they did
/// before it, and the water they knew of. Reads only; the verdict is
/// deaths.rs's.
#[test]
fn what_those_who_died_of_thirst_did_before() {
    for p in people().iter().filter(|p| !is_bunker_person(p) && p["def"] != "player") {
        let Some(death) = p["deaths"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|d| d["of"] == "dehydration")
        else {
            continue;
        };
        let npc = op_with("npc", json!({"id": p["id"]}));
        let died = npc["did"]
            .as_array()
            .into_iter()
            .flatten()
            .position(|d| d["did"].as_str().is_some_and(|w| w.contains("dehydration")));
        let before: Vec<String> = npc["did"]
            .as_array()
            .into_iter()
            .flatten()
            .take(died.map_or(usize::MAX, |i| i + 1))
            .map(|d| format!("{} {}", d["tick"], d["did"]))
            .collect();
        let knows: Vec<String> = npc["knows"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|k| format!("{} at {} seen {}", k["kind"], k["at"], k["seen_at"]))
            .collect();
        eprintln!(
            "\n{} {} died of thirst {death}\n  spawned at {}, now {} doing {}\n  did (last 12 before): {:#?}\n  knows {} things: {:?}",
            p["id"],
            p["def"],
            npc["spawned_at"],
            npc["at"],
            npc["doing"],
            &before[before.len().saturating_sub(12)..],
            knows.len(),
            knows,
        );
    }
}

#[test]
fn thirsty_topside_people_see_to_their_thirst() {
    let list = people();
    let thirsty: Vec<_> = list
        .iter()
        .filter(|p| !is_bunker_person(p) && p["def"] != "player")
        .filter_map(|p| p["id"].as_u64())
        .filter_map(|id| {
            let npc = op_with("npc", json!({"id": id}));
            (npc["dead"] == false && npc["needs"]["thirst"].as_f64().is_some_and(|t| t < THIRSTY)).then_some(npc)
        })
        .collect();
    eprintln!("{} topside people thirsty (thirst under {THIRSTY})", thirsty.len());
    let mut neglected = Vec::new();
    for npc in &thirsty {
        let water: Vec<String> = npc["knows"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|k| matches!(k["kind"].as_str(), Some("well" | "river" | "water" | "water bottle")))
            .map(|k| format!("{} at {}", k["kind"], k["at"]))
            .collect();
        let doing = npc["doing"].as_str().unwrap_or("?");
        eprintln!(
            "{} {} at {} thirst {:.0}: doing {doing} about {}; knows water: {:?}; carries {}; way goal {} planned {} queued {} retry {}; last did {}",
            npc["id"],
            npc["def"],
            npc["at"],
            npc["needs"]["thirst"].as_f64().unwrap_or(0.0),
            npc["about"],
            water,
            npc["carrying"],
            npc["way"]["goal"],
            npc["way"]["planned_at"],
            npc["way"]["queued"],
            npc["way"]["retry_at"],
            npc["did"].as_array().and_then(|d| d.last()).map_or("nothing".to_string(), |d| d.to_string()),
        );
        let learned: Vec<String> = npc["learned"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|l| l["situation"].as_str().is_some_and(|s| s.contains("thirst: Low")))
            .take(8)
            .map(|l| format!("{} {:.2} in {}", l["choice"], l["value"].as_f64().unwrap_or(0.0), l["situation"]))
            .collect();
        eprintln!("  alive for {}, lives {}, learned while thirsty: {learned:#?}", npc["alive_for"], npc["lives"]);
        eprintln!("  in reach {}; water in reach {}; this tick {}", npc["in_reach"], npc["water_in_reach"], npc["this_tick"]);
        // Knowing no water, they head out to look (life.md: nothing known
        // answers a need); knowing some, they go to it.
        if !water.is_empty() && !npc["about"].as_str().is_some_and(|a| a.contains("Thirst")) {
            neglected.push(format!("{} {} doing {doing}", npc["id"], npc["def"]));
        }
    }
    assert!(neglected.is_empty(), "thirsty but not seeing to it: {neglected:#?}");
}
