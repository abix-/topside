//! The Tap past its opening (docs/episodes/the-tap.md): every pivot point
//! and its choices, what the episode pushes all the way, the endings, and
//! what carries across the storm, one behaviour a test. The opening's own
//! tests are tests/opening.rs; each system these lean on is also tested
//! alone (holding, stocking, newcomers, joining, drawn_from). Lines and
//! pivot names marked draft in the-tap.md are tested as written until the
//! operator corrects them.
//!
//! Every test starts from a save (authority.md "Running tests": save states
//! for everything): the opening's (tests/opening.rs) or one just before its
//! event (tests/saves.rs), reached once organically and saved, then loaded;
//! it keeps the game paused and moves it only with `step`. What a test
//! needs past its save is given by the ops the game already has (command,
//! talk, give, use), never by setting a pivot.

use serde_json::json;

use crate::endpoint::{op, op_with, pause, people, step};
use crate::opening::{SECOND, apart, at_the_tap, choices, choose, choosing, heard_from, out_the_door, player, player_spot, reached, roxanne, spot_of, the_tap, within};
use crate::saves::{days_of_water, faction_of, newest, of_faction, part, walk_to};

fn says(by: u64, ways: &[&str]) -> bool {
    heard_from(by).iter().any(|(_, w)| ways.contains(&w.as_str()))
}

/// The player's room door, 6 m down: coming home.
fn home() -> (f64, f64, f64) {
    let door = op("world")["bunker_doors"].as_array().into_iter().flatten().find(|d| d["floor"].as_f64().is_some_and(|f| f < -5.0)).cloned().expect("the player's room door");
    (door["at"][0].as_f64().unwrap_or(0.0), door["at"][1].as_f64().unwrap_or(0.0), door["floor"].as_f64().unwrap_or(-6.0))
}

/// From the save with Mara at the tap, the player walks within talking
/// reach of her.
fn meet_mara() -> u64 {
    crate::saves::tap_mara_at_the_tap();
    let mara = part("Mara");
    walk_to(None, spot_of(mara));
    assert!(within(60, || apart(player_spot(), spot_of(mara)).0 < 2.5), "setup: the player never came within talking reach of Mara");
    mara
}

/// Where a bunker's store stands, from the world op.
fn store_of(faction: &str) -> (f64, f64, f64) {
    let store = op("world")["bunkers"][faction]["store"].clone();
    (store["at"][0].as_f64().expect("the world op reports where the bunker's store is"), store["at"][1].as_f64().unwrap_or(0.0), store["floor"].as_f64().unwrap_or(0.0))
}

/// Put `item` from the player's inventory in their hand the way the player
/// does: open the inventory (Tab), drag it onto the first hotbar slot with
/// the mouse, close it, and press 1.
fn hold_in_hand(item: &str) {
    op_with("key", json!({"key": "Tab"}));
    step(SECOND / 4);
    let hud = op("hud");
    let from = hud["filled"].as_array().into_iter().flatten().find(|s| s["holder"] == "Inventory" && s["item"] == item && s["at"].is_array()).map(|s| s["at"].clone());
    let to = hud["slots"].as_array().into_iter().flatten().find(|s| s["holder"] == "Hotbar" && s["index"] == 0).map(|s| s["at"].clone());
    let (Some(from), Some(to)) = (from, to) else {
        panic!("setup: no {item} shown in the inventory, or no first hotbar slot shown: {hud}");
    };
    op_with("pointer", json!({"drag": {"from": from, "to": to}}));
    step(SECOND / 4);
    op_with("key", json!({"key": "Tab"}));
    step(SECOND / 4);
    op_with("key", json!({"key": "1"}));
    step(SECOND / 4);
}

// Pivot point 1, the other answers.

/// "I don't believe it.": the pivot point is "does not believe it", Roxanne
/// answers, and she goes to the tap on her own.
#[test]
fn not_believing_roxanne_she_goes_on_her_own() {
    let _p = pause();
    choosing();
    let her = roxanne();
    let tap = the_tap();
    let words = choose(&["Water doesn't just run out there. You're wrong.", "I don't believe it."]);
    assert!(within(5, || reached("does not believe it")), "{words:?} reached nothing: {}", op("world")["reached"]);
    assert!(within(5, || says(her, &["Suit yourself. I'll go on my own.", "Then stay thirsty."])), "Roxanne never answered {words:?}");
    let from = apart(spot_of(her), tap).0;
    assert!(within(60, || apart(spot_of(her), tap).0 < from - 10.0), "Roxanne did not go toward the tap on her own: still {:.0} m off", apart(spot_of(her), tap).0);
}

