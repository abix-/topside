//! Topside people live until the storm (design.md "The founding idea",
//! a livable world): people die by fights and the storm, never because
//! the world has nothing to eat or drink. Every death the running game's
//! `world` op reports; fails when a topside person starved or died of
//! thirst.

use std::collections::BTreeMap;

use serde_json::Value as Json;

use crate::endpoint::op;

#[test]
fn no_topside_person_starves_or_dies_of_thirst() {
    let w = op("world");
    let deaths = w["deaths"].as_array().cloned().unwrap_or_default();
    let topside: Vec<&Json> = deaths
        .iter()
        .filter(|d| !d["faction"].as_str().is_some_and(|f| f.starts_with("bunker")) && d["def"] != "player")
        .collect();
    let mut by_cause: BTreeMap<String, u32> = BTreeMap::new();
    for d in &topside {
        let killer = if d["by"].is_null() { "nobody" } else { "a person" };
        *by_cause
            .entry(format!("{} died of {}, killed by {killer}", d["def"].as_str().unwrap_or("?"), d["of"].as_str().unwrap_or("?")))
            .or_default() += 1;
    }
    let lines: Vec<String> = by_cause.iter().map(|(k, n)| format!("{n:>4}  {k}")).collect();
    eprintln!(
        "world op, tick {}, storms {}: {} deaths, {} topside\n{}",
        w["tick"],
        w["storms"],
        deaths.len(),
        topside.len(),
        lines.join("\n")
    );
    let wasted: Vec<&&Json> = topside
        .iter()
        .filter(|d| matches!(d["of"].as_str(), Some("starvation" | "dehydration")))
        .collect();
    assert!(wasted.is_empty(), "{} topside people starved or died of thirst:\n{}", wasted.len(), lines.join("\n"));
}
