//! The small world's chunks are made (design.md "The small world
//! first": all of it made, so nothing depends on where people walk),
//! read from the running game's `world` op.

use serde_json::{Value as Json, json};

const URL: &str = "http://127.0.0.1:15703/topside";

fn world() -> Option<Json> {
    let resp = ureq::post(URL).send_json(json!({"op": "world", "args": {}})).ok()?;
    resp.into_body().read_json().ok()
}

#[test]
fn every_chunk_of_the_small_world_is_made() {
    let world = world().unwrap_or_else(|| panic!("no topside game answering on {URL}; start it with scripts/build.ps1 run"));
    let chunks = &world["result"]["chunks"];
    let made = chunks["made"].as_u64().unwrap_or(0);
    assert!(made > 0, "the game has made chunks: {chunks}");
    let missing = chunks["missing"]
        .as_u64()
        .unwrap_or_else(|| panic!("the running world has no edge, so it is not the small world: {chunks}"));
    assert_eq!(missing, 0, "every chunk of the small world and the ring past its edge is made: {chunks}");
}
