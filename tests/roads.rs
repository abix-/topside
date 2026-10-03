//! The world has roads (2D world generation: roads join the sites that have
//! buildings), read from the running game's `world` op: the road tiles in
//! the chunks made, the seed the world rolled from, and the sites a road
//! starts from.

use serde_json::{Value as Json, json};

use crate::endpoint::url;

fn world() -> Option<Json> {
    let resp = ureq::post(url()).send_json(json!({"op": "world", "args": {}})).ok()?;
    resp.into_body().read_json().ok()
}

#[test]
fn the_world_has_roads() {
    let world = world().unwrap_or_else(|| panic!("no topside game answering on {}; start it with scripts/build.ps1 run", url()));
    let result = &world["result"];
    let roads = result["roads"].as_u64().unwrap_or(0);
    let monuments = result["monuments"].as_array().cloned().unwrap_or_default();
    let with_buildings: Vec<&Json> = monuments.iter().filter(|m| m["buildings"].as_u64().unwrap_or(0) > 0).collect();
    eprintln!(
        "world op, tick {}: seed {}, road tiles {roads}, {} monuments, {} with buildings (where roads start): {}",
        result["tick"],
        result["seed"],
        monuments.len(),
        with_buildings.len(),
        with_buildings.iter().map(|m| format!("{} {}", m["kind"], m["at"])).collect::<Vec<_>>().join(", ")
    );
    assert!(with_buildings.is_empty() || roads > 0, "sites with buildings stand, but no road tile in the chunks made");
}
