//! Weapons are made in layers (design.md "What 2D changes"): people in
//! the running game hold a handle with a head on it, and a death they
//! dealt was of the damage their weapon's layers work out to.

use serde_json::{Value as Json, json};

use crate::endpoint::{op_with, people};

#[test]
fn people_hold_layered_weapons_and_their_hits_land_the_layers_damage() {
    let _p = crate::endpoint::pause();
    crate::opening::you_wake();
    let list = people();
    // One alive holding a layered weapon is told to attack the living
    // person nearest them (the `command` op), so a death by a layered
    // weapon comes within seconds of the game's start.
    let at = |p: &Json| (p["at"][0].as_f64().unwrap_or(f64::NAN), p["at"][1].as_f64().unwrap_or(f64::NAN));
    let armed = list
        .iter()
        .find(|p| p["dead"] == false && p["def"] != "player" && p["weapon"]["layers"].as_array().is_some_and(|l| !l.is_empty()))
        .cloned()
        .expect("someone alive holding a layered weapon at the game's start");
    let (ax, az) = at(&armed);
    let victim = list
        .iter()
        .filter(|p| p["dead"] == false && p["def"] != "player" && p["id"] != armed["id"])
        .min_by(|a, b| {
            let d = |p: &Json| (at(p).0 - ax).powi(2) + (at(p).1 - az).powi(2);
            d(a).total_cmp(&d(b))
        })
        .cloned()
        .expect("someone else alive");
    op_with("command", json!({"id": armed["id"], "do": "attack", "who": victim["id"]}));
    let dead = crate::opening::within(20, || {
        op_with("people", json!({"ids": [victim["id"]]})).as_array().and_then(|p| p.first().cloned()).is_some_and(|p| p["dead"] == true)
    });
    eprintln!("{} {} holding {} attacked {} {} at {:?} from {:?}: dead {dead}", armed["id"], armed["def"], armed["weapon"], victim["id"], victim["def"], at(&victim), (ax, az));
    let list = people();
    let weapon = |p: &Json| p["weapon"].clone();
    let layered: Vec<&Json> = list
        .iter()
        .filter(|p| p["dead"] == false && weapon(p)["layers"].as_array().is_some_and(|l| !l.is_empty()))
        .collect();
    eprintln!(
        "{} people, {} alive holding a weapon with layers\n{}",
        list.len(),
        layered.len(),
        layered
            .iter()
            .map(|p| format!("{} {}: {}", p["id"], p["def"].as_str().unwrap_or("?"), weapon(p)))
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(!layered.is_empty(), "nobody alive holds a weapon with layers");
    for p in &layered {
        assert!(weapon(p)["damage"].is_string(), "{} holds a weapon with no damage: {}", p["id"], weapon(p));
    }

    // Every death dealt by someone holding a layered weapon, against the
    // damage that weapon lands.
    let by_id = |id: &Json| list.iter().find(|p| p["id"] == *id);
    let mut checked = Vec::new();
    for p in &list {
        for death in p["deaths"].as_array().into_iter().flatten() {
            let Some(killer) = by_id(&death["by"]) else {
                continue;
            };
            let w = weapon(killer);
            if !w["layers"].as_array().is_some_and(|l| !l.is_empty()) {
                continue;
            }
            checked.push(format!(
                "{} killed by {} {} holding {}: of {}",
                p["id"], killer["id"], killer["def"].as_str().unwrap_or("?"), w, death["of"]
            ));
            assert_eq!(death["of"], w["damage"], "a death not of the killer's weapon: {}", checked.last().unwrap());
        }
    }
    eprintln!("{} deaths by a layered weapon\n{}", checked.len(), checked.join("\n"));
    assert!(!checked.is_empty(), "nobody has died by a layered weapon yet; read again after");
}
