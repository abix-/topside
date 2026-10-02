//! Trading (design.md "Trading", authority.md "Trading"), driven as the
//! player does it: "Let's trade." said in conversation by its number key,
//! the trade panel's slots clicked onto the table, Deal clicked; the trader
//! takes or refuses by their prices (modforge `trade`), and what each
//! carries changes only when they take it.

use std::time::Duration;

use serde_json::{Value as Json, json};

use crate::endpoint::{op, op_with, people};

/// Up to `secs` real seconds for `done`, a quarter second apart.
fn within(secs: u64, done: impl Fn() -> bool) -> bool {
    for _ in 0..secs * 4 {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    done()
}

/// How many of `item` a "carrying" list holds.
fn count(carrying: &Json, item: &str) -> u64 {
    carrying.as_array().map_or(0, |c| c.iter().filter(|s| s["item"] == item).filter_map(|s| s["count"].as_u64()).sum())
}

fn players_count(item: &str) -> u64 {
    count(&op("player")["carrying"], item)
}

fn theirs_count(id: u64, item: &str) -> u64 {
    count(&op_with("npc", json!({"id": id}))["carrying"], item)
}

/// Click a shown slot of `holder` holding `item` and not on the table yet
/// (the `hud` op says where it is), as the player's mouse does.
fn click_slot(holder: &str, item: &str) -> bool {
    let hud = op("hud");
    let side = if holder == "Theirs" { "theirs" } else { "mine" };
    let on_table: Vec<u64> = hud["trading"][side].as_array().map_or(Vec::new(), |o| o.iter().filter_map(|o| o[0].as_u64()).collect());
    let filled = hud["filled"].as_array().cloned().unwrap_or_default();
    let Some(at) = filled
        .iter()
        .find(|s| s["holder"] == holder && s["item"] == item && s["at"].is_array() && !s["index"].as_u64().is_some_and(|i| on_table.contains(&i)))
        .map(|s| s["at"].clone())
    else {
        return false;
    };
    op_with("pointer", json!({"at": at, "click": true}));
    true
}

/// Two people trade with each other on their own (design.md "Trading";
/// operator, 2026-10-02: a brain choice when one has extra of what the
/// other needs): one carrying water to spare, the other thirsty with scrap
/// and knowing no water; brought together, the thirsty one's brain trades
/// for water, and both remember it.
#[test]
fn a_thirsty_person_trades_with_one_carrying_water_to_spare() {
    op_with("replay", json!({"stop": true}));
    let strangers: Vec<u64> = people().into_iter().filter(|p| p["faction"].is_null() && p["dead"] != true).filter_map(|p| p["id"].as_u64()).take(8).collect();
    assert!(strangers.len() >= 2, "two living strangers needed, found {}", strangers.len());
    // A pair at a time: where there is water about (a river in sight), the
    // thirsty one drinks it instead, rightly; then the next pair.
    let (mut seller, mut buyer, mut traded) = (0, 0, false);
    for pair in strangers.chunks(2).filter(|p| p.len() == 2) {
        (seller, buyer) = (pair[0], pair[1]);
        op_with("give", json!({"id": seller, "item": "water bottle", "count": 3, "clear": true}));
        op_with("give", json!({"id": buyer, "item": "scrap", "count": 60, "clear": true}));
        op_with("forget", json!({"id": buyer}));
        op_with("needs", json!({"id": buyer, "thirst": 20, "hunger": 90}));
        // The buyer stood a metre from the seller, facing them (the `use`
        // op); then their own brain.
        op_with("use", json!({"id": buyer, "person": seller}));
        let did = |w: &str| op_with("npc", json!({"id": buyer}))["did"].as_array().is_some_and(|d| d.iter().any(|d| d["did"].as_str().is_some_and(|d| d.starts_with(w))));
        traded = within(20, || did(&format!("traded with {seller}")) || did("ate water"));
        traded = traded && did(&format!("traded with {seller}"));
        let b = op_with("npc", json!({"id": buyer}));
        let last: Vec<Json> = b["did"].as_array().map(|d| d.iter().rev().take(3).map(|d| d["did"].clone()).collect()).unwrap_or_default();
        eprintln!(
            "the pair {seller} and {buyer}: traded {traded}; the buyer doing {} about {}, asleep {}, thirst {}, at {} feet {}, the seller at {}, last did {last:?}",
            b["doing"], b["about"], b["asleep"], b["needs"]["thirst"], b["at"], b["feet"], op_with("npc", json!({"id": seller}))["at"]
        );
        if traded {
            break;
        }
    }
    let (b, s) = (op_with("npc", json!({"id": buyer})), op_with("npc", json!({"id": seller})));
    let last = |p: &Json| p["did"].as_array().map(|d| d.iter().rev().take(4).map(|d| d["did"].clone()).collect::<Vec<_>>()).unwrap_or_default();
    eprintln!("the buyer {buyer}: doing {} about {}, at {} feet {}, way {}, carrying {}, last did {:?}", b["doing"], b["about"], b["at"], b["feet"], b["way"], b["carrying"], last(&b));
    eprintln!("the seller {seller}: doing {}, at {} feet {}, carrying {}, last did {:?}", s["doing"], s["at"], s["feet"], s["carrying"], last(&s));
    assert!(traded, "the thirsty one never traded with the one carrying water");
    // Thirsty, they may drink it at once: what they remember says they got
    // it.
    let remembers = |p: &Json, w: &str| p["did"].as_array().is_some_and(|d| d.iter().any(|d| d["did"].as_str().is_some_and(|d| d.starts_with(w) && d.contains("got water bottle") || d.starts_with(w) && d.contains("gave water bottle"))));
    assert!(remembers(&b, &format!("traded with {seller}")), "the buyer does not remember getting water: {:?}", last(&b));
    assert!(remembers(&s, &format!("traded with {buyer}")), "the seller does not remember giving water: {:?}", last(&s));
    assert!(count(&s["carrying"], "scrap") >= 1, "the seller got no scrap");
}

#[test]
fn the_player_trades_with_a_stranger_through_the_panel_and_a_poor_offer_is_refused() {
    crate::endpoint::player_alive();
    op_with("replay", json!({"stop": true}));
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    // A stranger (no faction): nobody an episode sends on errands. Some
    // are busy and never let the player ask; then the next.
    let strangers: Vec<Json> = people().into_iter().filter(|p| p["faction"].is_null() && p["dead"] != true).take(5).collect();
    let mut trading = None;
    for stranger in &strangers {
        let them = stranger["id"].as_u64().expect("their id");
        // Face them and press E (the `use` op on a person); "Let's trade."
        // by its number key, again if the choices changed under it (the
        // `hud` op shows them once a game second).
        let open = within(15, || {
            let hud = op("hud");
            if hud["trading"]["with"].as_u64() == Some(them) {
                return true;
            }
            if let Some(number) = hud["talk"]["choices"].as_array().and_then(|c| c.iter().position(|c| c == "Let's trade.")) {
                op_with("key", json!({"key": (number + 1).to_string()}));
                std::thread::sleep(Duration::from_millis(1000));
            } else if hud["talk"]["to"].as_u64() != Some(them) {
                op_with("use", json!({"id": player, "person": them}));
            }
            false
        });
        eprintln!("asked {} ({them}) to trade: the panel open {open}; the talk box {}", stranger["name"], op("hud")["talk"]);
        if open {
            trading = Some(them);
            break;
        }
        op_with("key", json!({"key": "Escape"}));
    }
    crate::talk_ui::shot("trade_open");
    let them = trading.unwrap_or_else(|| panic!("none of {} strangers would let the player ask to trade", strangers.len()));
    // What each carries, set: the player cloth and scrap, they two waters.
    op_with("give", json!({"id": player, "item": "cloth", "count": 1, "clear": true}));
    op_with("give", json!({"id": player, "item": "scrap", "count": 100}));
    op_with("give", json!({"id": them, "item": "water bottle", "count": 2, "clear": true}));
    within(3, || players_count("scrap") == 100 && theirs_count(them, "water bottle") == 2);

    let before = (players_count("scrap"), players_count("cloth"), players_count("water bottle"));
    let theirs_before = (theirs_count(them, "scrap"), theirs_count(them, "cloth"), theirs_count(them, "water bottle"));
    eprintln!("before: the player scrap, cloth, water {before:?}; the stranger {theirs_before:?}");

    // A poor offer: the cloth (2) for her water (2 x 10, asked at more).
    // Clicked once shown (the `hud` op shows the screen once a game second).
    assert!(within(3, || click_slot("Inventory", "cloth")), "no cloth shown in the player's side");
    assert!(within(3, || click_slot("Theirs", "water bottle")), "no water shown in the stranger's side");
    let on_table = within(5, || op("hud")["trading"]["mine"].as_array().is_some_and(|m| m.len() == 1) && op("hud")["trading"]["theirs"].as_array().is_some_and(|t| t.len() == 1));
    assert!(on_table, "the clicked stacks never went on the table: {}", op("hud")["trading"]);
    let deal = op("hud")["trading"]["deal_at"].clone();
    op_with("pointer", json!({"at": deal, "click": true}));
    let refused = within(5, || op("hud")["trading"]["refused"] == true);
    crate::talk_ui::shot("trade_refused");
    assert!(refused, "cloth for two waters was not refused: {}", op("hud")["trading"]);
    assert_eq!(players_count("water bottle"), before.2, "refused, and the water moved anyway");

    // Scrap too, a stack at a time (it stacks to 20), the deal put again
    // each time, until she takes it.
    let mut traded = false;
    for stacks in 1..=5 {
        let on_table = op("hud")["trading"]["mine"].as_array().map_or(0, |m| m.len());
        if !click_slot("Inventory", "scrap") {
            break;
        }
        within(5, || op("hud")["trading"]["mine"].as_array().is_some_and(|m| m.len() > on_table));
        op_with("pointer", json!({"at": deal, "click": true}));
        traded = within(4, || players_count("water bottle") >= before.2 + 2);
        eprintln!("{stacks} stack(s) of scrap and the cloth: taken {traded}; the trade {}", op("hud")["trading"]);
        if traded {
            break;
        }
    }
    crate::talk_ui::shot("trade_done");
    let after = (players_count("scrap"), players_count("cloth"), players_count("water bottle"));
    let theirs_after = (theirs_count(them, "scrap"), theirs_count(them, "cloth"), theirs_count(them, "water bottle"));
    eprintln!("after: the player scrap, cloth, water {after:?}; the stranger {theirs_after:?}; the trade {}", op("hud")["trading"]);
    op_with("key", json!({"key": "Escape"}));
    assert!(traded, "the deal never went through");
    // Whole stacks change hands: what one side lost, the other got.
    let lost = |b: u64, a: u64| b as i64 - a as i64;
    assert!(lost(before.0, after.0) >= 20 && lost(before.1, after.1) >= 1, "the player paid less than the scrap and cloth put down");
    assert!(lost(after.2, before.2) >= 2, "the player got less than the two waters");
    assert_eq!(
        (lost(before.0, after.0), lost(before.1, after.1), lost(after.2, before.2)),
        (lost(theirs_after.0, theirs_before.0), lost(theirs_after.1, theirs_before.1), lost(theirs_before.2, theirs_after.2)),
        "what one side gave is not what the other got"
    );
}
