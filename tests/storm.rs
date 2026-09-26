//! The storm (design.md "The storm"): it hurts whoever is in the open,
//! the bunker people shelter, and the world shifts to a new one with new
//! topside people, read from the running game's `world` and `people` ops
//! after the first storm.

use crate::endpoint::{is_bunker_person, op, people};

#[test]
fn the_storm_hurts_the_open_bunker_people_shelter_and_the_world_shifts() {
    let w = op("world");
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