/// Not believing her, Mara's people find the tap first: from just before
/// they come, one of Mara's bunker comes to it.
#[test]
fn not_believing_roxanne_maras_people_find_the_tap_first() {
    let _p = pause();
    crate::saves::tap_not_believed();
    assert!(reached("does not believe it"), "the save is not after not believing Roxanne");
    let maras = faction_of(part("Mara"));
    let tap = the_tap();
    assert!(within(120, || of_faction(&maras).iter().any(|id| apart(spot_of(*id), tap).0 < 8.0)), "none of Mara's people came to the tap");
}

/// "Check it again first.": the pivot point is "sends Dell", Roxanne
/// answers, goes to the tap alone, and comes back home.
#[test]
fn sending_roxanne_she_goes_alone_and_comes_back() {
    let _p = pause();
    choosing();
    let her = roxanne();
    let tap = the_tap();
    let words = choose(&["Go and look again. I'll wait.", "Check it again first."]);
    assert!(within(5, || reached("sends Dell")), "{words:?} reached nothing: {}", op("world")["reached"]);
    assert!(within(5, || says(her, &["Fine. Don't drink anything while I'm gone.", "Back soon."])), "Roxanne never answered {words:?}");
    assert!(within(300, || apart(spot_of(her), tap).0 < 4.0), "Roxanne never reached the tap");
    assert!(within(300, || spot_of(her).2 < -0.5), "Roxanne never came back down into the bunker");
}

// Pivot point 2, who you take.

/// Roxanne leading the player there, the pivot point is "takes Dell".
#[test]
fn going_with_roxanne_is_takes_dell() {
    let _p = pause();
    out_the_door();
    assert!(within(300, || reached("takes Dell")), "the player came to the tap with Roxanne and reached nothing: {}", op("world")["reached"]);
}

/// Going to the tap with no one following or leading, the pivot point is
/// "takes no one".
#[test]
fn going_alone_is_takes_no_one() {
    let _p = pause();
    choosing();
    choose(&["Where exactly?", "Tell me the way again."]);
    op_with("key", json!({"key": "Escape"}));
    step(SECOND);
    walk_to(None, the_tap());
    assert!(within(300, || reached("takes no one")), "the player went to the tap alone and reached nothing: {}", op("world")["reached"]);
    assert!(!reached("takes Dell"), "going alone reached takes Dell");
}

/// Another person of the player's bunker following them there, the pivot
/// point is "takes someone else".
#[test]
fn going_with_another_of_the_bunker_is_takes_someone_else() {
    let _p = pause();
    choosing();
    let other = of_faction("player").into_iter().find(|id| *id != roxanne() && *id != player()).expect("another person in the player's bunker besides Roxanne (todo 11af joining)");
    choose(&["Where exactly?", "Tell me the way again."]);
    op_with("key", json!({"key": "Escape"}));
    op_with("command", json!({"id": other, "do": "follow", "who": player()}));
    walk_to(None, the_tap());
    assert!(within(300, || reached("takes someone else")), "the player went to the tap with {other} following and reached nothing");
}

// Pivot point 3, the first meeting with Mara's people at the tap.

/// At the tap, Mara speaks first: "This is ours. We found it first." or
/// "Walk away. This one's taken.", and the meeting is reached.
#[test]
fn at_the_tap_mara_says_it_is_hers() {
    let _p = pause();
    let mara = meet_mara();
    assert!(within(10, || says(mara, &["This is ours. We found it first.", "Walk away. This one's taken."])), "Mara said none of her lines at the tap: {:?}", heard_from(mara));
    assert!(within(5, || reached("the meeting")), "the meeting is not reached");
}

/// One of the player's answers at the meeting reaches its pivot point and
/// Mara answers it.
fn answer_mara(said: &[&str], pivot: &str, replies: &[&str]) {
    let _p = pause();
    let mara = meet_mara();
    assert!(within(30, || choices().iter().any(|c| said.contains(&c.as_str()))), "none of {said:?} offered at the meeting: {:?}", choices());
    let words = choose(said);
    assert!(within(5, || reached(pivot)), "{words:?} did not reach {pivot}: {}", op("world")["reached"]);
    assert!(within(5, || says(mara, replies)), "Mara never answered {words:?}: {:?}", heard_from(mara));
}

#[test]
fn talking_with_mara_at_the_tap() {
    answer_mara(&["There's enough for both of us."], "talks with Mara", &["Enough for today. Ask again tomorrow.", "Maybe. Don't get greedy."]);
}

#[test]
fn threatening_mara_at_the_tap() {
    answer_mara(&["Step away from it."], "threatens Mara", &["Try it.", "You'll regret that."]);
}

