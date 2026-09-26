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
