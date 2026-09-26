//! A person who dies still exists (life.md "What a person did"): dead
//! topside people are still in the running game's `people` op, and how
//! they died is in their own memory.

use crate::endpoint::{is_bunker_person, people};

#[test]
fn dead_topside_people_stay_with_how_they_died() {
    let list = people();
    let dead: Vec<String> = list
        .iter()
        .filter(|p| !is_bunker_person(p) && p["dead"] == true)
        .map(|p| format!("{} {}: {}", p["id"], p["def"].as_str().unwrap_or("?"), p["killed_by"]))
        .collect();
    let unknown = dead.iter().filter(|d| d.ends_with("null")).count();
    eprintln!("{} people, {} dead topside people\n{}", list.len(), dead.len(), dead.join("\n"));
    assert!(!dead.is_empty(), "no dead topside person in the running game yet");
    assert_eq!(unknown, 0, "dead topside people whose memory does not say how they died:\n{}", dead.join("\n"));
}
