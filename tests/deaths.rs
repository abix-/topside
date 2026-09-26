//! Topside people live until the storm (design.md "The founding idea",
//! a livable world): people die by fights and the storm, never because
//! the world has nothing to eat or drink. Every death from each person's
//! own memory in the running game's `people` op; fails when a topside
//! person starved or died of thirst.

use std::collections::BTreeMap;

use crate::endpoint::{is_bunker_person, people};

#[test]
fn no_topside_person_starves_or_dies_of_thirst() {
    let list = people();
    let mut by_cause: BTreeMap<String, u32> = BTreeMap::new();
    let mut wasted = 0;
    for p in list.iter().filter(|p| !is_bunker_person(p) && p["def"] != "player") {
        for d in p["deaths"].as_array().into_iter().flatten() {
            let of = d["of"].as_str().unwrap_or("?");
            let killer = if d["by"].is_null() { "nobody" } else { "a person" };
            *by_cause.entry(format!("{} died of {of}, killed by {killer}", p["def"].as_str().unwrap_or("?"))).or_default() += 1;
            wasted += matches!(of, "starvation" | "dehydration") as u32;
        }
    }
    let lines: Vec<String> = by_cause.iter().map(|(k, n)| format!("{n:>4}  {k}")).collect();
    eprintln!("people op: {} people; topside deaths:\n{}", list.len(), lines.join("\n"));
    assert_eq!(wasted, 0, "{wasted} topside people starved or died of thirst:\n{}", lines.join("\n"));
}
