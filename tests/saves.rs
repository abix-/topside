//! The play-through (authority.md "Running tests": save states for
//! everything, operator 2026-10-02): episode 1 played once, in order, and a
//! save made just before each event a test checks, so the test loads it and
//! never waits through a long game span. Each save is made from the one
//! before it by playing; each maker is its own ignored test, run by
//! `pwsh -NoProfile -File scripts/build.ps1 saves` (allowed past a test's 2
//! minutes), again whenever content changes; the saves are committed. A
//! maker whose event the game does not do yet fails, and the tests that
//! load its save fail with it, saying so.

use serde_json::json;

use crate::endpoint::{op, op_with, people, save_just_before, start_from, step};
use crate::opening::{SECOND, apart, at_the_tap, choose, choosing, player, player_spot, roxanne, spot_of, the_tap, within, woke};

pub(crate) fn part(name: &str) -> u64 {
    op("world")["cast"].as_array().and_then(|c| c.iter().find(|p| p["part"] == name)).and_then(|p| p["id"].as_u64()).unwrap_or_else(|| panic!("no one plays {name}"))
}

pub(crate) fn faction_of(id: u64) -> String {
    people().into_iter().find(|p| p["id"].as_u64() == Some(id)).and_then(|p| p["faction"].as_str().map(str::to_string)).unwrap_or_default()
}

pub(crate) fn of_faction(faction: &str) -> Vec<u64> {
    people().into_iter().filter(|p| p["faction"] == faction && p["dead"] != true).filter_map(|p| p["id"].as_u64()).collect()
}

pub(crate) fn days_of_water(faction: &str) -> Option<f64> {
    op("world")["bunkers"][faction]["days_of_water"].as_f64()
}

/// Walk someone (the player when `who` is None) to a ground point.
pub(crate) fn walk_to(who: Option<u64>, at: (f64, f64, f64)) {
    let mut c = json!({"do": "move_to", "x": at.0, "y": at.2, "z": at.1});
    if let Some(id) = who {
        c["id"] = json!(id);
    }
    op_with("command", c);
}

/// Whether one of Mara's people stands within 8 m of the tap.
fn maras_at_the_tap() -> bool {
    let maras = faction_of(part("Mara"));
    let tap = the_tap();
    of_faction(&maras).iter().any(|id| apart(spot_of(*id), tap).0 < 8.0)
}

/// The newest person in the world (ActorIds are issued in order, so the
/// highest is whoever came last).
pub(crate) fn newest() -> u64 {
    people().into_iter().filter_map(|p| p["id"].as_u64()).max().unwrap_or(0)
}

/// `giver` gives `to` a water `times` times through the talk path's Offer
/// (the one way the game raises a feeling: a gift).
pub(crate) fn gifts(giver: u64, to: u64, times: u32) {
    for _ in 0..times {
        op_with("give", json!({"id": giver, "item": "water", "count": 1, "clear": true}));
        step(SECOND);
        op_with("talk", json!({"id": giver, "to": to, "said": {"Offer": {"slot": 0}}}));
        step(SECOND);
    }
}

/// The player, beside `id`, asks them to stay with the player's bunker
/// ("You can stay with us.", the-tap.md pivot point 6, draft).
pub(crate) fn ask_to_join(id: u64) {
    op_with("use", json!({"id": player(), "person": id}));
    assert!(within(30, || crate::opening::choices().iter().any(|c| c == "You can stay with us.")), "the player cannot ask {id} to stay: {:?}", crate::opening::choices());
    crate::opening::choose(&["You can stay with us."]);
}

/// The stranger the player stands beside (the nearest of no faction).
pub(crate) fn the_stranger() -> u64 {
    let me = player_spot();
    people()
        .into_iter()
        .filter(|p| p["dead"] != true && p["faction"].is_null())
        .filter_map(|p| p["id"].as_u64())
        .min_by(|a, b| apart(spot_of(*a), me).0.total_cmp(&apart(spot_of(*b), me).0))
        .expect("a living stranger")
}

// The saves, each from the one before.

/// At the tap, just before Mara's people come to it.
pub(crate) fn tap_maras_people_coming() {
    start_from("tap_maras_people_coming", || {
        at_the_tap();
        save_just_before("tap_maras_people_coming", 60, maras_at_the_tap);
    });
}

