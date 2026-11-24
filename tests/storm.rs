//! The storm (design.md "The storm"): it hurts whoever is in the open,
//! the bunker people shelter, and the world shifts to a new one with new
//! topside people, read from the running game's `world` and `people` ops
//! after the first storm.

use crate::endpoint::{is_bunker_person, op, people};

/// The `storm` op brings the next storm now: the sky warns on the game's
/// next step (the `world` op publishes once a game second, so it is read
/// again until it has published since the request).
#[test]
fn the_storm_op_brings_the_storm_now() {
    crate::opening::you_wake();
    let before = op("world");
    let asked = op("storm");
    eprintln!("storm op answered {asked}; before it: tick {}, storm {}, {} storms", before["tick"], before["storm"], before["storms"]);
    let mut after = op("world");
    for _ in 0..50 {
        if after["tick"].as_u64() > before["tick"].as_u64() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        after = op("world");
    }
    eprintln!("after it: tick {}, storm {}, {} storms", after["tick"], after["storm"], after["storms"]);
    assert!(after["tick"].as_u64() > before["tick"].as_u64(), "the world op did not publish again within 5 seconds");
    assert_ne!(after["storm"], "calm", "the sky does not warn and no storm hits after the storm op");
}

#[test]
fn the_storm_hurts_the_open_bunker_people_shelter_and_the_world_shifts() {
    // From just before the storm shifts the world, stepped to the shift.
    let _p = crate::endpoint::pause();
    crate::saves::storm_about_to_shift_the_world();
    let before = op("world")["storms"].as_u64().unwrap_or(0);
    for _ in 0..10 {
        if op("world")["storms"].as_u64().unwrap_or(0) > before {
            break;
        }
        crate::endpoint::step(crate::opening::SECOND);
    }
    // The new world's monuments are listed as their chunks are made, a
    // set number a tick: a second at a time, at most ten.
    let mut w = op("world");
    for _ in 0..10 {
        if w["monuments"].as_array().is_some_and(|m| !m.is_empty()) {
            break;
        }
        crate::endpoint::step(crate::opening::SECOND);
        w = op("world");
    }
    let storms = w["storms"].as_u64().unwrap_or(0);
    let list = people();
    // Deaths of the storm, from each person's own memory.
    let storm_deaths = |p: &serde_json::Value| p["deaths"].as_array().map_or(0, |d| d.iter().filter(|d| d["of"] == "storm").count());
    let of_storm: usize = list.iter().map(storm_deaths).sum();
    let bunker_of_storm: usize = list.iter().filter(|p| is_bunker_person(p)).map(storm_deaths).sum();
    let bunker_people = list.iter().filter(|p| is_bunker_person(p)).count();
    let topside = list.iter().filter(|p| !is_bunker_person(p) && p["dead"] != true).count();
    let monuments = w["monuments"].as_array().map_or(0, Vec::len);
    eprintln!(
        "world op, tick {}: {storms} storms; {} died of the storm, {bunker_of_storm} of them bunker people; {bunker_people} bunker people, {topside} topside people alive, {monuments} monuments",
        w["tick"],
        of_storm
    );
    assert!(storms > 0, "no storm has come yet (tick {}); read again after the first", w["tick"]);
    assert!(of_storm > 0, "the storm hurt nobody in the open");
    assert!(bunker_of_storm * 2 < bunker_people, "{bunker_of_storm} of {bunker_people} bunker people died in the storm instead of sheltering");
    assert!(topside > 0, "the new world has no topside people");
    assert!(monuments > 0, "the new world has no monuments");
}
