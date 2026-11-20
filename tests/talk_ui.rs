//! The talk box played the way the player plays it (operator,
//! 2026-09-27: see what the player sees, drive every step): keys through
//! the `key` op, a choice said by its number key, a screenshot at each
//! step in logs/talk_*.png to be looked at. Takes seconds.

use std::time::Duration;

use crate::endpoint::{op, op_with};

pub fn shot(name: &str) {
    op_with("screenshot", serde_json::json!({"name": name}));
    std::thread::sleep(Duration::from_millis(400));
}

/// The `hud` op's talk panel once it has published again (once a game
/// second), at most 3 real seconds.
pub fn talk_after(done: impl Fn(&serde_json::Value) -> bool) -> serde_json::Value {
    let mut talk = op("hud")["talk"].clone();
    for _ in 0..12 {
        if done(&talk) {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
        talk = op("hud")["talk"].clone();
    }
    talk
}

/// The talk panel once it has closed by itself, at most 8 real seconds
/// (a conversation stays 4 real seconds after its last line, to be read).
pub fn closed_within_8s() -> serde_json::Value {
    let mut talk = op("hud")["talk"].clone();
    for _ in 0..32 {
        if talk["open"] == false {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
        talk = op("hud")["talk"].clone();
    }
    talk
}

/// The player's lines heard from `id` so far.
fn heard_from(id: u64) -> Vec<String> {
    op("player")["heard"]
        .as_array()
        .map_or(Vec::new(), |h| h.iter().filter(|h| h["by"].as_u64() == Some(id)).filter_map(|h| h["said"].as_str().map(str::to_string)).collect())
}

/// Look at someone and press E (episodes.md "Conversations"): the prompt
/// names them; with nothing to talk about they say one short line and the
/// conversation ends by itself. Played on a topside stranger.
#[test]
fn e_on_someone_with_nothing_to_say_gets_one_line_and_ends() {
    // Paused and stepped, so each read is of the copy published at a stop.
    let _p = crate::endpoint::pause();
    crate::opening::you_wake();
    let people = crate::endpoint::people();
    // Someone not out to kill the player and with nothing to be told: a
    // person of another bunker (a stranger of no faction can be asked to
    // stay with the player's bunker, so they wait for the player's word).
    // Nobody the episode cast: a part has lines of its own.
    let cast: Vec<u64> = op("world")["cast"].as_array().into_iter().flatten().filter_map(|c| c["id"].as_u64()).collect();
    let Some(stranger) = people.iter().find(|p| crate::endpoint::is_bunker_person(p) && p["dead"] != true && !p["id"].as_u64().is_some_and(|id| cast.contains(&id))) else {
        panic!("no living person of another bunker this run");
    };
    let (id, name) = (stranger["id"].as_u64().unwrap_or(0), stranger["name"].as_str().unwrap_or_default().to_string());
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    let before = heard_from(id).len();
    // Stand by them, looking at them: the use op also presses E.
    op_with("use", serde_json::json!({"id": player, "person": id}));
    let want = format!("[E] Talk to {name}");
    let mut prompt = serde_json::Value::Null;
    let mut open = serde_json::Value::Null;
    // A tick at a time until the prompt names them, at most a game second.
    for _ in 0..crate::opening::SECOND {
        crate::endpoint::step(1);
        prompt = op("hud")["prompt"].clone();
        if open.is_null() && op("hud")["talk"]["open"] == true {
            open = op("hud")["talk"].clone();
        }
        if prompt.as_str() == Some(want.as_str()) {
            break;
        }
    }
    shot("stranger_1_talking");
    let said = crate::opening::within(3, || heard_from(id).len() > before);
    let heard = heard_from(id);
    eprintln!("{name} ({id}): panel {open}; prompt {prompt}; heard {heard:?}");
    assert_eq!(prompt.as_str(), Some(want.as_str()), "the prompt does not name {name}");
    assert!(said, "{name} said nothing");
    crate::opening::within(8, || op("hud")["talk"]["open"] == false);
    let closed = op("hud")["talk"].clone();
    shot("stranger_2_ended");
    eprintln!("after their line: {closed}");
    assert_eq!(closed["open"], false, "the conversation with {name} never ended");
}