#[test]
fn leaving_mara_the_tap() {
    answer_mara(&["Keep it."], "leaves Mara the tap", &["Smart.", "Good. Go."]);
}

/// Pushed all the way: Mara's people find the tap, camp there and guard it
/// (todo 11z holding): from just before they come, one comes and is still
/// within 8 m of it a game minute on.
#[test]
fn maras_people_hold_the_tap() {
    let _p = pause();
    crate::saves::tap_maras_people_coming();
    let maras = faction_of(part("Mara"));
    let tap = the_tap();
    let near = || of_faction(&maras).into_iter().find(|id| apart(spot_of(*id), tap).0 < 8.0);
    assert!(within(120, || near().is_some()), "none of Mara's people came to hold the tap");
    let holder = near().unwrap_or_default();
    step(60 * SECOND);
    assert!(apart(spot_of(holder), tap).0 < 8.0, "Mara's person {holder} left the tap within a game minute: they hold nothing");
}

// Pivot point 4, what you tell your people.

/// Drinking at the tap and coming home having told no one: "tells no one".
#[test]
fn coming_home_having_told_no_one() {
    let _p = pause();
    at_the_tap();
    let tap = the_tap();
    op_with("use", json!({"id": player(), "at": [tap.0, tap.1]}));
    step(SECOND);
    walk_to(None, home());
    assert!(within(300, || reached("tells no one")), "the player came home having told no one, and reached nothing: {}", op("world")["reached"]);
}

/// Having told Roxanne of the tap (the talk path's Tell), coming home is
/// "tells everyone" while the bunker is the player and Roxanne.
#[test]
fn coming_home_having_told_roxanne() {
    let _p = pause();
    at_the_tap();
    let tap = the_tap();
    op_with("use", json!({"id": player(), "at": [tap.0, tap.1]}));
    step(SECOND);
    let key = op("world")["things"].as_array().and_then(|t| t.iter().find(|t| t["name"] == "tap")).and_then(|t| t["key"].as_u64()).expect("the tap's key");
    walk_to(None, spot_of(roxanne()));
    assert!(within(120, || apart(player_spot(), spot_of(roxanne())).0 < 2.5), "setup: the player never came to Roxanne");
    op_with("talk", json!({"id": player(), "to": roxanne(), "said": {"Tell": {"key": key}}}));
    step(SECOND);
    walk_to(None, home());
    assert!(within(300, || reached("tells everyone")), "the player told Roxanne, came home, and reached nothing: {}", op("world")["reached"]);
}

// Pivot point 5, Roxanne's storing (todo 11ag, operator: too much is water
// for 10 days of the bunker's drinking).

/// Pushed all the way: knowing the tap, Roxanne stores past 10 days of the
/// bunker's drinking: from just before, the store passes 10 days.
#[test]
fn roxanne_stores_past_ten_days_of_water() {
    let _p = pause();
    crate::saves::tap_store_nine_days();
    let start = days_of_water("player").expect("the world op reports the player's bunker's days of water");
    assert!(start <= 10.0, "the save already holds {start:.1} days; it is not just before");
    assert!(within(120, || days_of_water("player").is_some_and(|d| d > 10.0)), "Roxanne never stored past 10 days of water: {:?}", days_of_water("player"));
}

/// The player checking the store (Use on it) while it holds more than 10
/// days of water: Roxanne says her line, and the answers are offered.
#[test]
fn checking_the_full_store_roxanne_explains() {
    let _p = pause();
    crate::saves::tap_store_past_ten_days();
    let at = store_of("player");
    walk_to(Some(roxanne()), at);
    walk_to(None, at);
    assert!(within(300, || apart(player_spot(), at).0 < 1.5), "setup: the player never reached the store");
    op_with("use", json!({"id": player(), "at": [at.0, at.1], "height": at.2}));
    let her = roxanne();
    assert!(within(120, || says(her, &["We might need it. You don't know. None of us know.", "It's not stealing if it's ours."])), "Roxanne never explained the store: {:?}", heard_from(her));
    assert!(within(30, || ["Stop. We have enough.", "Then take more."].iter().all(|a| choices().iter().any(|c| c == a))), "the answers are not offered: {:?}", choices());
}

// Pivot point 6, the strangers (todo 11i newcomers; lines draft).

