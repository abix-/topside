//! The map each person carries (life.md "The map each person carries",
//! todo 11af) and the player's map on screen (authority.md
//! "Presentation", Diablo 2's automap, todo 13f), read from the running
//! game.

use crate::endpoint::{op, people};

#[test]
fn people_carry_maps_that_fill_as_they_look_around() {
    let list = people();
    let tiles: Vec<u64> = list.iter().filter(|p| p["dead"] == false).filter_map(|p| p["map_tiles"].as_u64()).collect();
    let with_map = tiles.iter().filter(|t| **t > 0).count();
    eprintln!("{} people alive, {with_map} with tiles on their map, most {:?}", tiles.len(), tiles.iter().max());
    assert!(!tiles.is_empty(), "the people op gives no map_tiles");
    assert!(with_map * 2 > tiles.len(), "fewer than half the people have seen anything");
}

/// The map drawn around the player shows only what their map holds, with
/// the player at its centre.
#[test]
fn the_map_shows_only_what_the_player_has_seen_around_them() {
    let view = op("view");
    let map = &view["map"];
    eprintln!("map: {map}");
    assert!(map.is_object(), "no map drawn: {view}");
    let player = &map["player"];
    let (cx, cy) = (map["centre"][0].as_i64().unwrap_or(i64::MAX), map["centre"][1].as_i64().unwrap_or(i64::MAX));
    let (px, py) = (player["tile"][0].as_i64().unwrap_or(0), player["tile"][1].as_i64().unwrap_or(0));
    // Drawn up to a quarter second ago: the player may have walked since.
    assert!((cx - px).abs() <= 5 && (cy - py).abs() <= 5, "the map is not centred on the player: {map}");
    let shown = map["shown"].as_u64().unwrap_or(0);
    assert!(shown > 0, "the map shows nothing: {map}");
    assert!(shown <= player["tiles_seen"].as_u64().unwrap_or(0), "the map shows more than the player has seen: {map}");
}
