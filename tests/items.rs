//! Items in layers (design.md "What 2D changes": every item is made in
//! layers), read from the running game's `items` op: every item lying in
//! the world with layers.

use serde_json::Value as Json;

use crate::endpoint::op;

fn items() -> Vec<Json> {
    op("items").as_array().cloned().unwrap_or_default()
}

/// The starting bunker's pipe shotgun lies with its barrel, grip, and
/// stock, and its numbers come from them: the pipe barrel's 20 m on the
/// body's 20, the barrel's and the stock's narrowing on its 10 degrees,
/// the grip's quickening on its 1.2 s.
#[test]
fn the_pipe_shotgun_lies_with_its_parts_and_their_numbers() {
    crate::opening::you_wake();
    let list = items();
    let gun = list
        .iter()
        .find(|i| i["item"] == "pipe shotgun" && i["layers"] == serde_json::json!(["pipe barrel", "wooden grip", "wooden stock"]))
        .unwrap_or_else(|| panic!("no pipe shotgun with its parts in the world: {list:?}"));
    eprintln!("{gun}");
    assert_eq!(gun["reach"].as_f64(), Some(40.0), "{gun}");
    assert_eq!(gun["spread_degrees"].as_f64(), Some(5.0), "{gun}");
    assert!((gun["delay"].as_f64().unwrap_or(0.0) - 1.1).abs() < 1e-4, "{gun}");
}

/// Every layered item lying in the world is drawn as its model with each
/// of its layers' models on it (`<item> <layer>`).
#[test]
fn a_layered_item_lying_in_the_world_shows_its_layers() {
    crate::opening::you_wake();
    let list = items();
    eprintln!("{} layered items lying in the world", list.len());
    assert!(!list.is_empty(), "no layered item lying in the world");
    for item in &list {
        let models: Vec<&str> = item["models"].as_array().into_iter().flatten().filter_map(Json::as_str).collect();
        let base = models.first().copied().unwrap_or_default();
        for layer in item["layers"].as_array().into_iter().flatten().filter_map(Json::as_str) {
            assert!(models.contains(&format!("{base} {layer}").as_str()), "{item}: no model for {layer}");
        }
    }
}
