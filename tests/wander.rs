//! People wander to spots they can reach (life.md "Wander: random,
//! reachable, unknown"): someone in the running game is wandering, and
//! no search the game could not finish was a wander (the `ways` op).

use crate::endpoint::{op, people};

/// A stroll, or heading out to look (the brain's tree, content/src/brain.rs).
fn wandering(doing: &serde_json::Value) -> bool {
    doing.as_str().is_some_and(|d| d == "wander" || d == "head out")
}

#[test]
fn people_wander_to_spots_they_can_reach() {
    let list = people();
    let wandering_now = list.iter().filter(|p| wandering(&p["doing"])).count();
    let ways = op("ways").as_array().cloned().unwrap_or_default();
    let failed: Vec<String> = ways
        .iter()
        // A stroll picks only spots reachable without a search (todo 11ag);
        // a trip out 150 m may fail, and is dropped (pathing.md rule 5).
        .filter(|w| w["doing"] == "wander")
        .map(|w| format!("{} from {} to {}: start blocked {}, goal blocked {}", w["who"], w["from"], w["to"], w["from_blocked"], w["to_blocked"]))
        .collect();
    eprintln!("{} people, {wandering_now} wandering; {} failed searches, {} of them wanders\n{}", list.len(), ways.len(), failed.len(), failed.join("\n"));
    // Every failed search, by what the person was doing, for looking at.
    let mut by_doing = std::collections::BTreeMap::<String, u32>::new();
    for w in &ways {
        *by_doing.entry(format!("{} (goal blocked {})", w["doing"], w["to_blocked"])).or_default() += 1;
    }
    eprintln!("failed searches by what they were doing: {by_doing:#?}");
    assert!(wandering_now > 0, "nobody in the running game is wandering");
    assert!(failed.is_empty(), "wanders the game could not reach:\n{}", failed.join("\n"));
}
