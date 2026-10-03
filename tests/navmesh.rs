//! The navigation mesh (docs/pathing.md "A navigation mesh"), proved on
//! the bunker: the mesh built from the colliders gives a way from the
//! entrance down the stairwell to the player's room, two levels down.

use std::time::Duration;

use serde_json::{Value as Json, json};

use crate::endpoint::{op, op_with};

/// The way from `from` to `to` ([x, height, z]), asked until the mesh is
/// built and answers, at most `secs` real seconds. Only this ask's own
/// answer counts: the game keeps the last answer until the next is worked
/// out, so a slow frame hands back the one asked before (another piece, or
/// the test before); its echoed `from` and `to` must be this ask's, as the
/// game holds them (f32).
fn way(from: [f64; 3], to: [f64; 3], secs: u64) -> Json {
    let same = |echoed: &Json, asked: [f64; 3]| {
        echoed.as_array().is_some_and(|e| e.len() == 3 && e.iter().zip(asked).all(|(e, a)| e.as_f64().map(|e| e as f32) == Some(a as f32)))
    };
    let mut own = json!({"error": format!("no answer of its own to {from:?} -> {to:?} in {secs} s")});
    for _ in 0..secs * 2 {
        op_with("nav_path", json!({"from": from, "to": to}));
        std::thread::sleep(Duration::from_millis(500));
        let answer = op("nav_path");
        if !(same(&answer["from"], from) && same(&answer["to"], to)) {
            continue;
        }
        let done = answer["path"].is_array();
        own = answer;
        if done {
            break;
        }
    }
    own
}

/// Each piece of the way down on its own, to see which is joined: the
/// bunker point (0, 0); the stairwell's middle x 5.2, its landings at z 0,
/// its half landings at z -5 (modforge monument.rs's stair tower).
#[test]
fn each_piece_of_the_way_down_is_joined() {
    let pieces = [
        ("entrance to the stairwell's landing", [0.0, 0.0, 0.0], [5.2, 0.0, 0.0]),
        ("landing down to the half landing", [5.2, 0.0, 0.0], [5.2, -1.5, -5.0]),
        ("half landing down to level 1's landing", [5.2, -1.5, -5.0], [5.2, -3.0, 0.0]),
        ("level 1's landing into its room", [5.2, -3.0, 0.0], [0.0, -3.0, 0.0]),
        ("level 1's landing down to the half landing", [5.2, -3.0, 0.0], [5.2, -4.5, -5.0]),
        ("half landing down to level 2's landing", [5.2, -4.5, -5.0], [5.2, -6.0, 0.0]),
        ("level 2's landing into the player's room", [5.2, -6.0, 0.0], [0.0, -6.0, 0.0]),
        // The same, the landings 1 m inside the chunk the flights are in
        // (the landing lies across the chunk edge at z 0).
        ("in its chunk: landing down to the half landing", [5.2, 0.0, -1.0], [5.2, -1.5, -5.0]),
        ("in its chunk: half landing down to level 1's landing", [5.2, -1.5, -5.0], [5.2, -3.0, -1.0]),
        ("level 1's landing across the chunk edge", [5.2, -3.0, -1.0], [5.2, -3.0, 1.0]),
        ("the ground across the chunk edge", [-10.0, 0.0, -1.0], [-10.0, 0.0, 1.0]),
    ];
    let mut broken = Vec::new();
    for (name, from, to) in pieces {
        let answer = way(from, to, 10);
        let ok = answer["path"].as_array().is_some_and(|p| !p.is_empty());
        eprintln!("{name}: {}", if ok { "joined".to_string() } else { answer["error"].to_string() });
        if !ok {
            broken.push(name);
        }
    }
    assert!(broken.is_empty(), "not joined: {broken:?}");
}

/// One mesh per chunk (docs/pathing.md): every chunk made has its mesh,
/// none left waiting.
#[test]
fn every_chunk_made_has_its_mesh() {
    let mut answer = Json::Null;
    for _ in 0..60 {
        op_with("nav_path", json!({"from": [0.0, 0.0, 0.0], "to": [0.0, 0.0, 0.0]}));
        std::thread::sleep(Duration::from_millis(500));
        answer = op("nav_path");
        if answer["waiting"] == 0 && answer["chunk_meshes"].as_u64().is_some_and(|n| n > 0) {
            break;
        }
    }
    let made = op("world")["chunks"].clone();
    eprintln!("chunk meshes {}, waiting {}; the world's chunks: {made}", answer["chunk_meshes"], answer["waiting"]);
    assert_eq!(answer["waiting"], 0, "chunk meshes still waiting");
    assert!(answer["chunk_meshes"].as_u64().is_some_and(|n| n >= 1024), "fewer chunk meshes than the small world's 1024 chunks");
}

/// The chunks' meshes are one connected world: a way from the player's
/// bunker to the bunker 300 m east, across about ten chunk edges.
#[test]
fn a_way_runs_across_the_world_from_bunker_to_bunker() {
    let answer = way([0.0, 0.0, 12.0], [300.0, 0.0, 12.0], 30);
    let path = answer["path"].as_array().cloned().unwrap_or_default();
    eprintln!("{} waypoints; error {}; first {:?}, last {:?}", path.len(), answer["error"], path.first(), path.last());
    assert!(!path.is_empty(), "no way across the world: {}", answer["error"]);
    let last = path.last().and_then(|p| p[0].as_f64()).unwrap_or(0.0);
    assert!(last > 295.0, "the way stops at x {last}");
}

#[test]
fn the_mesh_gives_a_way_down_the_stairwell_to_the_players_room() {
    // The bunker point (0, 0): its entrance floor, and the player's room
    // 6 m down.
    let answer = way([0.0, 0.0, 0.0], [0.0, -6.0, 0.0], 20);
    eprintln!("{answer:#}");
    let path = answer["path"].as_array().cloned().unwrap_or_default();
    assert!(!path.is_empty(), "no way down: {}", answer["error"]);
    let y = |p: &Json| p[1].as_f64().unwrap_or(0.0);
    let last = path.last().cloned().unwrap_or(Json::Null);
    assert!((y(&last) + 6.0).abs() < 0.5, "the way ends at height {}, not 6 m down", y(&last));
    assert!(path.iter().any(|p| p[0].as_f64().is_some_and(|x| x > 3.4)), "the way never goes into the stairwell east of the rooms");
}
