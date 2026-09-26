//! Weapons are made in layers (design.md "What 2D changes"): people in
//! the running game hold a handle with a head on it, and a death they
//! dealt was of the damage their weapon's layers work out to.

use serde_json::Value as Json;

use crate::endpoint::people;

#[test]
fn people_hold_layered_weapons_and_their_hits_land_the_layers_damage() {
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
