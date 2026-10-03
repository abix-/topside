//! A source drawn by another (todo 11ad, operator 2026-10-02), the system
//! alone, on The Tap's tap, drawn from Mara's bunker's store: every drink at
//! the one takes one from the other (tests/opening.rs's first drink proves
//! that part), until it runs dry; the other says how much it holds when
//! looked at or used, and reads dry when low (the-tap.md "Mara's well":
//! under a quarter of what it held). From the opening's save at the tap,
//! paused, moved only by `step`.

use serde_json::{Value as Json, json};

use crate::endpoint::{op, op_with, pause, people, step};
use crate::opening::{SECOND, apart, player, player_spot, the_tap};

fn maras_bunker() -> String {
    let mara = op("world")["cast"].as_array().and_then(|c| c.iter().find(|p| p["part"] == "Mara")).and_then(|p| p["id"].as_u64()).expect("Mara's part");
    people().into_iter().find(|p| p["id"].as_u64() == Some(mara)).and_then(|p| p["faction"].as_str().map(str::to_string)).expect("Mara's bunker")
}

fn bunker(faction: &str) -> Json {
    op("world")["bunkers"][faction].clone()
}

/// The litres of water a bunker's store holds (the world op's "water").
fn water(faction: &str) -> f64 {
    bunker(faction)["water"].as_f64().unwrap_or(0.0)
}

/// How many times the player has drunk at the tap (their memory, the
/// player op's `ate_what`).
fn drunk_at_the_tap() -> u64 {
    op("player")["ate_what"]["tap"].as_u64().unwrap_or(0)
}

/// Drink at the tap until Mara's store holds nothing, at most 2000 drinks.
fn drink_it_dry(maras: &str) {
    let tap = the_tap();
    for _ in 0..2000 {
        if water(maras) <= 0.0 {
            return;
        }
        op_with("use", json!({"id": player(), "at": [tap.0, tap.1]}));
        step(SECOND / 2);
    }
    panic!("Mara's store still holds {} water after 2000 drinks at the tap", water(maras));
}

/// Mara's store run dry, a drink at the tap gives nothing: the player has
/// not drunk again, and the store stays at 0.
#[test]
fn a_dry_store_gives_nothing_at_the_tap() {
    let _p = pause();
    // A few drinks above reading dry (tests/saves.rs).
    crate::saves::tap_maras_store_nearly_dry();
    let maras = maras_bunker();
    drink_it_dry(&maras);
    let drunk = drunk_at_the_tap();
    let tap = the_tap();
    op_with("use", json!({"id": player(), "at": [tap.0, tap.1]}));
    step(2 * SECOND);
    assert_eq!(drunk_at_the_tap(), drunk, "the player drank at the tap with Mara's store dry");
    assert_eq!(water(&maras), 0.0, "Mara's store holds water again without being filled");
}

/// Mara's store reads dry once it holds under a quarter of what it held, and
/// not before.
#[test]
fn the_store_reads_dry_under_a_quarter() {
    let _p = pause();
    // What it held when the episode started: pivot point 1's save.
    crate::opening::choosing();
    let maras = maras_bunker();
    let full = water(&maras);
    crate::saves::tap_maras_store_nearly_dry();
    assert!(full >= 1.0, "setup: Mara's store held {full} L at the start, too little to see a quarter");
    let tap = the_tap();
    for _ in 0..2000 {
        let now = water(&maras);
        let dry = bunker(&maras)["dry"] == true;
        assert_eq!(dry, now * 4.0 < full, "Mara's store holding {now} of {full} L reads dry: {dry}");
        if now <= 0.0 {
            return;
        }
        op_with("use", json!({"id": player(), "at": [tap.0, tap.1]}));
        step(SECOND / 2);
    }
}

