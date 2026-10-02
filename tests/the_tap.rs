//! The Tap past its opening (docs/episodes/the-tap.md): every pivot point
//! and its choices, what the episode pushes all the way, the endings, and
//! what carries across the storm, one behaviour a test. The opening's own
//! tests are tests/opening.rs; each system these lean on is also tested
//! alone (holding, stocking, noticing, newcomers, joining, drawn_from,
//! endings). Lines and pivot names marked draft in the-tap.md are tested
//! as written until the operator corrects them.
//!
//! Every test starts from one of the opening's saves (tests/opening.rs),
//! keeps the game paused, and moves it only with `step`. What a test needs
//! set up past a save (Mara at the tap, a stranger following) is given by
//! the ops the game already has (command, talk, give, use), never by
//! setting a pivot.

use serde_json::{Value as Json, json};

use crate::endpoint::{op, op_with, pause, people, step};
use crate::opening::{
    SECOND, apart, at_the_tap, choices, choose, choosing, heard_from, out_the_door, player, player_spot, reached, roxanne, spot_of, the_tap, within,
};

/// The person playing a part of The Tap ("Dell" is Roxanne's part).
fn part(name: &str) -> u64 {
    op("world")["cast"].as_array().and_then(|c| c.iter().find(|p| p["part"] == name)).and_then(|p| p["id"].as_u64()).unwrap_or_else(|| panic!("no one plays {name}"))
}

fn faction_of(id: u64) -> String {
    people().into_iter().find(|p| p["id"].as_u64() == Some(id)).and_then(|p| p["faction"].as_str().map(str::to_string)).unwrap_or_default()
}

/// Everyone alive of a faction.
fn of_faction(faction: &str) -> Vec<u64> {
    people().into_iter().filter(|p| p["faction"] == faction && p["dead"] != true).filter_map(|p| p["id"].as_u64()).collect()
}

/// A bunker as the world op reports it, by its faction.
fn bunker(faction: &str) -> Json {
    op("world")["bunkers"][faction].clone()
}

fn says(by: u64, ways: &[&str]) -> bool {
    heard_from(by).iter().any(|(_, w)| ways.contains(&w.as_str()))
}

/// Walk someone (the player when `who` is None) to a ground point at a
/// height, with the command op.
fn walk_to(who: Option<u64>, at: (f64, f64, f64)) {
    let mut c = json!({"do": "move_to", "x": at.0, "y": at.2, "z": at.1});
    if let Some(id) = who {
        c["id"] = json!(id);
    }
    op_with("command", c);
}

/// The player's room door, 6 m down: coming home.
fn home() -> (f64, f64, f64) {
    let door = op("world")["bunker_doors"].as_array().into_iter().flatten().find(|d| d["floor"].as_f64().is_some_and(|f| f < -5.0)).cloned().expect("the player's room door");
    (door["at"][0].as_f64().unwrap_or(0.0), door["at"][1].as_f64().unwrap_or(0.0), door["floor"].as_f64().unwrap_or(-6.0))
}

