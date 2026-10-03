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

fn water(faction: &str) -> u64 {
    bunker(faction)["water"].as_u64().unwrap_or(0)
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
        if water(maras) == 0 {
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
    assert_eq!(water(&maras), 0, "Mara's store holds water again without being filled");
}

/// Mara's store reads dry once it holds under a quarter of what it held, and
/// not before.
#[test]
fn the_store_reads_dry_under_a_quarter() {
    let _p = pause();
    // What it held when the episode started: the opening's first save.
    crate::opening::woke();
    let maras = maras_bunker();
    let full = water(&maras);
    crate::saves::tap_maras_store_nearly_dry();
    assert!(full >= 4, "setup: Mara's store held {full} at the start, too little to see a quarter");
    let tap = the_tap();
    for _ in 0..2000 {
        let now = water(&maras);
        let dry = bunker(&maras)["dry"] == true;
        assert_eq!(dry, now * 4 < full, "Mara's store holding {now} of {full} reads dry: {dry}");
        if now == 0 {
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
    let said = format!("{} water", water(&maras));
    let prompt = op("hud")["prompt"].clone();
    assert!(prompt.as_str().is_some_and(|p| p.contains(&said)), "Mara's store does not say it holds {said}: the prompt is {prompt}");
}
