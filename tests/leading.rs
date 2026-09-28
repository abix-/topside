//! Going with someone (life.md "Going with someone"): the player believes
//! Dell ("Show me."); Dell leads them to the tap and the player follows her
//! with no key pressed (World of Warcraft's auto-follow, `ai::follow`); W
//! ends the following, and Dell, left behind, waits; the player catching
//! up, she goes on (modforge `brain::waits_for`). The choice through the
//! panel with the pointer, the keys through the `key` op, the catching up
//! with the `use` op, screenshots in logs/leading_*.png. Takes seconds.

use std::time::Duration;

use crate::endpoint::{op, op_with};
use crate::talk_ui::shot;

fn reached(pivot: &str) -> bool {
    op("world")["reached"].as_array().is_some_and(|r| r.iter().any(|p| p["pivot"] == pivot))
}

fn npc(id: u64) -> serde_json::Value {
    op_with("npc", serde_json::json!({"id": id}))
}

fn xy(v: &serde_json::Value) -> [f64; 2] {
    [v[0].as_f64().unwrap_or(0.0), v[1].as_f64().unwrap_or(0.0)]
}

fn far(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

#[test]
fn the_player_follows_dell_to_the_tap() {
    crate::endpoint::player_alive();
    let w = op("world");
    let Some(dell) = w["cast"].as_array().and_then(|c| c.iter().find(|p| p["part"] == "Dell")).and_then(|p| p["id"].as_u64()) else {
        eprintln!("the episode is {}, with no Dell; not played this run", w["episode"]);
        return;
    };
    let tap = xy(&w["things"].as_array().and_then(|t| t.iter().find(|t| t["name"] == "tap")).expect("The Tap has its tap")["at"]);
    if reached("does not believe it") || reached("sends Dell") {
        eprintln!("the opening was answered another way this episode; not played this run");
        return;
    }
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    if !reached("believes Dell") {
        op_with("use", serde_json::json!({"id": player, "person": dell}));
        let chosen = crate::the_tap::click_row(&["Show me.", "Let's go."]);
        for _ in 0..16 {
            if reached("believes Dell") {
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        shot("leading_1_show_me");
        assert!(reached("believes Dell"), "{chosen:?} reached nothing");
    } else if op("player")["following"].as_u64() != Some(dell) {
        eprintln!("\"Show me.\" was chosen earlier and the player no longer follows Dell; not played this run");
        return;
    }
    // The player follows Dell with no key pressed: both go toward the tap.
    let start = xy(&op("player")["at"]);
    let mut p = op("player");
    for _ in 0..40 {
        if far(xy(&p["at"]), start) >= 20.0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
        p = op("player");
    }
    let d = npc(dell);
    shot("leading_2_following");
    eprintln!(
        "the player following {} at {}, from {start:?}; Dell at {}, leading {}, doing {}, about {}, way {}, this tick {}; the panel {}",
        p["following"], p["at"], d["at"], d["leading"], d["doing"], d["about"], d["way"], d["this_tick"], op("hud")["talk"]
    );
    let failed: Vec<serde_json::Value> = op("ways").as_array().map_or(Vec::new(), |w| w.iter().filter(|w| w["id"].as_u64() == Some(dell) || w["id"].as_u64() == Some(player)).cloned().collect());
    eprintln!("Dell's and the player's ways not found: {failed:?}");
    if !failed.is_empty() {
        let tiles = d["goal_chunk"]["tiles"].as_array().map_or(String::new(), |r| r.iter().filter_map(|r| r.as_str()).collect::<Vec<_>>().join("\n"));
        eprintln!("the walk grid around Dell's goal {} (G):\n{tiles}", d["way"]["goal"]);
        let at_tap = op_with("walk", serde_json::json!({"at": tap}));
        let tiles = at_tap["tiles"].as_array().map_or(String::new(), |r| r.iter().filter_map(|r| r.as_str()).collect::<Vec<_>>().join("\n"));
        eprintln!("the walk grid at the tap {tap:?} (@), chunk {} tile {}:\n{tiles}\nsolids by the tap: {}", at_tap["chunk"], at_tap["tile"], at_tap["solids"]);
    }
    assert_eq!(p["following"].as_u64(), Some(dell), "the player does not follow Dell");
    assert!(far(xy(&p["at"]), start) >= 20.0, "the player did not walk after Dell");
    assert!(far(xy(&p["at"]), tap) < far(start, tap), "the player did not go toward the tap");
    // Dell may walk faster; she waits once 12 m ahead (brain::LEAD_WAIT),
    // plus what she walks before her brain turns to it.
    assert!(far(xy(&p["at"]), xy(&d["at"])) <= 25.0, "the player fell behind Dell: {}", far(xy(&p["at"]), xy(&d["at"])));
    // W, the player's own move, ends the following; Dell, left behind,
    // waits. Pressed once the conversation has closed (keys move nobody
    // while a panel is open).
    crate::talk_ui::closed_within_8s();
    op_with("key", serde_json::json!({"key": "W"}));
    let mut p = op("player");
    for _ in 0..12 {
        if p["following"].is_null() {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
        p = op("player");
    }
    assert!(p["following"].is_null(), "W did not end the following");
    let mut d = npc(dell);
    for _ in 0..60 {
        if !d["leading"]["waiting_at"].is_null() {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
        d = npc(dell);
    }
    let me = xy(&op("player")["at"]);
    shot("leading_3_waits");
    eprintln!("Dell leading {}, at {}, the player at {me:?}", d["leading"], d["at"]);
    assert!(!d["leading"]["waiting_at"].is_null(), "Dell never waited for the player left behind");
    // She waits once more than 12 m ahead (brain::LEAD_WAIT), plus what she
    // walks before her brain turns to it (about a second).
    assert!(far(xy(&d["at"]), me) <= 25.0, "Dell waits too far off: {:?} from the player", far(xy(&d["at"]), me));
    // The player catches up: Dell goes on toward the tap (and, the player
    // no longer following, waits again further on).
    let waited_at = xy(&d["at"]);
    op_with("use", serde_json::json!({"id": player, "at": [waited_at[0] + 2.0, waited_at[1]]}));
    let mut d = npc(dell);
    for _ in 0..40 {
        if far(xy(&d["at"]), tap) < far(waited_at, tap) - 3.0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
        d = npc(dell);
    }
    shot("leading_4_goes_on");
    eprintln!("after catching up: Dell leading {}, at {}, the tap at {tap:?}", d["leading"], d["at"]);
    assert!(far(xy(&d["at"]), tap) < far(waited_at, tap) - 3.0, "Dell did not go on toward the tap");
}
