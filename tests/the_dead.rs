//! A person who dies still exists (life.md "What a person did"): dead
//! topside people are still in the running game's `people` op, and how
//! they died is in their own memory.

use serde_json::json;

use crate::endpoint::{is_bunker_person, op_with, pause, people, step};

/// From the game's start (tap_0_you_wake), one living topside person
/// killed (the `kill` op, through the one hit path) is still in the
/// `people` op a game second later, dead, with how they died.
#[test]
fn dead_topside_people_stay_with_how_they_died() {
    let _p = pause();
    crate::opening::you_wake();
    let one = people()
        .into_iter()
        .find(|p| !is_bunker_person(p) && !["player", "bunker person"].contains(&p["def"].as_str().unwrap_or("")) && p["dead"] != true)
        .expect("a living topside person at the game's start");
    let id = one["id"].as_u64().expect("their id");
    op_with("kill", json!({"id": id}));
    step(crate::opening::SECOND);
    let them = op_with("people", json!({"ids": [id]})).as_array().and_then(|p| p.first().cloned()).unwrap_or_default();
    eprintln!("killed {id} {}: now {them}", one["def"]);
    assert!(them["dead"] == true, "the killed topside person {id} is not in the people op, dead: {them}");
    let how = them["deaths"].as_array().and_then(|d| d.last().cloned()).unwrap_or_default();
    assert!(how["of"].is_string(), "the dead topside person {id}'s memory does not say how they died: {them}");
}
