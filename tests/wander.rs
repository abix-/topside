//! People wander to spots they can reach (life.md "Wander: random,
//! reachable, unknown"): someone in the running game is wandering, and
//! no search the game could not finish was a wander (the `ways` op).

use crate::endpoint::{op, people};

#[test]
fn people_wander_to_spots_they_can_reach() {
    let list = people();
    let wandering = list.iter().filter(|p| p["doing"].as_str().is_some_and(|d| d.starts_with("Wander"))).count();
    let ways = op("ways").as_array().cloned().unwrap_or_default();
    let failed: Vec<String> = ways
        .iter()
        .filter(|w| w["doing"].as_str().is_some_and(|d| d.starts_with("Wander")))
        .map(|w| format!("{} from {} to {}: start blocked {}, goal blocked {}", w["who"], w["from"], w["to"], w["from_blocked"], w["to_blocked"]))
        .collect();
    eprintln!("{} people, {wandering} wandering; {} failed searches, {} of them wanders\n{}", list.len(), ways.len(), failed.len(), failed.join("\n"));
    assert!(wandering > 0, "nobody in the running game is wandering");
    assert!(failed.is_empty(), "wanders the game could not reach:\n{}", failed.join("\n"));
}
