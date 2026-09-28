//! Relationships at both levels (life.md "How a person feels about
//! others"), played the way the player plays it: at the tap the player
//! says "Step away from it." to Mara through the talk panel with the
//! pointer; her feeling toward the player, from her memory of it, is
//! hostile, she turns on the player, and her bunker mates, who heard
//! nothing, do not. The meeting is brought on with the `use` op (Mara to
//! the tap, the player by her), nothing waited for. Screenshots in
//! logs/relationship_*.png. Takes seconds.

use std::time::Duration;

use crate::endpoint::{op, op_with};
use crate::talk_ui::{shot, talk_after};

fn reached(pivot: &str) -> bool {
    op("world")["reached"].as_array().is_some_and(|r| r.iter().any(|p| p["pivot"] == pivot))
}

fn npc(id: u64) -> serde_json::Value {
    op_with("npc", serde_json::json!({"id": id}))
}

fn relation_toward(n: &serde_json::Value, id: u64) -> Option<serde_json::Value> {
    n["feelings"].as_array()?.iter().find(|f| f["toward"].as_u64() == Some(id)).map(|f| f["relation"].clone())
}

#[test]
fn a_threat_turns_mara_and_not_her_bunker() {
    let w = op("world");
    let Some(mara) = w["cast"].as_array().and_then(|c| c.iter().find(|p| p["part"] == "Mara")).and_then(|p| p["id"].as_u64()) else {
        eprintln!("the episode is {}, with no Mara; not played this run", w["episode"]);
        return;
    };
    if reached("the meeting") {
        eprintln!("the meeting was already played this episode; not played this run");
        return;
    }
    let tap = w["things"].as_array().and_then(|t| t.iter().find(|t| t["name"] == "tap")).expect("The Tap has its tap")["at"].clone();
    crate::endpoint::player_alive();
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    // Mara at the tap, the player by her: the meeting.
    op_with("use", serde_json::json!({"id": mara, "at": tap}));
    let near_tap = |at: &serde_json::Value| {
        let d = |i: usize| at[i].as_f64().unwrap_or(1e9) - tap[i].as_f64().unwrap_or(0.0);
        (d(0).powi(2) + d(1).powi(2)).sqrt() <= 3.0
    };
    let mut at = npc(mara)["at"].clone();
    for _ in 0..12 {
        if near_tap(&at) {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
        at = npc(mara)["at"].clone();
    }
    assert!(near_tap(&at), "the use op did not bring Mara to the tap at {tap}: she is at {at}");
    op_with("use", serde_json::json!({"id": player, "person": mara}));
    // The meeting's line and choices, at most 8 real seconds (a game just
    // started runs slow).
    let has_threat = |t: &serde_json::Value| t["open"] == true && t["rows"].as_array().is_some_and(|r| r.iter().any(|r| r["words"] == "Step away from it."));
    let mut talk = talk_after(has_threat);
    for _ in 0..20 {
        if has_threat(&talk) {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
        talk = op("hud")["talk"].clone();
    }
    shot("relationship_1_meeting");
    let Some(row) = talk["rows"].as_array().and_then(|r| r.iter().find(|r| r["words"] == "Step away from it.")).cloned() else {
        panic!(
            "the meeting's choices are not in the panel: {talk}; tap at {tap}, Mara at {}, the player at {}; reached {}",
            npc(mara)["at"],
            op("player")["at"],
            op("world")["reached"]
        );
    };
    // The threat, with the mouse.
    op_with("pointer", serde_json::json!({"at": row["at"]}));
    std::thread::sleep(Duration::from_millis(400));
    op_with("pointer", serde_json::json!({"click": true}));
    for _ in 0..12 {
        if reached("threatens Mara") {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    shot("relationship_2_threat");
    assert!(reached("threatens Mara"), "\"Step away from it.\" reached nothing");
    // Her feeling from her memory of it, and she turns on the player.
    let mut m = npc(mara);
    for _ in 0..20 {
        if m["last_threat"]["who"].as_u64() == Some(player) {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
        m = npc(mara);
    }
    shot("relationship_3_mara_turns");
    eprintln!("Mara: feelings {}, last threat {}, doing {}", m["feelings"], m["last_threat"], m["doing"]);
    assert_eq!(relation_toward(&m, player), Some(serde_json::json!("Hostile")), "Mara is not hostile to the player after the threat");
    assert_eq!(m["last_threat"]["who"].as_u64(), Some(player), "Mara never turned on the player");
    // Her bunker mates were not threatened: none of them is hostile, and
    // their bunker's standing, one grudge among them, is not hostile.
    let faction = m["faction"].clone();
    let standing = op("world")["factions"]
        .as_array()
        .and_then(|f| f.iter().find(|f| f["faction"] == faction && f["toward"] == "player").cloned());
    eprintln!("Mara's bunker ({faction}) toward the player's: {standing:?}");
    let standing = standing.expect("the world op reports Mara's bunker's standing toward the player's");
    assert_ne!(standing["relation"], "Hostile", "one threat to Mara turned her whole bunker");
    for mate in crate::endpoint::people().iter().filter(|p| p["faction"] == faction && p["id"].as_u64() != Some(mara)) {
        let id = mate["id"].as_u64().unwrap_or(0);
        assert_ne!(relation_toward(&npc(id), player), Some(serde_json::json!("Hostile")), "{id}, Mara's bunker mate, is hostile without being threatened");
    }
}

/// Word spreads: one of Mara's bunker mates, brought to her with the `use`
/// op, is told by her what the player said to her, and feels it too, at
/// half (modforge `talk::share`).
#[test]
fn word_of_the_threat_spreads_to_a_mate() {
    let w = op("world");
    let Some(mara) = w["cast"].as_array().and_then(|c| c.iter().find(|p| p["part"] == "Mara")).and_then(|p| p["id"].as_u64()) else {
        eprintln!("the episode is {}, with no Mara; not played this run", w["episode"]);
        return;
    };
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    let m = npc(mara);
    if relation_toward(&m, player).is_none() {
        eprintln!("Mara feels nothing toward the player this episode; not played this run");
        return;
    }
    let faction = m["faction"].clone();
    let mates: Vec<u64> = crate::endpoint::people()
        .iter()
        .filter(|p| p["faction"] == faction && p["dead"] != true && p["id"].as_u64() != Some(mara))
        .filter_map(|p| p["id"].as_u64())
        .collect();
    let Some(mate) = mates.iter().copied().find(|id| relation_toward(&npc(*id), player).is_none()) else {
        eprintln!("every mate of Mara's already feels something toward the player; not played this run");
        return;
    };
    op_with("use", serde_json::json!({"id": mate, "person": mara}));
    let mut n = npc(mate);
    for _ in 0..16 {
        if relation_toward(&n, player).is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
        n = npc(mate);
    }
    let told: Vec<String> = n["did"]
        .as_array()
        .map_or(Vec::new(), |d| d.iter().filter_map(|d| d["did"].as_str()).filter(|d| d.starts_with("was told")).map(str::to_string).collect());
    eprintln!("mate {mate}: feelings {}; told {told:?}", n["feelings"]);
    assert!(told.iter().any(|t| t.starts_with(&format!("was told by {mara}:"))), "Mara told her mate {mate} nothing");
    let felt = n["feelings"].as_array().and_then(|f| f.iter().find(|f| f["toward"].as_u64() == Some(player))).and_then(|f| f["feeling"].as_f64());
    let hers = m["feelings"].as_array().and_then(|f| f.iter().find(|f| f["toward"].as_u64() == Some(player))).and_then(|f| f["feeling"].as_f64());
    assert!(felt.is_some_and(|f| f < 0.0), "mate {mate} feels nothing bad toward the player after being told: {felt:?}");
    assert!(felt.zip(hers).is_some_and(|(f, h)| f > h), "being told is felt less than being threatened: {felt:?} against Mara's {hers:?}");
    // Mara told of the places she knows too (one seen already is kept as
    // seen).
    let places: Vec<&str> = n["knows"].as_array().map_or(Vec::new(), |k| k.iter().filter(|k| k["told_by"].as_u64() == Some(mara)).filter_map(|k| k["kind"].as_str()).collect());
    eprintln!("mate {mate}, places told of by Mara: {places:?}");
    assert!(!places.is_empty(), "Mara told her mate {mate} of no place");
}

/// Word spreads, but not what is kept quiet: Dell wants the tap kept
/// secret (docs/episodes/the-tap.md), so a bunker mate brought to her with
/// the `use` op learns what else she knows and not the tap.
#[test]
fn dell_keeps_the_tap_quiet() {
    let w = op("world");
    let cast = |part: &str| w["cast"].as_array().and_then(|c| c.iter().find(|p| p["part"] == part)).and_then(|p| p["id"].as_u64());
    let (Some(dell), Some(tap)) = (cast("Dell"), w["things"].as_array().and_then(|t| t.iter().find(|t| t["name"] == "tap")).and_then(|t| t["key"].as_u64())) else {
        eprintln!("the episode is {}, with no Dell and tap; not played this run", w["episode"]);
        return;
    };
    let d = npc(dell);
    assert!(d["quiet"].as_array().is_some_and(|q| q.iter().any(|k| k.as_u64() == Some(tap))), "Dell does not keep the tap quiet: {}", d["quiet"]);
    let faction = d["faction"].clone();
    let Some(mate) = crate::endpoint::people()
        .iter()
        .filter(|p| p["faction"] == faction && p["dead"] != true && p["id"].as_u64() != Some(dell))
        .find_map(|p| p["id"].as_u64())
    else {
        eprintln!("Dell has no living bunker mate; not played this run");
        return;
    };
    let knows = |n: &serde_json::Value, key: u64| n["knows"].as_array().is_some_and(|k| k.iter().any(|k| k["key"].as_u64() == Some(key)));
    let dells: Vec<u64> = d["knows"].as_array().map_or(Vec::new(), |k| k.iter().filter_map(|k| k["key"].as_u64()).collect());
    op_with("use", serde_json::json!({"id": mate, "person": dell}));
    let mut n = npc(mate);
    for _ in 0..16 {
        if dells.iter().any(|k| *k != tap && knows(&n, *k)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
        n = npc(mate);
    }
    let told: Vec<&serde_json::Value> = n["knows"].as_array().map_or(Vec::new(), |k| k.iter().filter(|k| k["told_by"].as_u64() == Some(dell)).collect());
    eprintln!("mate {mate}, told by Dell: {told:?}");
    assert!(!told.is_empty(), "Dell told her mate {mate} nothing");
    assert!(!knows(&n, tap), "Dell's mate {mate} knows the tap");
}