/// Word of the tap brings newcomers: from just before, 2 to 4 people who
/// were not in the world come, walking toward the tap.
#[test]
fn word_of_the_tap_brings_strangers() {
    let _p = pause();
    crate::saves::tap_strangers_coming();
    let last = newest();
    let tap = the_tap();
    let came = || people().into_iter().filter_map(|p| p["id"].as_u64()).filter(|id| *id > last).collect::<Vec<u64>>();
    assert!(within(120, || came().len() >= 2), "no newcomers came: {:?}", came());
    let group = came();
    assert!(group.len() <= 4, "{} newcomers came in one group, more than 4", group.len());
    let start: Vec<f64> = group.iter().map(|id| apart(spot_of(*id), tap).0).collect();
    step(30 * SECOND);
    let nearer = group.iter().zip(&start).filter(|(id, d)| apart(spot_of(**id), tap).0 < **d - 5.0).count();
    assert!(nearer == group.len(), "the newcomers do not all walk toward the tap: {nearer} of {}", group.len());
}

/// Talking with a stranger who came for the water: their line, and the
/// four answers.
#[test]
fn a_stranger_who_came_for_the_water_asks_for_it() {
    let _p = pause();
    crate::saves::tap_a_stranger_came();
    let stranger = newest();
    assert!(within(300, || apart(player_spot(), spot_of(stranger)).0 < 2.5 || { walk_to(None, spot_of(stranger)); false }), "setup: the player never reached the stranger");
    op_with("use", json!({"id": player(), "person": stranger}));
    assert!(within(10, || says(stranger, &["We heard there's water here.", "Word is you've got water to spare."])), "the stranger said neither line: {:?}", heard_from(stranger));
    let answers = ["What have you got?", "Not for you. Move on.", "Try the people at the tap. They've got plenty.", "You can stay with us."];
    assert!(within(30, || answers.iter().all(|a| choices().iter().any(|c| c == a))), "the four answers are not offered: {:?}", choices());
}

// Pivot point 7, Mara's well (operator: reads dry under a quarter).

/// Checking Mara's store while it reads dry is "finds Mara's well dry".
#[test]
fn checking_maras_dry_store_finds_out() {
    let _p = pause();
    crate::saves::tap_at_maras_dry_store();
    let at = store_of(&faction_of(part("Mara")));
    op_with("use", json!({"id": player(), "at": [at.0, at.1], "height": at.2}));
    assert!(within(5, || reached("finds Mara's well dry")), "checking Mara's dry store reached nothing: {}", op("world")["reached"]);
}

// Pivot point 8, Mara's child (operator: asks whenever the player is near).

/// Near Mara's child, they ask for water, and the four answers are offered.
#[test]
fn maras_child_asks_for_water() {
    let _p = pause();
    crate::saves::tap_beside_maras_child();
    let child = part("Mara's child");
    assert!(within(10, || says(child, &["Is there water today?", "I'm thirsty."])), "Mara's child never asked: {:?}", heard_from(child));
    let answers = ["Here. Take this water.", "What will Mara give for it?", "Not today.", "Tell Mara it's hers if she stands with us."];
    assert!(within(30, || answers.iter().all(|a| choices().iter().any(|c| c == a))), "the four answers are not offered: {:?}", choices());
}

/// "Here. Take this water.": a water leaves the player for the child, and
/// "brings Mara water" is reached.
#[test]
fn bringing_the_child_water() {
    let _p = pause();
    crate::saves::tap_beside_maras_child();
    let child = part("Mara's child");
    op_with("give", json!({"id": player(), "item": "water", "count": 1}));
    step(SECOND);
    let water = |id: u64| op_with("npc", json!({"id": id}))["carrying"].as_array().into_iter().flatten().filter(|s| s["item"] == "water").map(|s| s["count"].as_u64().unwrap_or(0)).sum::<u64>();
    let child_had = water(child);
    assert!(within(30, || choices().iter().any(|c| c == "Here. Take this water.")), "setup: the answer is not offered");
    choose(&["Here. Take this water."]);
    assert!(within(5, || reached("brings Mara water")), "bringing the child water reached nothing: {}", op("world")["reached"]);
    assert!(water(child) > child_had, "the child has no more water than before");
}

// Pivot point 9, the fight for the tap (draft: it starts the first game
// morning after Mara's people hold the tap and strangers have come).

