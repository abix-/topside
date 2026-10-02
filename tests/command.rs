//! Commands and replays (authority.md "Commands and replays"): the player
//! given plain orders over the `command` op, carried out with the brain's
//! own actions in the running game, with no key or mouse; a replay saved
//! and played gives the same commands again.

use std::time::Duration;

use serde_json::{Value as Json, json};

use crate::endpoint::{op, op_with, people, why_stopped};

fn player_at() -> (f64, f64) {
    let s = op("status");
    (s["player"][0].as_f64().unwrap_or(0.0), s["player"][1].as_f64().unwrap_or(0.0))
}

fn at(p: &Json) -> Option<(f64, f64)> {
    Some((p["at"][0].as_f64()?, p["at"][1].as_f64()?))
}

fn apart(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// The nearest living person farther than `beyond` metres who is no one's
/// enemy by faction: a bunker person or a stranger of no faction; with
/// `still`, only one standing still this step (everyone walks at one
/// speed, so a person walking away is never caught).
fn someone_near(beyond: f64, still: bool) -> Option<(u64, String, (f64, f64))> {
    let me = player_at();
    let standing = |p: &Json| p["actions"].as_array().is_some_and(|a| a.iter().all(|s| s.as_str().is_none_or(|s| !s.starts_with("Move") || s.contains("x: 0.0, y: 0.0"))));
    let mut near: Vec<(u64, String, (f64, f64), f64)> = people()
        .into_iter()
        .filter(|p| p["dead"] != true && (crate::endpoint::is_bunker_person(p) || p["faction"].is_null()))
        .filter(|p| !still || standing(p))
        .filter_map(|p| Some((p["id"].as_u64()?, p["name"].as_str().unwrap_or_default().to_string(), at(&p)?, p["feet"].as_f64()?)))
        .filter(|(_, _, there, _)| apart(*there, me) > beyond)
        .collect();
    near.sort_by(|a, b| apart(a.2, me).total_cmp(&apart(b.2, me)));
    // The nearest the navigation mesh has a way to (someone can stand where
    // no way reaches: on a roof, in a shut building).
    near.into_iter().take(8).find(|(_, _, there, feet)| way_to((there.0, *feet, there.1))).map(|(id, name, there, _)| (id, name, there))
}

/// Whether the navigation mesh has a way from where the player stands to
/// `to` (x, height, z), asked of the `nav_path` op.
fn way_to(to: (f64, f64, f64)) -> bool {
    let (px, pz) = player_at();
    let pf = op("status")["player_feet"].as_f64().unwrap_or(0.0);
    op_with("nav_path", json!({"from": [px, pf, pz], "to": [to.0, to.1, to.2]}));
    std::thread::sleep(Duration::from_millis(150));
    op("nav_path")["path"].is_array()
}

/// Real seconds to walk `metres` and then some: the player walks
/// `player::WALK_SPEED`, 15 m a second.
fn walk_secs(metres: f64) -> u64 {
    (metres / 10.0) as u64 + 10
}

/// Up to `secs` real seconds for `done`, a quarter second apart.
fn within(secs: u64, done: impl Fn() -> bool) -> bool {
    for _ in 0..secs * 4 {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    done()
}

#[test]
fn talk_to_walks_the_player_over_and_the_talk_box_opens() {
    crate::endpoint::player_alive();
    // No commands left from an earlier test.
    op_with("replay", json!({"stop": true}));
    // Farther than talking distance, so the player walks over first.
    let (id, name, there) = someone_near(5.0, true).expect("a living bunker person or stranger standing somewhere");
    let from = player_at();
    eprintln!("talk to {name} ({id}), {:.1} m away", apart(there, from));
    let given = op_with("command", json!({"do": "talk_to", "who": id}));
    eprintln!("given: {given}");
    let opened = within(walk_secs(apart(there, from)), || {
        let hud = op("hud");
        let them = people().into_iter().find(|p| p["id"].as_u64() == Some(id)).and_then(|p| at(&p));
        eprintln!("  the player {:?}, {name} {them:?}, looking at: {}", player_at(), hud["prompt"]);
        hud["talk"]["to"].as_u64() == Some(id)
    });
    crate::talk_ui::shot("command_talk_to");
    let talk = op("hud")["talk"].clone();
    eprintln!("the player now at {:?}, walked {:.1} m; the talk box: {talk}", player_at(), apart(player_at(), from));
    if !opened && let Some(p) = people().into_iter().find(|p| p["id"].as_u64() == Some(id)) {
        let (x, z) = at(&p).unwrap_or((0.0, 0.0));
        why_stopped((x, p["feet"].as_f64().unwrap_or(0.0), z));
    }
    op_with("key", json!({"key": "Escape"}));
    assert!(opened, "talk to {name} never opened the talk box with them");
}

#[test]
fn move_to_walks_the_player_there() {
    crate::endpoint::player_alive();
    // No commands left from an earlier test.
    op_with("replay", json!({"stop": true}));
    // Where someone stands is ground someone can stand on, at their height
    // (the player wakes two levels down, so most ways climb the stairs).
    let (id, name, there) = someone_near(2.0, false).expect("a living bunker person or stranger somewhere");
    let feet = people().into_iter().find(|p| p["id"].as_u64() == Some(id)).and_then(|p| p["feet"].as_f64()).expect("the people op reports their feet");
    let from = player_at();
    let from_feet = op("status")["player_feet"].as_f64().unwrap_or(0.0);
    eprintln!("move to where {name} ({id}) stands, {there:?} at height {feet:.2}, {:.1} m from {from:?} at height {from_feet:.2}", apart(there, from));
    op_with("command", json!({"do": "move_to", "x": there.0, "y": feet, "z": there.1}));
    let arrived = within(walk_secs(apart(there, from) + (feet - from_feet).abs() * 4.0), || {
        apart(player_at(), there) <= 2.0 && op("status")["player_feet"].as_f64().is_some_and(|f| (f - feet).abs() < 0.5)
    });
    crate::talk_ui::shot("command_move_to");
    eprintln!("the player now at {:?}, height {}, {:.1} m from the point", player_at(), op("status")["player_feet"], apart(player_at(), there));
    if !arrived {
        why_stopped((there.0, feet, there.1));
    }
    assert!(arrived, "the player never reached {there:?} at height {feet:.2}");
}

fn replay(name: &str) -> Option<Json> {
    let path = format!("{}/../replays/{name}.json", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok())
}

#[test]
fn a_saved_replay_played_again_gives_the_same_commands() {
    crate::endpoint::player_alive();
    let here = player_at();
    // A move to where the player stands: given, and done at once.
    let feet = op("status")["player_feet"].as_f64().unwrap_or(0.0);
    op_with("command", json!({"do": "move_to", "x": here.0, "y": feet, "z": here.1}));
    std::thread::sleep(Duration::from_millis(300));
    for name in ["command test", "command test again"] {
        let _ = std::fs::remove_file(format!("{}/../replays/{name}.json", env!("CARGO_MANIFEST_DIR")));
    }
    op_with("replay", json!({"save": "command test"}));
    let saved = within(3, || replay("command test").is_some());
    let first = replay("command test").unwrap_or(Json::Null);
    let given = first["given"].as_array().cloned().unwrap_or_default();
    eprintln!("saved: {saved}, {} commands, seed {}", given.len(), first["seed"]);
    assert!(saved, "replays/command test.json was never written");
    let last = given.last().cloned().unwrap_or(Json::Null);
    assert_eq!(last["command"]["do"], "move_to", "the move just given is not the last command saved: {last}");
    // Played: the replay's first command is given again on the next step.
    op_with("replay", json!({"play": "command test"}));
    std::thread::sleep(Duration::from_millis(300));
    op_with("replay", json!({"save": "command test again"}));
    let again = within(3, || replay("command test again").is_some_and(|r| r["given"].as_array().is_some_and(|g| g.len() > given.len())));
    let after = replay("command test again").unwrap_or(Json::Null);
    let replayed = after["given"][given.len()].clone();
    // The rest of the replay would give every command of this run again,
    // to the tests after this one: stopped.
    op_with("replay", json!({"stop": true}));
    eprintln!("the first command given again: {replayed}; the replay's first: {}", given[0]);
    assert!(again, "playing the replay gave no command");
    assert_eq!((&replayed["who"], &replayed["command"]), (&given[0]["who"], &given[0]["command"]), "the replay gave a different command");
}
