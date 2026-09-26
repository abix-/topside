//! What the running game's camera sees (authority.md "Presentation": one 3D
//! world in first person): the camera at the player's eyes, and meshes in
//! front of it, drawn cel shaded.

use crate::endpoint::op;

#[test]
fn the_camera_sees_the_world_cel_shaded() {
    let view = op("view");
    eprintln!("view: {view}");
    let camera = &view["camera"];
    assert!(camera.is_object(), "no camera: {view}");
    let height = camera["at"][1].as_f64().unwrap_or(0.0);
    assert!((1.0..3.0).contains(&height), "the camera is not at eye height: {view}");
    assert!(view["seen"].as_u64().unwrap_or(0) > 0, "the camera sees no mesh: {view}");
    assert_eq!(view["standard"].as_u64(), Some(0), "meshes not cel shaded: {view}");
}

/// Buildings stand every level above the ground (todo 13a): floor tiles
/// up on the storeys, and the stair towers' steps and landings.
#[test]
fn buildings_stand_their_floors_above_the_ground_with_stairs() {
    let view = op("view");
    let status = op("status");
    eprintln!("floors above {}, blocks above {}, the player's feet at {}", view["floors_above"], view["blocks_above"], status["player_feet"]);
    assert!(view["floors_above"].as_u64().unwrap_or(0) > 0, "no floor above the ground: {view}");
    assert!(view["blocks_above"].as_u64().unwrap_or(0) > 0, "no step or landing above the ground: {view}");
}