/// The fight starts whether or not the player is there: from just before,
/// with the player at home, Mara's people and the strangers fight at the
/// tap.
#[test]
fn the_fight_for_the_tap_starts_without_the_player() {
    let _p = pause();
    crate::saves::tap_fight_morning();
    assert!(player_spot().2 < -0.5, "the save does not have the player at home");
    let maras = faction_of(part("Mara"));
    let hurt = |p: &serde_json::Value| p["health"].as_f64().unwrap_or(100.0) < 100.0 || p["dead"] == true;
    // The strangers: those of no faction who came for the water, at the tap
    // when the save was made.
    let tap = the_tap();
    let strangers: Vec<u64> = people().into_iter().filter(|p| p["faction"].is_null() && p["dead"] != true).filter_map(|p| p["id"].as_u64()).filter(|id| apart(spot_of(*id), tap).0 < 30.0).collect();
    assert!(!strangers.is_empty(), "the save has no strangers at the tap");
    let strangers_hurt = || people().iter().any(|p| p["id"].as_u64().is_some_and(|id| strangers.contains(&id)) && hurt(p));
    let maras_hurt = || people().iter().any(|p| p["faction"] == maras.as_str() && hurt(p));
    assert!(within(120, || strangers_hurt() && maras_hurt()), "no fight at the tap with the player at home");
}

// Pivot point 10, the last choice, and the endings (draft).

/// Breaking the tap (Use on it with a tool in hand: the-tap.md, orchestrator
/// 2026-10-03) between the storm's warning and the storm is "breaks it",
/// and the ending shown before the storm is "Break it".
#[test]
fn breaking_the_tap_ends_in_break_it() {
    let _p = pause();
    at_the_tap();
    let tap = the_tap();
    op_with("give", json!({"id": player(), "item": "pipe", "count": 1}));
    step(SECOND);
    hold_in_hand("pipe");
    op_with("storm", json!({}));
    step(SECOND);
    op_with("use", json!({"id": player(), "at": [tap.0, tap.1]}));
    assert!(within(5, || reached("breaks it")), "using the tap with a pipe carried before the storm reached nothing: {}", op("world")["reached"]);
    assert!(within(120, || op("hud")["ending"]["name"] == "Break it"), "the ending shown is not Break it: {}", op("hud")["ending"]);
}

/// Doing none of the last choices, the ending is "Walk away".
#[test]
fn doing_nothing_ends_in_walk_away() {
    let _p = pause();
    at_the_tap();
    walk_to(None, home());
    op_with("storm", json!({}));
    assert!(within(120, || op("hud")["ending"]["name"] == "Walk away"), "the ending shown is not Walk away: {}", op("hud")["ending"]);
}

// What carries across the storm.

/// The storm takes the tap and everything the episode placed; what people
/// remember stays.
#[test]
fn the_storm_takes_the_tap_and_memory_stays() {
    let _p = pause();
    at_the_tap();
    let her = roxanne();
    let knew: Vec<u64> = op_with("npc", json!({"id": her}))["knows"].as_array().into_iter().flatten().filter_map(|k| k["key"].as_u64()).collect();
    let heard_before = op("player")["heard"].as_array().map_or(0, |h| h.len());
    let seed = op("world")["seed"].clone();
    op_with("storm", json!({}));
    assert!(within(120, || op("world")["seed"] != seed), "the storm never came");
    let things = op("world")["things"].clone();
    assert!(!things.as_array().into_iter().flatten().any(|t| t["name"] == "tap"), "the tap is still in the world after the storm: {things}");
    assert!(op("player")["heard"].as_array().map_or(0, |h| h.len()) >= heard_before, "the player forgot what they heard");
    let knows_now: Vec<u64> = op_with("npc", json!({"id": her}))["knows"].as_array().into_iter().flatten().filter_map(|k| k["key"].as_u64()).collect();
    let kept = knew.iter().filter(|k| knows_now.contains(k)).count();
    assert!(kept > 0 || knew.is_empty(), "Roxanne forgot everything she knew across the storm");
}

/// The next episode is cast from what people remember of the player: every
/// part is played by someone who feels something toward the player.
#[test]
fn the_next_episode_is_cast_by_feeling() {
    let _p = pause();
    let _ = meet_mara();
    let seed = op("world")["seed"].clone();
    op_with("storm", json!({}));
    assert!(within(120, || op("world")["seed"] != seed), "the storm never came");
    let w = op("world");
    let cast = w["cast"].as_array().cloned().unwrap_or_default();
    assert!(!cast.is_empty(), "the next episode ({}) has no one cast", w["episode"]);
    // A part the-fever.md marks as new in that episode (Sil, a travelling
    // doctor) remembers nothing of the player: left out (orchestrator,
    // 2026-10-03).
    for p in cast.iter().filter(|p| p["part"] != "Sil") {
        let id = p["id"].as_u64().unwrap_or(0);
        let feels = op_with("npc", json!({"id": id}))["feelings"].as_array().into_iter().flatten().any(|f| f["toward"].as_u64() == Some(player()));
        assert!(feels, "{} ({id}) in the next episode feels nothing toward the player", p["part"]);
    }
}
