//! The talk panel played the way the player plays it (operator,
//! 2026-09-27: see what the player sees, drive every step): keys through
//! the `key` op, the mouse through the `pointer` op, a screenshot at each
//! step in logs/talk_*.png to be looked at. Takes seconds.

use std::time::Duration;

use crate::endpoint::{op, op_with};

fn shot(name: &str) {
    op_with("screenshot", serde_json::json!({"name": name}));
    std::thread::sleep(Duration::from_millis(400));
}

/// The `hud` op's talk panel once it has published again (once a game
/// second), at most 3 real seconds.
fn talk_after(done: impl Fn(&serde_json::Value) -> bool) -> serde_json::Value {
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

#[test]
fn the_player_talks_to_dell_through_the_panel() {
    let w = op("world");
    let Some(dell) = w["cast"].as_array().and_then(|c| c.iter().find(|p| p["part"] == "Dell")).and_then(|p| p["id"].as_u64()) else {
        eprintln!("the episode is {}, with no Dell; not played this run", w["episode"]);
        return;
    };
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    // Stand by Dell facing her (the `use` op aims there), then close
    // whatever that opened with Escape, as a player would.
    let at = op_with("npc", serde_json::json!({"id": dell}))["at"].clone();
    op_with("use", serde_json::json!({"id": player, "at": at}));
    std::thread::sleep(Duration::from_millis(500));
    op_with("key", serde_json::json!({"key": "Escape"}));
    let closed = talk_after(|t| t["open"] == false);
    shot("talk_0_by_dell");
    eprintln!("by Dell, after Escape: {closed}");
    // E: the talk panel opens.
    op_with("key", serde_json::json!({"key": "E"}));
    let open = talk_after(|t| t["open"] == true);
    shot("talk_1_open");
    eprintln!("after E: {open}");
    assert_eq!(open["open"], true, "E on Dell opened no talk panel");
    let row = |t: &serde_json::Value| t["rows"].as_array().and_then(|r| r.iter().find(|r| r["words"] == "Tell them of the tap").cloned());
    let target = row(&open).expect("a row to tell of the tap");
    // The mouse over that row.
    op_with("pointer", serde_json::json!({"at": target["at"]}));
    std::thread::sleep(Duration::from_millis(600));
    let hovered = op("hud")["talk"].clone();
    shot("talk_2_hover");
    eprintln!("rows before the hover: {}", open["rows"]);
    eprintln!("rows after the hover:  {}", hovered["rows"]);
    // A click on it.
    let said = |p: &serde_json::Value| p["said"].as_array().map_or(0, |s| s.iter().filter(|s| s["to"].as_u64() == Some(dell)).count());
    let before = said(&op("player"));
    op_with("pointer", serde_json::json!({"click": true}));
    let mut after = op("player");
    for _ in 0..12 {
        if said(&after) > before {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
        after = op("player");
    }
    shot("talk_3_clicked");
    eprintln!("the player said to Dell {before} times before the click, {} after: {}", said(&after), after["said"]);
    let shown = talk_after(|t| t["lines"].as_str().is_some_and(|l| l.contains("You: told of the tap")));
    eprintln!("the panel's lines after the click: {:?}", shown["lines"]);
    assert_eq!(hovered["rows"], open["rows"], "the rows moved when the mouse went over them");
    assert!(said(&after) > before, "a click on \"Tell them of the tap\" said nothing");
    assert!(
        shown["lines"].as_str().is_some_and(|l| l.contains("You: told of the tap")),
        "the panel does not show what the player said"
    );
}
