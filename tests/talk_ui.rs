//! The talk panel played the way the player plays it (operator,
//! 2026-09-27: see what the player sees, drive every step): keys through
//! the `key` op, the mouse through the `pointer` op, a screenshot at each
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
    let people = crate::endpoint::people();
    // Someone not out to kill the player: a stranger of no faction.
    let Some(stranger) = people.iter().find(|p| !crate::endpoint::is_bunker_person(p) && p["dead"] != true && p["faction"].is_null()) else {
        eprintln!("no living stranger this run");
        return;
    };
    let (id, name) = (stranger["id"].as_u64().unwrap_or(0), stranger["name"].as_str().unwrap_or_default().to_string());
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    let before = heard_from(id).len();
    // Stand by them, looking at them: the use op also presses E.
    op_with("use", serde_json::json!({"id": player, "person": id}));
    let open = talk_after(|t| t["open"] == true);
    let prompt = op("hud")["prompt"].clone();
    shot("stranger_1_talking");
    let mut heard = heard_from(id);
    for _ in 0..12 {
        if heard.len() > before {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
        heard = heard_from(id);
    }
    eprintln!("{name} ({id}): panel {open}; prompt {prompt}; heard {heard:?}");
    assert!(heard.len() > before, "{name} said nothing");
    let closed = closed_within_8s();
    shot("stranger_2_ended");
    eprintln!("after their line: {closed}");
    assert_eq!(closed["open"], false, "the conversation with {name} never ended");
}

#[test]
fn the_player_talks_to_dell_through_the_panel() {
    let w = op("world");
    let Some(dell) = w["cast"].as_array().and_then(|c| c.iter().find(|p| p["part"] == "Dell")).and_then(|p| p["id"].as_u64()) else {
        eprintln!("the episode is {}, with no Dell; not played this run", w["episode"]);
        return;
    };
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    // Stand by Dell looking at her and use (the `use` op: E, as the
    // player's hands press it): she stops to talk.
    op_with("use", serde_json::json!({"id": player, "person": dell}));
    let dell_name = crate::endpoint::people().into_iter().find(|p| p["id"].as_u64() == Some(dell)).and_then(|p| p["name"].as_str().map(str::to_string));
    let want = dell_name.as_deref().map(|n| format!("[E] Talk to {n}"));
    let mut prompt = op("hud")["prompt"].clone();
    for _ in 0..12 {
        if prompt.as_str() == want.as_deref() {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
        prompt = op("hud")["prompt"].clone();
    }
    let open = talk_after(|t| t["open"] == true);
    shot("talk_1_open");
    eprintln!("by Dell ({dell_name:?}): prompt {prompt}; {open}");
    assert_eq!(prompt.as_str(), want.as_deref(), "the prompt names Dell's person");
    assert_eq!(open["open"], true, "talking to Dell opened no talk panel");
    // The choice to believe Dell, however it is said this time; the
    // opening already chosen this episode (another test played it), there
    // is nothing more of it to read.
    let believe = ["Show me.", "Let's go."];
    let row = |t: &serde_json::Value| t["rows"].as_array().and_then(|r| r.iter().find(|r| believe.iter().any(|b| r["words"] == *b)).cloned());
    let Some(target) = row(&open) else {
        eprintln!("the opening's choices are closed (already chosen this episode); the rows: {}", open["rows"]);
        return;
    };
    // Dell's opening, as the panel shows it: no slot left unfilled.
    let lines = open["lines"].as_str().unwrap_or_default().to_string();
    eprintln!("Dell's lines: {lines:?}");
    assert!(lines.contains("tap"), "the panel shows no line of Dell's about the tap: {lines:?}");
    assert!(!lines.contains('{'), "a slot left unfilled: {lines:?}");
    // The mouse over that row.
    op_with("pointer", serde_json::json!({"at": target["at"]}));
    std::thread::sleep(Duration::from_millis(600));
    let hud = op("hud");
    let hovered = hud["talk"].clone();
    eprintln!("the pointer op put the mouse at {}; picking has it at {}", target["at"], hud["pointer"]);
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
    let words = target["words"].as_str().unwrap_or_default().to_string();
    let shown = talk_after(|t| t["lines"].as_str().is_some_and(|l| l.contains(&format!("You: {words}"))));
    eprintln!("the panel's lines after the click: {:?}", shown["lines"]);
    let reached = op("world")["reached"].clone();
    eprintln!("reached: {reached}");
    assert_eq!(hovered["rows"], open["rows"], "the rows moved when the mouse went over them");
    assert!(said(&after) > before, "a click on {words:?} said nothing");
    assert!(
        shown["lines"].as_str().is_some_and(|l| l.contains(&format!("You: {words}"))),
        "the panel does not show what the player said"
    );
    assert!(
        reached.as_array().is_some_and(|r| r.iter().any(|p| p["pivot"] == "believes Dell")),
        "choosing {words:?} reached no pivot point believing Dell"
    );
    // Dell's reply, then the conversation ends by itself.
    let replies = ["Follow me. Keep up.", "This way. Stay close."];
    let mut heard = heard_from(dell);
    for _ in 0..12 {
        if heard.iter().any(|h| replies.contains(&h.as_str())) {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
        heard = heard_from(dell);
    }
    shot("talk_4_reply");
    let closed = closed_within_8s();
    shot("talk_5_ended");
    eprintln!("Dell said {heard:?}; then {closed}");
    assert!(heard.iter().any(|h| replies.contains(&h.as_str())), "Dell never replied to {words:?}");
    assert_eq!(closed["open"], false, "the conversation never ended");
}
