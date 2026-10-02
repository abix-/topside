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
        eprintln!("use op answered {}", op_with("use", serde_json::json!({"id": player, "person": dell})));
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
    crate::endpoint::player_alive();
    let w = op("world");
    let Some((dell, _)) = dell_and_the_tap(&w) else {
        eprintln!("the episode is {}, with no Dell; not read this run", w["episode"]);
        return;
    };
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    let mut talk = serde_json::Value::Null;
    for _ in 0..3 {
                op_with("use", serde_json::json!({"id": player, "person": dell}));
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
    assert!(
        !topics.iter().any(|t| t.starts_with("Tell them of") || t.starts_with("Ask about") || *t == "Threaten them"),
        "the always-on topics are gone (episodes.md \"Conversations\"): {topics:?}"
    );
}

/// The `talk` op: Dell tells the player of the tap again; both remember.
#[test]
fn the_talk_op_brings_a_talk_now() {
    crate::endpoint::player_alive();
    let w = op("world");
    let Some((dell, tap)) = dell_and_the_tap(&w) else {
        eprintln!("the episode is {}, with no Dell and tap; not read this run", w["episode"]);
        return;
    };
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
        op_with("use", serde_json::json!({"id": player, "person": dell}));
    std::thread::sleep(std::time::Duration::from_millis(500));
    let heard = |p: &serde_json::Value| p["heard"].as_array().map_or(0, |h| h.iter().filter(|h| h["by"].as_u64() == Some(dell)).count());
    let before = heard(&op("player"));
    let asked = op_with("talk", serde_json::json!({"id": dell, "to": player, "said": {"Tell": {"key": tap}}}));
    eprintln!("talk op answered {asked}");
    let p = player_until(|p| heard(p) > before);
    eprintln!("the player heard Dell {before} times before, {} after: {}", heard(&p), p["heard"]);
    assert!(heard(&p) > before, "Dell told the player through the talk op, and the player heard nothing");
}

/// A line stays up as long as it takes to read (modforge
/// `talk::reading_secs`: about 15 characters a second, never under 1.5 s):
/// while a line of theirs still has reading time left, no choices show and
/// they say nothing more; the time it starts with is its length's.
#[test]
fn each_line_stays_long_enough_to_read_before_what_comes_after() {
    crate::endpoint::player_alive();
    op_with("replay", serde_json::json!({"stop": true}));
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    let them = crate::endpoint::people()
        .into_iter()
        .find(|p| p["name"] == "Roxanne" && p["dead"] != true)
        .and_then(|p| p["id"].as_u64())
        .expect("Roxanne, alive");
    let reading = |words: &str| (words.chars().count() as f64 / 15.0).max(1.5);
    let (mut theirs_read, mut checked, mut first_seen) = (0, 0, std::collections::HashMap::<String, f64>::new());
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(25) {
        let talk = op("hud")["talk"].clone();
        if talk["to"].as_u64() != Some(them) {
            op_with("use", serde_json::json!({"id": player, "person": them}));
        }
        let by = talk["line"]["by"].as_str().unwrap_or_default().to_string();
        let words = talk["line"]["words"].as_str().unwrap_or_default().to_string();
        let left = talk["reading_for"].as_f64().unwrap_or(0.0);
        if !words.is_empty() && by != "You" && left > 0.05 {
            theirs_read += 1;
            let choices = talk["choices"].as_array().map_or(0, |c| c.len());
            assert_eq!(choices, 0, "choices shown while \"{words}\" still had {left:.1} s to be read");
            first_seen.entry(words.clone()).or_insert(left);
        }
        // Answer when it is the player's turn, so the conversation goes on.
        if let Some(n) = talk["choices"].as_array().filter(|c| !c.is_empty()).map(|c| c.len().min(2)) {
            checked += 1;
            op_with("key", serde_json::json!({"key": n.to_string()}));
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    op_with("key", serde_json::json!({"key": "Escape"}));
    eprintln!("their lines seen being read {theirs_read} times; choices answered {checked} times; first reading time left per line: {first_seen:?}");
    assert!(theirs_read > 0, "never saw a line of theirs being read");
    for (words, left) in &first_seen {
        // The `hud` op shows the screen once a game second: up to a second
        // of the reading time has passed by the first look.
        assert!(*left <= reading(words) + 0.05 && *left >= reading(words) - 1.1, "\"{words}\" first seen with {left:.2} s to read, its length gives {:.2}", reading(words));
    }
}
