//! People drawn in 3D (design.md: people built from pieces), read from
//! what the running game reports each person's model shows: their body
//! and pieces, what is in their hand with its layers, and the walk while
//! they move.

use serde_json::Value as Json;

use crate::endpoint::people;

#[test]
fn people_show_their_pieces_hold_their_weapon_and_walk() {
    // A game second from the game's start: everyone drawn, some walking.
    let _p = crate::endpoint::pause();
    crate::opening::you_wake();
    crate::endpoint::step(crate::opening::SECOND);
    let list = people();
    let drawn: Vec<&Json> = list.iter().filter(|p| p["dead"] == false && p["drawn"].is_object()).collect();
    let shows = |p: &Json, layer: &str| p["drawn"]["layers"].as_array().is_some_and(|l| l.iter().any(|x| x == layer));
    let pieced = drawn.iter().filter(|p| shows(p, "body") && p["drawn"]["layers"].as_array().is_some_and(|l| l.len() > 1)).count();
    let layered_hands = drawn
        .iter()
        .filter(|p| p["drawn"]["held"].as_str().is_some_and(|h| h.contains('|')))
        .count();
    let walking = drawn.iter().filter(|p| p["drawn"]["walking"] == true).count();
    let wearing = drawn
        .iter()
        .filter(|p| shows(p, "road sign vest"))
        .count();
    eprintln!(
        "{} people drawn: {pieced} show their body and a piece, {layered_hands} hold a layered weapon, {walking} walking, {wearing} wearing the vest",
        drawn.len()
    );
    if let Some(p) = drawn.first() {
        eprintln!("for one: {}", p["drawn"]);
    }
    assert!(!drawn.is_empty(), "nobody is drawn (the game runs without a window?)");
    assert!(pieced > 0, "nobody shows their body and a piece of their look");
    assert!(layered_hands > 0, "nobody holds a weapon with its layers in their hand");
    assert!(walking > 0, "nobody's walk is playing");
}