/// Not believing Roxanne, just before Mara's people come to the tap.
pub(crate) fn tap_not_believed() {
    start_from("tap_not_believed", || {
        choosing();
        choose(&["Water doesn't just run out there. You're wrong.", "I don't believe it."]);
        save_just_before("tap_not_believed", 60, maras_at_the_tap);
    });
}

/// Mara at the tap, the player at it too, not yet within talking reach.
pub(crate) fn tap_mara_at_the_tap() {
    start_from("tap_mara_at_the_tap", || {
        at_the_tap();
        let mara = part("Mara");
        let tap = the_tap();
        walk_to(Some(mara), tap);
        assert!(within(900, || apart(spot_of(mara), tap).0 < 4.0), "Mara never came to the tap");
    });
}

/// Knowing the tap, just before Roxanne's store passes 10 days of water.
pub(crate) fn tap_store_nine_days() {
    start_from("tap_store_nine_days", || {
        at_the_tap();
        save_just_before("tap_store_nine_days", 120, || days_of_water("player").is_some_and(|d| d > 10.0));
    });
}

/// Roxanne's store just past 10 days of water.
pub(crate) fn tap_store_past_ten_days() {
    start_from("tap_store_past_ten_days", || {
        tap_store_nine_days();
        assert!(within(120, || days_of_water("player").is_some_and(|d| d > 10.0)), "the store never passed 10 days");
    });
}

/// Mara's people knowing the tap, just before strangers come for the water.
pub(crate) fn tap_strangers_coming() {
    start_from("tap_strangers_coming", || {
        at_the_tap();
        let before = newest();
        save_just_before("tap_strangers_coming", 120, || newest() > before);
    });
}

/// Strangers have come; the newest is one of them.
pub(crate) fn tap_a_stranger_came() {
    start_from("tap_a_stranger_came", || {
        tap_strangers_coming();
        let before = newest();
        assert!(within(120, || newest() > before), "no stranger came");
    });
}

/// Mara's people holding the tap and strangers come, the player at home:
/// just before the fight starts.
pub(crate) fn tap_fight_morning() {
    start_from("tap_fight_morning", || {
        tap_a_stranger_came();
        let room = op("world")["bunker_doors"].as_array().into_iter().flatten().find(|d| d["floor"].as_f64().is_some_and(|f| f < -5.0)).cloned().expect("the player's room door");
        walk_to(None, (room["at"][0].as_f64().unwrap_or(0.0), room["at"][1].as_f64().unwrap_or(0.0), room["floor"].as_f64().unwrap_or(-6.0)));
        let hurt = |p: &serde_json::Value| p["health"].as_f64().unwrap_or(100.0) < 100.0 || p["dead"] == true;
        save_just_before("tap_fight_morning", 180, || {
            let maras = faction_of(part("Mara"));
            people().iter().any(|p| p["faction"] == maras.as_str() && hurt(p))
        });
    });
}

/// Mara's store run dry by drinking at the tap, the player beside it.
pub(crate) fn tap_at_maras_dry_store() {
    start_from("tap_at_maras_dry_store", || {
        at_the_tap();
        let maras = faction_of(part("Mara"));
        let tap = the_tap();
        for _ in 0..4000 {
            if op("world")["bunkers"][maras.as_str()]["dry"] == true {
                break;
            }
            op_with("use", json!({"id": player(), "at": [tap.0, tap.1]}));
            step(SECOND / 2);
        }
        assert_eq!(op("world")["bunkers"][maras.as_str()]["dry"], true, "Mara's store never read dry");
        let store = op("world")["bunkers"][maras.as_str()]["store"].clone();
        let at = (store["at"][0].as_f64().expect("where Mara's store is"), store["at"][1].as_f64().unwrap_or(0.0), store["floor"].as_f64().unwrap_or(0.0));
        walk_to(None, at);
        assert!(within(900, || apart(player_spot(), at).0 < 1.5), "the player never reached Mara's store");
    });
}

/// Mara's store a few drinks above reading dry, the player at the tap.
pub(crate) fn tap_maras_store_nearly_dry() {
    start_from("tap_maras_store_nearly_dry", || {
        woke();
        let maras = faction_of(part("Mara"));
        let full = op("world")["bunkers"][maras.as_str()]["water"].as_u64().unwrap_or(0);
        at_the_tap();
        let tap = the_tap();
        let water = || op("world")["bunkers"][maras.as_str()]["water"].as_u64().unwrap_or(0);
        while water() * 4 > full + 20 {
            op_with("use", json!({"id": player(), "at": [tap.0, tap.1]}));
            step(SECOND / 2);
        }
    });
}