/// Used, the store says how much water it holds: the prompt names the
/// count the world holds for it.
#[test]
fn the_store_says_how_much_it_holds() {
    let _p = pause();
    // The player beside Mara's store (tests/saves.rs).
    crate::saves::tap_at_maras_dry_store();
    let maras = maras_bunker();
    let store = bunker(&maras)["store"].clone();
    let at = (store["at"][0].as_f64().expect("the world op reports where Mara's store is"), store["at"][1].as_f64().unwrap_or(0.0), store["floor"].as_f64().unwrap_or(0.0));
    assert!(apart(player_spot(), at).0 < 1.5, "the save does not have the player beside Mara's store");
    op_with("use", json!({"id": player(), "at": [at.0, at.1], "height": at.2}));
    step(SECOND);
    let said = format!("{:.1} L water", water(&maras));
    let prompt = op("hud")["prompt"].clone();
    assert!(prompt.as_str().is_some_and(|p| p.contains(&said)), "Mara's store does not say it holds {said}: the prompt is {prompt}");
}

/// A person whose brain goes to drink at a source with a melee weapon in
/// hand puts it out of hand first, so a thing that breaks when struck (its
/// def's `breaks_when_struck`: the tap) is drunk from, not broken. One of
/// Mara's bunker cast in no part (no errand of the episode holding them; a
/// bunker person holds a pipe from their spawn) is walked to the tap
/// (`move_to`, done on arrival, so their own brain decides after), sees
/// it, and made thirsty drinks there: their drinks at the tap go up, their
/// bunker's store goes down, and nothing is in their hands. From the
/// pivot point 1's save, paused.
#[test]
fn a_person_drinking_at_the_tap_puts_their_weapon_away_first() {
    let _p = pause();
    crate::opening::choosing();
    let maras = maras_bunker();
    let cast: Vec<u64> = op("world")["cast"].as_array().into_iter().flatten().filter_map(|p| p["id"].as_u64()).collect();
    let who = people()
        .into_iter()
        .filter(|p| p["faction"] == maras.as_str() && p["dead"] != true && p["weapon"]["item"].is_string())
        .filter_map(|p| p["id"].as_u64())
        .filter(|id| !cast.contains(id))
        .min()
        .expect("setup: someone of Mara's bunker, in no part, holding a weapon");
    let them = || people().into_iter().find(|p| p["id"].as_u64() == Some(who)).unwrap_or(Json::Null);
    let held = them()["weapon"]["item"].clone();
    let tap = the_tap();
    op_with("command", json!({"id": who, "do": "move_to", "x": tap.0 + 1.5, "y": tap.2, "z": tap.1}));
    assert!(crate::opening::within(120, || apart(crate::opening::spot_of(who), tap).0 < 3.0), "setup: {who} never came within 3 m of the tap: they stand at {:?}", crate::opening::spot_of(who));
    let before = water(&maras);
    let drank = || them()["ate_what"]["tap"].as_u64().unwrap_or(0);
    op_with("needs", json!({"id": who, "thirst": 5}));
    let drinks = crate::opening::within(60, || drank() >= 1);
    if !drinks {
        let npc = op_with("npc", json!({"id": who}));
        let tap_key = op("world")["things"].as_array().and_then(|t| t.iter().find(|t| t["name"] == "tap")).map(|t| t["key"].clone());
        let knows_tap = npc["knows"].as_array().into_iter().flatten().find(|k| Some(&k["key"]) == tap_key.as_ref()).cloned();
        eprintln!("{who}: thirst {} doing {} about {} weapon {} at {:?}; knows the tap: {:?}; things: {}", them()["thirst"], npc["doing"], npc["about"], them()["weapon"], crate::opening::spot_of(who), knows_tap, op("world")["things"]);
    }
    assert!(drinks, "{who}, thirsty at the tap holding {held}, never drank from it (broken?): they did {}", them()["did"]);
    assert!(water(&maras) < before, "{who} drank at the tap but their bunker's store did not go down: {before} then {}", water(&maras));
    assert!(them()["weapon"].is_null(), "{who} drank at the tap with {} still in their hands", them()["weapon"]);
}
