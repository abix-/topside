//! Talking (operator, 2026-09-27), read from the running game: The Tap
//! opens with Dell telling the player of the tap, and the `talk` op brings
//! a talk now through the one talk path. Each takes seconds.

use crate::endpoint::{op, op_with};

/// The Tap's cast part's id, and the tap's key, from the `world` op.
fn dell_and_the_tap(w: &serde_json::Value) -> Option<(u64, u64)> {
    let dell = w["cast"].as_array()?.iter().find(|p| p["part"] == "Dell")?["id"].as_u64()?;
    let tap = w["things"].as_array()?.iter().find(|t| t["name"] == "tap")?["key"].as_u64()?;
    Some((dell, tap))
}

/// Read the `player` op until `done` holds, at most 5 real seconds.
fn player_until(done: impl Fn(&serde_json::Value) -> bool) -> serde_json::Value {
    let mut p = op("player");
    for _ in 0..20 {
        if done(&p) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
        p = op("player");
    }
    p
}

fn told_by(p: &serde_json::Value, key: u64) -> Option<u64> {
    p["knows"].as_array()?.iter().find(|k| k["key"].as_u64() == Some(key))?["told_by"].as_u64()
}

/// How The Tap opens: Dell tells the player of the tap. The player is put
/// a metre from Dell (the `use` op) in case they walked off.
#[test]
fn dell_tells_the_player_of_the_tap() {
    let w = op("world");
    if w["episode"] != "The Tap" {
        eprintln!("the episode is {}, not The Tap; its opening is not read this run", w["episode"]);
        return;
    }
    let (dell, tap) = dell_and_the_tap(&w).expect("The Tap has Dell and the tap");
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    let at = op_with("npc", serde_json::json!({"id": dell}))["at"].clone();
    eprintln!("use op answered {}", op_with("use", serde_json::json!({"id": player, "at": at})));
    let p = player_until(|p| told_by(p, tap).is_some());
    eprintln!("player {player}: tap told by {:?}; heard {}", told_by(&p, tap), p["heard"]);
    assert_eq!(told_by(&p, tap), Some(dell), "the player knows the tap, told by Dell");
}

/// Use on a person opens the talk panel: its topics are what the player
/// knows (the tap, told by Dell) and a threat. The `use` op puts the
/// player a metre from Dell and uses there; used again if another test
/// closed it meanwhile.
#[test]
fn use_on_a_person_opens_the_talk_panel() {
    let w = op("world");
    let Some((dell, _)) = dell_and_the_tap(&w) else {
        eprintln!("the episode is {}, with no Dell; not read this run", w["episode"]);
        return;
    };
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    let mut talk = serde_json::Value::Null;
    for _ in 0..3 {
        let at = op_with("npc", serde_json::json!({"id": dell}))["at"].clone();
        op_with("use", serde_json::json!({"id": player, "at": at}));
        for _ in 0..12 {
            std::thread::sleep(std::time::Duration::from_millis(250));
            talk = op("hud")["talk"].clone();
            if talk["open"] == true {
                break;
            }
        }
        if talk["open"] == true {
            break;
        }
    }
    eprintln!("talk panel: {talk}");
    assert_eq!(talk["open"], true, "Use on Dell opened no talk panel");
    assert_eq!(talk["to"].as_u64(), Some(dell), "the panel talks to Dell");
    let topics: Vec<&str> = talk["topics"].as_array().map_or(Vec::new(), |t| t.iter().filter_map(|t| t.as_str()).collect());
    assert!(topics.contains(&"Tell them of the tap"), "a topic to tell of the tap: {topics:?}");
    assert!(topics.contains(&"Threaten them"), "a topic to threaten: {topics:?}");
}

/// The `talk` op: Dell tells the player of the tap again; both remember.
#[test]
fn the_talk_op_brings_a_talk_now() {
    let w = op("world");
    let Some((dell, tap)) = dell_and_the_tap(&w) else {
        eprintln!("the episode is {}, with no Dell and tap; not read this run", w["episode"]);
        return;
    };
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    let at = op_with("npc", serde_json::json!({"id": dell}))["at"].clone();
    op_with("use", serde_json::json!({"id": player, "at": at}));
    std::thread::sleep(std::time::Duration::from_millis(500));
    let heard = |p: &serde_json::Value| p["heard"].as_array().map_or(0, |h| h.iter().filter(|h| h["by"].as_u64() == Some(dell)).count());
    let before = heard(&op("player"));
    let asked = op_with("talk", serde_json::json!({"id": dell, "to": player, "said": {"Tell": {"key": tap}}}));
    eprintln!("talk op answered {asked}");
    let p = player_until(|p| heard(p) > before);
    eprintln!("the player heard Dell {before} times before, {} after: {}", heard(&p), p["heard"]);
    assert!(heard(&p) > before, "Dell told the player through the talk op, and the player heard nothing");
}