/// The player beside Mara's child.
pub(crate) fn tap_beside_maras_child() {
    start_from("tap_beside_maras_child", || {
        at_the_tap();
        let child = part("Mara's child");
        walk_to(None, spot_of(child));
        assert!(within(900, || apart(player_spot(), spot_of(child)).0 < 2.5), "the player never reached Mara's child");
    });
}

/// A raider walking for a bunker person's held place, just before they
/// come within 8 m.
pub(crate) fn holding_raider_coming() {
    start_from("holding_raider_coming", || {
        woke();
        let everyone = people();
        let alive = |def: &str| everyone.iter().filter(|p| p["dead"] != true && p["def"] == def && p["faction"] != "player").filter_map(|p| p["id"].as_u64()).next();
        let holder = alive("bunker person").expect("a living person of another bunker");
        let raider = alive("raider").expect("a living raider");
        let place = spot_of(holder);
        op_with("command", json!({"id": holder, "do": "hold", "x": place.0, "y": place.2, "z": place.1}));
        walk_to(Some(raider), place);
        save_just_before("holding_raider_coming", 30, || apart(spot_of(raider), place).0 < 8.0);
    });
}

/// Just before a bunker with someone very afraid of not having enough
/// passes 10 days of water.
pub(crate) fn stocking_past_need() {
    start_from("stocking_past_need", || {
        woke();
        save_just_before("stocking_past_need", 120, || {
            people().iter().any(|p| {
                p["traits"]["fear of not having enough"].as_f64().is_some_and(|f| f >= 0.9) && p["faction"].as_str().and_then(days_of_water).is_some_and(|d| d > 10.0)
            })
        });
    });
}

/// Roxanne told Mara of the tap; just before newcomers roll at the edge.
pub(crate) fn newcomers_word_told() {
    start_from("newcomers_word_told", || {
        woke();
        let mara = part("Mara");
        let key = op("world")["things"].as_array().and_then(|t| t.iter().find(|t| t["name"] == "tap")).and_then(|t| t["key"].as_u64()).expect("the tap's key");
        walk_to(Some(roxanne()), spot_of(mara));
        assert!(within(900, || apart(spot_of(roxanne()), spot_of(mara)).0 < 2.5), "Roxanne never reached Mara");
        op_with("talk", json!({"id": roxanne(), "to": mara, "said": {"Tell": {"key": key}}}));
        step(SECOND);
        let before = newest();
        save_just_before("newcomers_word_told", 70, || newest() > before);
    });
}

/// The player beside a stranger of no faction.
pub(crate) fn beside_a_stranger() {
    start_from("beside_a_stranger", || {
        woke();
        let stranger = people().into_iter().find(|p| p["dead"] != true && p["faction"].is_null()).and_then(|p| p["id"].as_u64()).expect("a living stranger");
        walk_to(None, spot_of(stranger));
        assert!(within(900, || apart(player_spot(), spot_of(stranger)).0 < 2.5), "the player never reached the stranger");
    });
}

/// A stranger who joined the player's bunker, Mara beside them.
pub(crate) fn joined_with_mara_near() {
    start_from("joined_with_mara_near", || {
        beside_a_stranger();
        let stranger = the_stranger();
        gifts(player(), stranger, 3);
        ask_to_join(stranger);
        assert!(within(10, || faction_of(stranger) == "player"), "the stranger did not join");
        let mara = part("Mara");
        walk_to(Some(mara), spot_of(stranger));
        assert!(within(900, || apart(spot_of(mara), spot_of(stranger)).0 < 2.5), "Mara never reached the stranger");
    });
}

// The makers: each its own ignored test, run by build.ps1 saves.

macro_rules! makers {
    ($($save:ident),* $(,)?) => {
        $(
            #[test]
            #[ignore]
            fn $save() {
                let _p = pause();
                super::$save();
            }
        )*
    };
}

mod make {
    use crate::endpoint::pause;
    makers!(
        tap_maras_people_coming,
        tap_not_believed,
        tap_mara_at_the_tap,
        tap_store_nine_days,
        tap_store_past_ten_days,
        tap_strangers_coming,
        tap_a_stranger_came,
        tap_fight_morning,
        tap_maras_store_nearly_dry,
        tap_at_maras_dry_store,
        tap_beside_maras_child,
        holding_raider_coming,
        stocking_past_need,
        newcomers_word_told,
        beside_a_stranger,
        joined_with_mara_near,
    );
}
