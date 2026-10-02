//! The Tap's first day played the way the player plays it
//! (docs/episodes/the-tap.md): "Show me." sends Dell to the tap; Mara
//! waits there; the player at the tap meets Mara, who says her line, and
//! chooses. The mouse through the `pointer` op, a screenshot at each step
//! in logs/tap_*.png to be looked at. Dell and Mara walk: at most 90 real
//! seconds, most of the test.

use std::time::Duration;

use crate::endpoint::{op, op_with};
use crate::talk_ui::{shot, talk_after};

fn at_of(id: u64) -> Option<[f64; 2]> {
    let at = op_with("npc", serde_json::json!({"id": id}))["at"].clone();
    Some([at.get(0)?.as_f64()?, at.get(1)?.as_f64()?])
}

fn far(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// Say the choice whose words are one of `words` in the open talk box by
/// its number key (episodes.md "How a conversation looks"); the words
/// said.
pub fn click_row(words: &[&str]) -> Option<String> {
    // The choice, at most 8 real seconds (a game just started runs slow).
    let row_of = |t: &serde_json::Value| {
        t["choices"].as_array().and_then(|c| c.iter().position(|c| words.iter().any(|w| c == *w)).map(|i| (i, c[i].clone())))
    };
    let mut talk = talk_after(|t| row_of(t).is_some());
    for _ in 0..20 {
        if row_of(&talk).is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
        talk = op("hud")["talk"].clone();
    }
    let (i, said) = row_of(&talk)?;
    op_with("key", serde_json::json!({"key": (i + 1).to_string()}));
    said.as_str().map(str::to_string)
}

fn reached(pivot: &str) -> bool {
    op("world")["reached"].as_array().is_some_and(|r| r.iter().any(|p| p["pivot"] == pivot))
}

#[test]
fn the_taps_first_day() {
    crate::endpoint::player_alive();
    let w = op("world");
    let cast = |part: &str| w["cast"].as_array().and_then(|c| c.iter().find(|p| p["part"] == part)).and_then(|p| p["id"].as_u64());
    let (Some(dell), Some(mara)) = (cast("Dell"), cast("Mara")) else {
        eprintln!("the episode is {}, with no Dell and Mara; not played this run", w["episode"]);
        return;
    };
    let tap = w["things"].as_array().and_then(|t| t.iter().find(|t| t["name"] == "tap")).expect("The Tap has its tap");
    let tap = [tap["at"][0].as_f64().unwrap_or(0.0), tap["at"][1].as_f64().unwrap_or(0.0)];
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    // "Show me.": the player by Dell, the panel, the choice.
    if !reached("believes Dell") {
        op_with("use", serde_json::json!({"id": player, "person": dell}));
        let chosen = click_row(&["Show me.", "Let's go."]);
        eprintln!("chose {chosen:?}");
        for _ in 0..12 {
            if reached("believes Dell") {
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        shot("tap_1_show_me");
        assert!(reached("believes Dell"), "\"Show me.\" reached nothing");
        op_with("key", serde_json::json!({"key": "Escape"}));
    }
    // Dell walks to the tap; Mara is there, or on her way.
    let mut dell_at = at_of(dell);
    let mut mara_at = at_of(mara);
    for _ in 0..90 {
        if dell_at.zip(mara_at).is_some_and(|(d, m)| far(d, tap) <= 5.0 && far(m, tap) <= 6.0) {
            break;
        }
        // The player follows Dell there, no key pressed (ai::follow).
        std::thread::sleep(Duration::from_secs(1));
        dell_at = at_of(dell);
        mara_at = at_of(mara);
    }
    eprintln!("tap at {tap:?}; Dell at {dell_at:?}, Mara at {mara_at:?}");
    for (part, id) in [("Dell", dell), ("Mara", mara)] {
        let n = op_with("npc", serde_json::json!({"id": id}));
        eprintln!("{part}: doing {}, about {}, last threat {}, feelings {}, way {}", n["doing"], n["about"], n["last_threat"], n["feelings"], n["way"]);
    }
    assert!(dell_at.is_some_and(|d| far(d, tap) <= 5.0), "Dell did not go to the tap");
    assert!(mara_at.is_some_and(|m| far(m, tap) <= 6.0), "Mara is not at the tap");
    // The player at the tap, by Mara: her line, then the panel on her.
    op_with("use", serde_json::json!({"id": player, "person": mara}));
    let heard_mara = |p: &serde_json::Value| p["heard"].as_array().and_then(|h| h.iter().rev().find(|h| h["by"].as_u64() == Some(mara)).cloned());
    let mut p = op("player");
    for _ in 0..12 {
        if heard_mara(&p).is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
        p = op("player");
    }
    shot("tap_2_meeting");
    eprintln!("Mara said {:?}", heard_mara(&p));
    assert!(heard_mara(&p).is_some(), "the player at the tap heard nothing from Mara");
    let talk = talk_after(|t| t["open"] == true && t["to"].as_u64() == Some(mara));
    if talk["to"].as_u64() != Some(mara) {
        op_with("use", serde_json::json!({"id": player, "person": mara}));
    }
    let chosen = click_row(&["There's enough for both of us.", "Step away from it.", "Keep it."]);
    eprintln!("chose {chosen:?}");
    let pivots = ["talks with Mara", "threatens Mara", "leaves Mara the tap"];
    for _ in 0..12 {
        if pivots.iter().any(|p| reached(p)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    shot("tap_3_chose");
    eprintln!("reached: {}", op("world")["reached"]);
    assert!(pivots.iter().any(|p| reached(p)), "the meeting's choice reached nothing");
    // Mara's reply, then it ends by itself.
    let closed = crate::talk_ui::closed_within_8s();
    shot("tap_4_ended");
    eprintln!("Mara said {:?}; then {closed}", heard_mara(&op("player")));
    assert_eq!(closed["open"], false, "the meeting never ended");
}
