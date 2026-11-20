//! What the running game's camera sees (authority.md "Presentation": one 3D
//! world in first person): the camera at the player's eyes, and meshes in
//! front of it, drawn cel shaded.

use crate::endpoint::op;
use serde_json::Value as Json;

/// The `view` op once `drawn` holds: paused, a tick at a time (the copy
/// is published at every stop), at most two game seconds, as the models
/// the load brought stream in over the frames after it.
fn view_once(drawn: impl Fn(&Json) -> bool) -> Json {
    let mut view = op("view");
    for _ in 0..120 {
        if drawn(&view) {
            break;
        }
        crate::endpoint::step(1);
        view = op("view");
    }
    view
}

#[test]
fn the_camera_sees_the_world_cel_shaded() {
    let _p = crate::endpoint::pause();
    crate::opening::you_wake();
    let view = view_once(|v| v["seen"].as_u64().unwrap_or(0) > 0 && v["standard"] == 0);
    eprintln!("view: {view}");
    let camera = &view["camera"];
    assert!(camera.is_object(), "no camera: {view}");
    // At the player's eyes above their feet, wherever they stand (they
    // wake in their room, a level below the ground).
    let feet = op("status")["player_feet"].as_f64().unwrap_or(0.0);
    let height = camera["at"][1].as_f64().unwrap_or(0.0) - feet;
    assert!((1.0..3.0).contains(&height), "the camera is {height:.2} m above the player's feet, not at eye height: {view}");
    assert!(view["seen"].as_u64().unwrap_or(0) > 0, "the camera sees no mesh: {view}");
    assert_eq!(view["standard"].as_u64(), Some(0), "meshes not cel shaded: {view}");
}

/// Buildings stand every level above the ground (todo 13a): floor tiles
/// up on the storeys, and the stair towers' steps and landings.
#[test]
fn buildings_stand_their_floors_above_the_ground_with_stairs() {
    let _p = crate::endpoint::pause();
    crate::opening::you_wake();
    let view = view_once(|v| v["floors_above"].as_u64().unwrap_or(0) > 0 && v["blocks_above"].as_u64().unwrap_or(0) > 0);
    let status = op("status");
    eprintln!("floors above {}, blocks above {}, the player's feet at {}", view["floors_above"], view["blocks_above"], status["player_feet"]);
    assert!(view["floors_above"].as_u64().unwrap_or(0) > 0, "no floor above the ground: {view}");
    assert!(view["blocks_above"].as_u64().unwrap_or(0) > 0, "no step or landing above the ground: {view}");
}
