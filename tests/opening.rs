//! The Tap's opening (docs/episodes/the-tap.md "How it opens" and "The
//! lines", todo 11aj), one behaviour a test, for the operator's /mygoal
//! "The Tap's opening plays". The path tested is the player believing
//! Roxanne ("Show me."); the other answers are later goals.
//!
//! Every test starts from one of the opening's saves (authority.md
//! "Running tests"), keeps the game paused, and moves it only with `step`,
//! so the same test plays the same every time. The first save,
//! `opening_woke`, can only be made on a game just launched
//! (scripts/build.ps1 run); every later save is reached from the one
//! before it by playing, and made once.
//!
//! Roxanne plays the part the-tap.md calls Dell: the game's world op names
//! that part "Dell".

use serde_json::{Value as Json, json};

use crate::endpoint::{op, op_with, pause, start_from, step};

/// One game second, in ticks.
const SECOND: u64 = 60;

/// Play a game second at a time, at most `secs` game seconds, until `done`
/// holds; whether it did.
fn within(secs: u64, done: impl Fn() -> bool) -> bool {
    for _ in 0..secs {
        if done() {
            return true;
        }
        step(SECOND);
    }
    done()
}

/// Roxanne's ActorId: the person playing The Tap's "Dell" part.
fn roxanne() -> u64 {
    let w = op("world");
    w["cast"].as_array().and_then(|c| c.iter().find(|p| p["part"] == "Dell")).and_then(|p| p["id"].as_u64()).unwrap_or_else(|| panic!("The Tap has no one in Roxanne's part: episode {}", w["episode"]))
}

fn player() -> u64 {
    op("player")["id"].as_u64().expect("the player op reports the player's id")
}

/// Where a person stands: ground x, ground z, and their feet's height.
fn spot_of(id: u64) -> (f64, f64, f64) {
    let p = op_with("npc", json!({"id": id}));
    (p["at"][0].as_f64().unwrap_or(f64::NAN), p["at"][1].as_f64().unwrap_or(f64::NAN), p["feet"].as_f64().unwrap_or(f64::NAN))
}

fn player_spot() -> (f64, f64, f64) {
    let s = op("status");
    (s["player"][0].as_f64().unwrap_or(f64::NAN), s["player"][1].as_f64().unwrap_or(f64::NAN), s["player_feet"].as_f64().unwrap_or(f64::NAN))
}