/// Mara brought to the tap and the player within talking reach of her
/// there (setup by the command op: the meeting is what is tested).
fn meet_mara_at_the_tap() -> u64 {
    let mara = part("Mara");
    let tap = the_tap();
    walk_to(Some(mara), tap);
    assert!(within(300, || apart(spot_of(mara), tap).0 < 4.0), "setup: Mara never came to the tap");
    walk_to(None, spot_of(mara));
    assert!(within(30, || apart(player_spot(), spot_of(mara)).0 < 2.5), "setup: the player never came within talking reach of Mara");
    mara
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

/// Not believing her, Mara's people find the tap first: within ten game
/// minutes one of Mara's bunker holds it.
#[test]
fn not_believing_roxanne_maras_people_find_the_tap_first() {
    let _p = pause();
    choosing();
    choose(&["Water doesn't just run out there. You're wrong.", "I don't believe it."]);
    let maras = faction_of(part("Mara"));
    let tap = the_tap();
    let held = within(600, || of_faction(&maras).iter().any(|id| apart(spot_of(*id), tap).0 < 8.0));
    assert!(held, "none of Mara's people came to the tap within ten game minutes");
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
    assert!(within(600, || apart(spot_of(her), tap).0 < 4.0), "Roxanne never reached the tap");
    assert!(within(900, || spot_of(her).2 < -0.5), "Roxanne never came back down into the bunker");
}

// Pivot point 2, who you take.

/// Roxanne leading the player there, the pivot point is "takes Dell".
#[test]
fn going_with_roxanne_is_takes_dell() {
    let _p = pause();
    out_the_door();
    assert!(within(600, || reached("takes Dell")), "the player came to the tap with Roxanne and reached nothing: {}", op("world")["reached"]);
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
    assert!(within(600, || reached("takes no one")), "the player went to the tap alone and reached nothing: {}", op("world")["reached"]);
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
    assert!(within(600, || reached("takes someone else")), "the player went to the tap with {other} following and reached nothing");
}

// Pivot point 3, the first meeting with Mara's people at the tap.

/// At the tap, Mara speaks first: "This is ours. We found it first." or
/// "Walk away. This one's taken.", and the meeting is reached.
#[test]
fn at_the_tap_mara_says_it_is_hers() {
    let _p = pause();
    at_the_tap();
    let mara = meet_mara_at_the_tap();
    assert!(within(10, || says(mara, &["This is ours. We found it first.", "Walk away. This one's taken."])), "Mara said none of her lines at the tap: {:?}", heard_from(mara));
    assert!(within(5, || reached("the meeting")), "the meeting is not reached");
}

/// Each of the player's answers at the meeting reaches its pivot point and
/// Mara answers it.
fn answer_mara(said: &[&str], pivot: &str, replies: &[&str]) {
    let _p = pause();
    at_the_tap();
    let mara = meet_mara_at_the_tap();
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
/// (todo 11z holding): one of them stays within 8 m of it a game minute on.
#[test]
fn maras_people_hold_the_tap() {
    let _p = pause();
    at_the_tap();
    let maras = faction_of(part("Mara"));
    let tap = the_tap();
    let near = || of_faction(&maras).into_iter().find(|id| apart(spot_of(*id), tap).0 < 8.0);
    assert!(within(600, || near().is_some()), "none of Mara's people came to hold the tap within ten game minutes");
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
    assert!(within(600, || reached("tells no one")), "the player came home having told no one, and reached nothing: {}", op("world")["reached"]);
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
    assert!(within(600, || reached("tells everyone")), "the player told Roxanne, came home, and reached nothing: {}", op("world")["reached"]);
}

// Pivot point 5, Roxanne's storing (todo 11ag, operator: too much is water
// for 10 days of the bunker's drinking).

/// Pushed all the way: knowing the tap, Roxanne stores past 10 days of the
/// bunker's drinking (the world op's `days_of_water` for the bunker).
#[test]
fn roxanne_stores_past_ten_days_of_water() {
    let _p = pause();
    at_the_tap();
    let days = || bunker("player")["days_of_water"].as_f64();
    let start = days().expect("the world op reports the player's bunker's days of water");
    assert!(start < 10.0, "the store already holds {start:.1} days of water at the tap; nothing is shown");
    assert!(within(1200, || days().is_some_and(|d| d > 10.0)), "Roxanne never stored past 10 days of water: {:?}", days());
}

/// The player checking the store (Use on it) while it holds more than 10
/// days of water: Roxanne says her line, and the three answers are offered.
#[test]
fn checking_the_full_store_roxanne_explains() {
    let _p = pause();
    at_the_tap();
    assert!(within(1200, || bunker("player")["days_of_water"].as_f64().is_some_and(|d| d > 10.0)), "setup: the store never held past 10 days");
    let store = bunker("player")["store"].clone();
    let at = (store["at"][0].as_f64().expect("the world op reports where the bunker's store is"), store["at"][1].as_f64().unwrap_or(0.0), store["floor"].as_f64().unwrap_or(0.0));
    walk_to(Some(roxanne()), at);
    op_with("use", json!({"id": player(), "at": [at.0, at.1], "height": at.2}));
    let her = roxanne();
    assert!(within(120, || says(her, &["We might need it. You don't know. None of us know.", "It's not stealing if it's ours."])), "Roxanne never explained the store: {:?}", heard_from(her));
    assert!(within(30, || ["Stop. We have enough.", "Then take more."].iter().all(|a| choices().iter().any(|c| c == a))), "the answers are not offered: {:?}", choices());
}

// Pivot point 6, the strangers (todo 11i newcomers; lines draft).

/// Word of the tap brings newcomers: within a game hour of Mara's people
/// knowing it, 2 to 4 people who were not in the world walk toward the tap.
#[test]
fn word_of_the_tap_brings_strangers() {
    let _p = pause();
    at_the_tap();
    let before: Vec<u64> = people().into_iter().filter_map(|p| p["id"].as_u64()).collect();
    let tap = the_tap();
    let newcomers = || -> Vec<u64> { people().into_iter().filter_map(|p| p["id"].as_u64()).filter(|id| !before.contains(id)).collect() };
    assert!(within(3600, || newcomers().len() >= 2), "no newcomers came within a game hour");
    let came = newcomers();
    assert!(came.len() <= 4, "{} newcomers came in one group, more than 4", came.len());
    let start: Vec<f64> = came.iter().map(|id| apart(spot_of(*id), tap).0).collect();
    step(30 * SECOND);
    let nearer = came.iter().zip(&start).filter(|(id, d)| apart(spot_of(**id), tap).0 < **d - 5.0).count();
    assert!(nearer == came.len(), "the newcomers do not all walk toward the tap: {nearer} of {}", came.len());
}

/// Talking with a stranger who came for the water: their line, and the
/// four answers.
#[test]
fn a_stranger_who_came_for_the_water_asks_for_it() {
    let _p = pause();
    at_the_tap();
    let before: Vec<u64> = people().into_iter().filter_map(|p| p["id"].as_u64()).collect();
    assert!(within(3600, || people().iter().any(|p| p["id"].as_u64().is_some_and(|id| !before.contains(&id)))), "setup: no stranger came");
    let stranger = people().into_iter().filter_map(|p| p["id"].as_u64()).find(|id| !before.contains(id)).unwrap_or_default();
    walk_to(None, spot_of(stranger));
    assert!(within(120, || apart(player_spot(), spot_of(stranger)).0 < 2.5), "setup: the player never reached the stranger");
    op_with("use", json!({"id": player(), "person": stranger}));
    assert!(within(10, || says(stranger, &["We heard there's water here.", "Word is you've got water to spare."])), "the stranger said neither line: {:?}", heard_from(stranger));
    let answers = ["What have you got?", "Not for you. Move on.", "Try the people at the tap. They've got plenty.", "You can stay with us."];
    assert!(within(30, || answers.iter().all(|a| choices().iter().any(|c| c == a))), "the four answers are not offered: {:?}", choices());
}

// Pivot point 7, Mara's well (operator: reads dry under a quarter).

/// Drinking at the tap takes from Mara's store until it reads dry (todo
/// 11ad); checking it there while dry is "finds Mara's well dry".
#[test]
fn checking_maras_dry_store_finds_out() {
    let _p = pause();
    at_the_tap();
    let maras = faction_of(part("Mara"));
    let tap = the_tap();
    for _ in 0..500 {
        if bunker(&maras)["dry"] == true {
            break;
        }
        op_with("use", json!({"id": player(), "at": [tap.0, tap.1]}));
        step(SECOND / 2);
    }
    assert_eq!(bunker(&maras)["dry"], true, "Mara's store never read dry from drinking at the tap: {}", bunker(&maras));
    let store = bunker(&maras)["store"].clone();
    let at = (store["at"][0].as_f64().expect("the world op reports where Mara's store is"), store["at"][1].as_f64().unwrap_or(0.0), store["floor"].as_f64().unwrap_or(0.0));
    walk_to(None, at);
    assert!(within(900, || apart(player_spot(), at).0 < 1.5), "setup: the player never reached Mara's store");
    op_with("use", json!({"id": player(), "at": [at.0, at.1], "height": at.2}));
    assert!(within(5, || reached("finds Mara's well dry")), "checking Mara's dry store reached nothing: {}", op("world")["reached"]);
}

// Pivot point 8, Mara's child (operator: asks whenever the player is near).

/// Near Mara's child, they ask for water, and the four answers are
/// offered.
#[test]
fn maras_child_asks_for_water() {
    let _p = pause();
    at_the_tap();
    let child = part("Mara's child");
    walk_to(None, spot_of(child));
    assert!(within(900, || apart(player_spot(), spot_of(child)).0 < 2.5), "setup: the player never reached Mara's child");
    assert!(within(10, || says(child, &["Is there water today?", "I'm thirsty."])), "Mara's child never asked: {:?}", heard_from(child));
    let answers = ["Here. Take this water.", "What will Mara give for it?", "Not today.", "Tell Mara it's hers if she stands with us."];
    assert!(within(30, || answers.iter().all(|a| choices().iter().any(|c| c == a))), "the four answers are not offered: {:?}", choices());
}

/// "Here. Take this water.": a water leaves the player for the child, and
/// "brings Mara water" is reached.
#[test]
fn bringing_the_child_water() {
    let _p = pause();
    at_the_tap();
    let child = part("Mara's child");
    op_with("give", json!({"id": player(), "item": "water", "count": 1}));
    step(SECOND);
    walk_to(None, spot_of(child));
    assert!(within(900, || apart(player_spot(), spot_of(child)).0 < 2.5), "setup: the player never reached Mara's child");
    let water = |id: u64| op_with("npc", json!({"id": id}))["carrying"].as_array().into_iter().flatten().filter(|s| s["item"] == "water").map(|s| s["count"].as_u64().unwrap_or(0)).sum::<u64>();
    let child_had = water(child);
    assert!(within(30, || choices().iter().any(|c| c == "Here. Take this water.")), "setup: the answer is not offered");
    choose(&["Here. Take this water."]);
    assert!(within(5, || reached("brings Mara water")), "bringing the child water reached nothing: {}", op("world")["reached"]);
    assert!(water(child) > child_had, "the child has no more water than before");
}

// Pivot point 9, the fight for the tap (draft: it starts the first game
// morning after Mara's people hold the tap and strangers have come).

/// The fight starts whether or not the player is there: with the player at
/// home, Mara's people and the strangers fight at the tap.
#[test]
fn the_fight_for_the_tap_starts_without_the_player() {
    let _p = pause();
    at_the_tap();
    walk_to(None, home());
    let maras = faction_of(part("Mara"));
    let alive_at_start = of_faction(&maras).len();
    let before: Vec<u64> = people().into_iter().filter_map(|p| p["id"].as_u64()).collect();
    let strangers_hit = || people().into_iter().filter(|p| p["id"].as_u64().is_some_and(|id| !before.contains(&id))).any(|p| p["health"].as_f64().unwrap_or(100.0) < 100.0 || p["dead"] == true);
    let maras_hit = || of_faction(&maras).len() < alive_at_start || people().into_iter().filter(|p| p["faction"] == maras.as_str()).any(|p| p["health"].as_f64().unwrap_or(100.0) < 100.0);
    assert!(within(7200, || strangers_hit() && maras_hit()), "no fight at the tap within two game hours with the player at home");
}

// Pivot point 10, the last choice, and the endings (draft).

/// Breaking the tap (Use on it with a tool carried) between the storm's
/// warning and the storm is "breaks it", and the ending shown before the
/// storm is "Break it".
#[test]
fn breaking_the_tap_ends_in_break_it() {
    let _p = pause();
    at_the_tap();
    let tap = the_tap();
    op_with("give", json!({"id": player(), "item": "pipe", "count": 1}));
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
    assert!(within(600, || op("world")["seed"] != seed), "the storm never came");
    let things = op("world")["things"].clone();
    assert!(!things.as_array().into_iter().flatten().any(|t| t["name"] == "tap"), "the tap is still in the world after the storm: {things}");
    assert!(op("player")["heard"].as_array().map_or(0, |h| h.len()) >= heard_before, "the player forgot what they heard");
    let knows_now: Vec<u64> = op_with("npc", json!({"id": her}))["knows"].as_array().into_iter().flatten().filter_map(|k| k["key"].as_u64()).collect();
    let people_kept = knew.iter().filter(|k| knows_now.contains(k)).count();
    assert!(people_kept > 0 || knew.is_empty(), "Roxanne forgot everything she knew across the storm");
}

/// The next episode is cast from what people remember of the player: every
/// part is played by someone who feels something toward the player.
#[test]
fn the_next_episode_is_cast_by_feeling() {
    let _p = pause();
    at_the_tap();
    let _ = meet_mara_at_the_tap();
    let seed = op("world")["seed"].clone();
    op_with("storm", json!({}));
    assert!(within(600, || op("world")["seed"] != seed), "the storm never came");
    let w = op("world");
    let cast = w["cast"].as_array().cloned().unwrap_or_default();
    assert!(!cast.is_empty(), "the next episode ({}) has no one cast", w["episode"]);
    for p in &cast {
        let id = p["id"].as_u64().unwrap_or(0);
        let feels = op_with("npc", json!({"id": id}))["feelings"].as_array().into_iter().flatten().any(|f| f["toward"].as_u64() == Some(player()));
        assert!(feels, "{} ({id}) in the next episode feels nothing toward the player", p["part"]);
    }
}
