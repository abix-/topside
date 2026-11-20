//! Everybody has a name (operator, 2026-09-27), and the player's bunker
//! holds the player and Roxanne, read from the running game's `people` op.

use crate::endpoint::people;

#[test]
fn everybody_has_a_name() {
    crate::opening::you_wake();
    let list = people();
    let unnamed: Vec<_> = list.iter().filter(|p| p["name"].as_str().is_none_or(str::is_empty)).map(|p| p["id"].clone()).collect();
    let some: Vec<_> = list.iter().take(5).map(|p| p["name"].clone()).collect();
    eprintln!("{} people; some of them: {some:?}; unnamed: {unnamed:?}", list.len());
    assert!(!list.is_empty(), "the people op lists nobody");
    assert!(unnamed.is_empty(), "people with no name: {unnamed:?}");
}

#[test]
fn roxanne_lives_in_the_players_bunker() {
    crate::opening::you_wake();
    let list = people();
    let roxanne: Vec<_> = list.iter().filter(|p| p["name"] == "Roxanne" && p["faction"] == "player").collect();
    eprintln!("Roxanne: {:?}", roxanne.iter().map(|p| (p["id"].clone(), p["dead"].clone())).collect::<Vec<_>>());
    assert_eq!(roxanne.len(), 1, "exactly one Roxanne of the player's bunker");
}