/// Ground distance between two spots, and whether they are on one level.
fn apart(a: (f64, f64, f64), b: (f64, f64, f64)) -> (f64, bool) {
    (((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt(), (a.2 - b.2).abs() < 1.0)
}

/// The tap: where it stands (ground x, ground z, height 0).
fn the_tap() -> (f64, f64, f64) {
    let w = op("world");
    let tap = w["things"].as_array().and_then(|t| t.iter().find(|t| t["name"] == "tap")).cloned().expect("The Tap has its tap in the world");
    (tap["at"][0].as_f64().unwrap_or(f64::NAN), tap["at"][1].as_f64().unwrap_or(f64::NAN), 0.0)
}

/// The door of the player's room: the bunker door on the player's level
/// nearest them.
fn the_players_door() -> Json {
    let me = player_spot();
    op("world")["bunker_doors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|d| d["floor"].as_f64().is_some_and(|f| (f - me.2).abs() < 0.5))
        .min_by(|a, b| {
            let far = |d: &Json| apart((d["at"][0].as_f64().unwrap_or(0.0), d["at"][1].as_f64().unwrap_or(0.0), me.2), me).0;
            far(a).total_cmp(&far(b))
        })
        .cloned()
        .expect("a door on the player's level")
}

/// Every line `by` said to the player, in order: (tick, words).
fn heard_from(by: u64) -> Vec<(u64, String)> {
    op("player")["heard"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|h| h["by"].as_u64() == Some(by))
        .map(|h| (h["tick"].as_u64().unwrap_or(0), h["said"].as_str().unwrap_or_default().to_string()))
        .collect()
}

fn reached(pivot: &str) -> bool {
    op("world")["reached"].as_array().is_some_and(|r| r.iter().any(|p| p["pivot"] == pivot))
}

fn talk() -> Json {
    op("hud")["talk"].clone()
}

fn choices() -> Vec<String> {
    talk()["choices"].as_array().into_iter().flatten().filter_map(|c| c.as_str().map(str::to_string)).collect()
}

/// Press the number key of the first choice shown that is one of `words`;
/// the words chosen.
fn choose(words: &[&str]) -> String {
    let shown = choices();
    let i = shown.iter().position(|c| words.contains(&c.as_str())).unwrap_or_else(|| panic!("none of {words:?} among the choices {shown:?}"));
    op_with("key", json!({"key": (i + 1).to_string()}));
    step(SECOND);
    shown[i].clone()
}

/// The player walks to the note in their room and reads it (E), as
/// bunker.rs does: the note lies 1.4 m west and 3.6 m south of where the
/// player wakes.
fn read_the_note() {
    let (x, z, feet) = player_spot();
    let note = (x - 1.4, z + 3.6);
    op_with("command", json!({"do": "move_to", "x": note.0, "y": feet, "z": note.1}));
    // A walk is done anywhere within the brain's REACH (1.5 m, modforge
    // brain.rs), which is also Use's reach (interact.rs REACH): wait for the
    // command to end there.
    let there = within(10, || op("player")["commands"].is_null() && apart(player_spot(), (note.0, note.1, feet)).0 <= 1.5);
    if !there {
        crate::endpoint::why_stopped((note.0, feet, note.1));
    }
    assert!(there, "the player never walked to the note at ({:.1}, {:.1}), feet {feet:.2}: they stand at {:?}", note.0, note.1, player_spot());
    op_with("key", json!({"key": "E"}));
    let read = within(3, || op("hud")["reading"] == true);
    assert!(read, "E by the note did not open it");
    op_with("key", json!({"key": "Escape"}));
    step(SECOND);
}

// The opening's saves, each made once by playing from the one before.

/// The player just woke in their room on the deepest level, nothing of
/// the opening done. Made only on a game just launched. Every save below
/// is loaded into a paused game: the test pauses first.
fn woke() {
    start_from("opening_woke", || {
        // A game just launched and paused at once has not yet published
        // its world: one game second first.
        step(SECOND);
        let w = op("world");
        let feet = player_spot().2;
        let fresh = w["episode"] == "The Tap" && w["reached"].as_array().is_some_and(|r| r.is_empty()) && (feet + 6.0).abs() < 0.3 && talk()["open"] != true;
        assert!(
            fresh,
            "the save opening_woke is made on a game just launched, before anyone plays: run scripts/build.ps1 run, then the opening tests (now: episode {}, reached {}, the player's feet at {feet:.2})",
            w["episode"], w["reached"]
        );
    });
}

/// The talk box open with Roxanne, at her knock on the player's shut door,
/// her opening not yet said. Reached by commanding her knock (the knock
/// itself, todo 15b), not by the note, so it does not wait on tests 3-6.
fn talking() {
    start_from("opening_talking", || {
        woke();
        let her = roxanne();
        op_with("command", json!({"id": her, "do": "knock", "who": player()}));
        let open = within(90, || talk()["to"].as_u64() == Some(her));
        assert!(open, "reaching opening_talking: Roxanne's knock never opened the talk box");
    });
}

/// The opening said and the player's answers shown.
fn choosing() {
    start_from("opening_choosing", || {
        talking();
        let shown = within(60, || !choices().is_empty());
        assert!(shown, "reaching opening_choosing: no answers shown within a game minute");
    });
}

/// "Show me." chosen; the player following Roxanne, out of the bunker on
/// the ground.
fn out_the_door() {
    start_from("opening_out_the_door", || {
        choosing();
        choose(&["Show me.", "Let's go."]);
        let out = within(120, || player_spot().2 > -0.5 && op("player")["following"].as_u64() == Some(roxanne()));
        assert!(out, "reaching opening_out_the_door: the player was not following Roxanne on the ground within two game minutes");
    });
}

/// The player and Roxanne at the tap.
fn at_the_tap() {
    start_from("opening_at_the_tap", || {
        out_the_door();
        let tap = the_tap();
        let there = within(600, || apart(player_spot(), tap).0 < 4.0 && apart(spot_of(roxanne()), tap).0 < 4.0);
        assert!(there, "reaching opening_at_the_tap: the player and Roxanne were not both at the tap within ten game minutes");
    });
}

// The tests, in the order the opening plays.

/// 1. The Tap's people already exist: Roxanne in the player's bunker, and
/// Mara's part a person of another bunker.
#[test]
fn the_taps_people_are_roxanne_of_the_players_bunker_and_mara_of_another() {
    let _paused = pause();
    woke();
    let w = op("world");
    let part = |name: &str| w["cast"].as_array().and_then(|c| c.iter().find(|p| p["part"] == name)).and_then(|p| p["id"].as_u64());
    let faction = |id: u64| crate::endpoint::people().into_iter().find(|p| p["id"].as_u64() == Some(id)).map(|p| p["faction"].clone()).unwrap_or(Json::Null);
    let (her, mara) = (part("Dell").expect("someone plays Roxanne's part"), part("Mara").expect("someone plays Mara's part"));
    let name = crate::endpoint::people().into_iter().find(|p| p["id"].as_u64() == Some(her)).and_then(|p| p["name"].as_str().map(str::to_string));
    assert_eq!(name.as_deref(), Some("Roxanne"), "Roxanne's part is played by {name:?}");
    assert_eq!(faction(her), "player", "Roxanne is of the player's bunker");
    assert!(faction(mara) != "player" && !faction(mara).is_null(), "Mara is of another bunker, not {}", faction(mara));
}

/// 2. The player wakes in their room on the deepest level and reads the
/// note there.
#[test]
fn the_player_wakes_in_their_room_and_reads_the_note() {
    let _paused = pause();
    woke();
    let feet = player_spot().2;
    assert!((feet + 6.0).abs() < 0.3, "the player woke with their feet at {feet:.2}, not 6 m down");
    read_the_note();
}

/// 3. Reading the note sends Roxanne down to the player at once: within
/// ten game seconds she is below the ground.
#[test]
fn reading_the_note_sends_roxanne_down_at_once() {
    let _paused = pause();
    woke();
    let her = roxanne();
    read_the_note();
    let going = within(10, || spot_of(her).2 < -0.5);
    assert!(going, "Roxanne did not start down after the note was read: she stands at {:?}", spot_of(her));
}

/// 4. Roxanne comes to the player's shut door on her own, within a game
/// minute of the note.
#[test]
fn roxanne_comes_to_the_players_shut_door() {
    let _paused = pause();
    woke();
    let her = roxanne();
    let door = the_players_door();
    let at = (door["at"][0].as_f64().unwrap_or(0.0), door["at"][1].as_f64().unwrap_or(0.0), door["floor"].as_f64().unwrap_or(0.0));
    read_the_note();
    let came = within(60, || {
        let (far, level) = apart(spot_of(her), at);
        far < 2.0 && level
    });
    assert!(came, "Roxanne never came to the player's door at {at:?}: she stands at {:?}", spot_of(her));
}

/// 5. Roxanne knocks on her own, the player hears it, and the talk box
/// opens with her.
#[test]
fn roxanne_knocks_and_the_talk_box_opens_with_her() {
    let _paused = pause();
    woke();
    let her = roxanne();
    read_the_note();
    let heard = within(90, || op("player")["heard_knocking"].as_array().is_some_and(|h| h.iter().any(|k| k["by"].as_u64() == Some(her))));
    assert!(heard, "the player never heard Roxanne knock after reading the note");
    let open = within(5, || talk()["to"].as_u64() == Some(her));
    assert!(open, "the talk box never opened with Roxanne after her knock: {}", talk());
}

/// 6. Roxanne's first words come through the shut door: "You awake in
/// there?" (todo 11aj), before the opening.
#[test]
fn roxanne_says_you_awake_in_there_through_the_door_first() {
    let _paused = pause();
    woke();
    let her = roxanne();
    read_the_note();
    let said = within(90, || !heard_from(her).is_empty());
    assert!(said, "Roxanne said nothing to the player after reading the note");
    let first = heard_from(her)[0].1.clone();
    assert_eq!(first, "You awake in there?", "Roxanne's first words were not through the door");
}

/// 7. Roxanne says the water is nearly gone and gives the way to the tap
/// from the bunker door; the player then knows the tap, told by her.
#[test]
fn roxanne_says_the_water_is_low_and_tells_the_way_to_the_tap() {
    let _paused = pause();
    talking();
    let her = roxanne();
    let shown = within(60, || !choices().is_empty());
    let lines: Vec<String> = heard_from(her).into_iter().map(|(_, w)| w).collect();
    eprintln!("Roxanne said: {lines:?}");
    assert!(shown, "no answers shown within a game minute");
    assert!(lines.iter().any(|l| l.contains("water")), "Roxanne never said the water is low");
    let way = lines.iter().find(|l| l.contains("tap")).cloned().unwrap_or_else(|| panic!("Roxanne never gave the way to the tap"));
    assert!(["left", "right", "straight"].iter().any(|w| way.contains(w)), "the way to the tap names no way out of the door: {way:?}");
    assert!(!way.contains('{'), "a part of the way left unfilled: {way:?}");
    let tap_key = op("world")["things"].as_array().and_then(|t| t.iter().find(|t| t["name"] == "tap")).and_then(|t| t["key"].as_u64()).expect("the tap's key");
    let told = op("player")["knows"].as_array().and_then(|k| k.iter().find(|k| k["key"].as_u64() == Some(tap_key))).and_then(|k| k["told_by"].as_u64());
    assert_eq!(told, Some(her), "the player does not know the tap, told by Roxanne");
}

/// 8. Each of Roxanne's lines stays up long enough to read (modforge
/// `talk::reading_secs`: about 15 characters a second, never under 1.5 s)
/// before her next line or the answers.
#[test]
fn each_line_stays_long_enough_to_read() {
    let _paused = pause();
    talking();
    let her = roxanne();
    let reading = |words: &str| ((words.chars().count() as f64 / 15.0).max(1.5) * SECOND as f64) as u64;
    let mut shown_at = None;
    for _ in 0..60 * 4 {
        if !choices().is_empty() {
            shown_at = Some(op("status")["tick"].as_u64().unwrap_or(0));
            break;
        }
        step(SECOND / 4);
    }
    let shown_at = shown_at.expect("no answers shown within a game minute");
    let lines = heard_from(her);
    eprintln!("Roxanne's lines (tick, words): {lines:?}; answers shown at tick {shown_at}");
    assert!(!lines.is_empty(), "Roxanne said nothing");
    for pair in lines.windows(2) {
        let ((t, words), (next, _)) = (&pair[0], &pair[1]);
        assert!(next - t >= reading(words), "{words:?} was up {} ticks, {} needed to read it", next - t, reading(words));
    }
    let (last, words) = lines.last().cloned().unwrap_or_default();
    // The answers are seen a quarter game second at a time.
    let up = (shown_at + SECOND / 4).saturating_sub(last);
    assert!(up >= reading(&words), "the answers came {up} ticks after {words:?}, {} needed to read it", reading(&words));
}

/// 9. The four answers show, each said one of its ways, and a number key
/// picks one ("Where exactly?" here, which leaves the opening open).
#[test]
fn the_four_answers_show_and_a_number_key_picks_one() {
    let _paused = pause();
    choosing();
    let shown = choices();
    eprintln!("the answers: {shown:?}");
    let ways: [&[&str]; 4] = [
        &["Show me.", "Let's go."],
        &["Water doesn't just run out there. You're wrong.", "I don't believe it."],
        &["Go and look again. I'll wait.", "Check it again first."],
        &["Where exactly?", "Tell me the way again."],
    ];
    for answer in ways {
        assert!(shown.iter().any(|c| answer.contains(&c.as_str())), "none of {answer:?} among the answers {shown:?}");
    }
    let said = |p: &Json| p["said"].as_array().map_or(0, |s| s.iter().filter(|s| s["to"].as_u64() == Some(roxanne())).count());
    let before = said(&op("player"));
    let words = choose(&["Where exactly?", "Tell me the way again."]);
    let after = within(3, || said(&op("player")) > before);
    assert!(after, "the number key for {words:?} said nothing");
}

/// 10. "Show me." is believing Roxanne, and she answers "Follow me. Keep
/// up." or "This way. Stay close."
#[test]
fn show_me_believes_roxanne_and_she_says_follow_me() {
    let _paused = pause();
    choosing();
    let her = roxanne();
    let words = choose(&["Show me.", "Let's go."]);
    let believed = within(5, || reached("believes Dell"));
    assert!(believed, "{words:?} reached nothing: reached {}", op("world")["reached"]);
    let replies = ["Follow me. Keep up.", "This way. Stay close."];
    let replied = within(5, || heard_from(her).iter().any(|(_, w)| replies.contains(&w.as_str())));
    assert!(replied, "Roxanne never answered {words:?}: she said {:?}", heard_from(her));
}

/// 11. A way runs on the navigation mesh from the bunker door to the tap.
#[test]
fn a_way_runs_from_the_bunker_door_to_the_tap() {
    let _paused = pause();
    out_the_door();
    let (x, z, feet) = player_spot();
    let tap = the_tap();
    op_with("nav_path", json!({"from": [x, feet, z], "to": [tap.0, tap.2, tap.1]}));
    // The mesh answers on the next frame, paused or not.
    let mut way = op("nav_path");
    for _ in 0..30 {
        if way["from"].is_array() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        way = op("nav_path");
    }
    eprintln!("the way from the bunker door ({x:.1}, {z:.1}) to the tap {tap:?}: {way}");
    assert!(way["waypoints"].as_array().is_some_and(|w| !w.is_empty()), "no way on the mesh from the bunker door to the tap: {way}");
}

/// 12. Roxanne leads the player to the tap, and both get there.
#[test]
fn roxanne_leads_the_player_to_the_tap() {
    let _paused = pause();
    out_the_door();
    let her = roxanne();
    let tap = the_tap();
    let there = within(600, || apart(player_spot(), tap).0 < 4.0 && apart(spot_of(her), tap).0 < 4.0);
    assert!(there, "not both at the tap {tap:?} within ten game minutes: the player at {:?}, Roxanne at {:?}", player_spot(), spot_of(her));
}

/// 13. The player's first drink at the tap takes one water from Mara's
/// bunker's store and is "the first drink".
#[test]
fn the_first_drink_takes_one_water_from_maras_store() {
    let _paused = pause();
    at_the_tap();
    let w = op("world");
    let mara = w["cast"].as_array().and_then(|c| c.iter().find(|p| p["part"] == "Mara")).and_then(|p| p["id"].as_u64()).expect("Mara's part");
    let bunker = crate::endpoint::people().into_iter().find(|p| p["id"].as_u64() == Some(mara)).map(|p| p["faction"].as_str().unwrap_or_default().to_string()).expect("Mara's bunker");
    let water = || op("world")["bunkers"][bunker.as_str()]["water"].as_u64().unwrap_or(0);
    let before = water();
    let tap = the_tap();
    op_with("use", json!({"id": player(), "at": [tap.0, tap.1]}));
    let drank = within(5, || water() < before);
    assert!(drank, "a drink at the tap took no water from Mara's bunker's store ({before})");
    assert!(within(5, || reached("the first drink")), "the first drink at the tap is not reached");
}
