//! Episode 1's save points past the opening (authority.md "Running tests":
//! save points are checkpoints in a speedrun, operator 2026-10-02): each
//! just before an event a test checks, or at it, reached once organically
//! from the one before and saved (`start_from`), then loaded by every test
//! that starts there. One a long game span from the one before is grown a
//! game minute at a time (`grow`), each minute its own checkpoint, so no run
//! plays more than the test limit allows and none starts over.

use serde_json::json;

use crate::endpoint::{Until, grow, op, op_with, people, start_from, step};
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

/// Have someone (the player when `who` is None) follow `them`, who may be
/// walking: a walk to where they stood finds them gone.
pub(crate) fn follow(who: Option<u64>, them: u64) {
    let mut c = json!({"do": "follow", "who": them});
    if let Some(id) = who {
        c["id"] = json!(id);
    }
    op_with("command", c);
}

/// One of Mara's people arriving at the tap (within 8 m) who was not there
/// at the first check after loading: the ones already there are the mark,
/// as `someone_new` marks the newest (orchestrator, 2026-10-03: Mara is
/// asked to the tap from the start, so "anyone within 8 m" had already
/// happened at the save point before).
fn maras_arriving_at_the_tap() -> impl Fn() -> bool {
    let mark: std::cell::RefCell<Option<Vec<u64>>> = std::cell::RefCell::new(None);
    move || {
        let maras = faction_of(part("Mara"));
        let tap = the_tap();
        let there: Vec<u64> = of_faction(&maras).into_iter().filter(|id| apart(spot_of(*id), tap).0 < 8.0).collect();
        let mut mark = mark.borrow_mut();
        let already = mark.get_or_insert_with(|| there.clone());
        there.iter().any(|id| !already.contains(id))
    }
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
        grow("tap_maras_people_coming", 60, at_the_tap, maras_arriving_at_the_tap(), Until::Before);
    });
}

/// Not believing Roxanne, just before Mara's people come to the tap.
pub(crate) fn tap_not_believed() {
    start_from("tap_not_believed", || {
        let disbelieve = || {
            choosing();
            choose(&["Water doesn't just run out there. You're wrong.", "I don't believe it."]);
        };
        grow("tap_not_believed", 60, disbelieve, maras_arriving_at_the_tap(), Until::Before);
    });
}

/// Mara at the tap, the player at it too, not yet within talking reach.
pub(crate) fn tap_mara_at_the_tap() {
    start_from("tap_mara_at_the_tap", || {
        let send_mara = || {
            at_the_tap();
            walk_to(Some(part("Mara")), the_tap());
        };
        grow("tap_mara_at_the_tap", 15, send_mara, || apart(spot_of(part("Mara")), the_tap()).0 < 4.0, Until::At);
    });
}

/// Knowing the tap, just before Roxanne's store passes 10 days of water.
pub(crate) fn tap_store_nine_days() {
    start_from("tap_store_nine_days", || {
        grow("tap_store_nine_days", 120, at_the_tap, || days_of_water("player").is_some_and(|d| d > 10.0), Until::Before);
    });
}

/// Roxanne's store just past 10 days of water.
pub(crate) fn tap_store_past_ten_days() {
    start_from("tap_store_past_ten_days", || {
        tap_store_nine_days();
        assert!(within(120, || days_of_water("player").is_some_and(|d| d > 10.0)), "the store never passed 10 days");
    });
}

/// Whether someone new has come into the world since the first time this
/// is asked: the newest person then is the mark. A grown chain asks first
/// right after its latest minute loads, and until someone comes no one
/// newer is there, so the mark is the same whichever minute it starts from.
pub(crate) fn someone_new() -> impl Fn() -> bool {
    let mark = std::cell::Cell::new(None);
    move || {
        let now = newest();
        if mark.get().is_none() {
            mark.set(Some(now));
        }
        now > mark.get().unwrap_or(now)
    }
}

/// Mara's people knowing the tap, just before strangers come for the water.
pub(crate) fn tap_strangers_coming() {
    start_from("tap_strangers_coming", || {
        grow("tap_strangers_coming", 120, at_the_tap, someone_new(), Until::Before);
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
        let go_home = || {
            tap_a_stranger_came();
            walk_to(None, crate::opening::middle_of(&crate::opening::bunker_room("below")));
        };
        let hurt = |p: &serde_json::Value| p["health"].as_f64().unwrap_or(100.0) < 100.0 || p["dead"] == true;
        let fighting = || {
            let maras = faction_of(part("Mara"));
            people().iter().any(|p| p["faction"] == maras.as_str() && hurt(p))
        };
        grow("tap_fight_morning", 180, go_home, fighting, Until::Before);
    });
}

/// Mara's store run dry by drinking at the tap, the player beside it.
pub(crate) fn tap_at_maras_dry_store() {
    start_from("tap_at_maras_dry_store", || {
        // Where Mara's store is, from the world op.
        let store_at = || {
            let store = op("world")["bunkers"][faction_of(part("Mara")).as_str()]["store"].clone();
            (store["at"][0].as_f64().expect("where Mara's store is"), store["at"][1].as_f64().unwrap_or(0.0), store["floor"].as_f64().unwrap_or(0.0))
        };
        let drink_dry_and_go = || {
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
            walk_to(None, store_at());
        };
        grow("tap_at_maras_dry_store", 15, drink_dry_and_go, || apart(player_spot(), store_at()).0 < 1.5, Until::At);
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
        let go = || {
            at_the_tap();
            follow(None, part("Mara's child"));
        };
        grow("tap_beside_maras_child", 15, go, || apart(player_spot(), spot_of(part("Mara's child"))).0 < 3.0, Until::At);
    });
}

/// The first living person of another bunker, and the first living raider.
fn holder_and_raider() -> (u64, u64) {
    let everyone = people();
    let alive = |def: &str| everyone.iter().filter(|p| p["dead"] != true && p["def"] == def && p["faction"] != "player").filter_map(|p| p["id"].as_u64()).next();
    (alive("bunker person").expect("a living person of another bunker"), alive("raider").expect("a living raider"))
}

/// A raider walking for a bunker person's held place, just before they
/// come within 8 m.
pub(crate) fn holding_raider_coming() {
    start_from("holding_raider_coming", || {
        let send_the_raider = || {
            woke();
            let (holder, raider) = holder_and_raider();
            let place = spot_of(holder);
            op_with("command", json!({"id": holder, "do": "hold", "x": place.0, "y": place.2, "z": place.1}));
            walk_to(Some(raider), place);
        };
        let near = || {
            let (holder, raider) = holder_and_raider();
            apart(spot_of(raider), spot_of(holder)).0 < 8.0
        };
        grow("holding_raider_coming", 30, send_the_raider, near, Until::Before);
    });
}

/// Just before a bunker with someone very afraid of not having enough
/// passes 10 days of water.
pub(crate) fn stocking_past_need() {
    start_from("stocking_past_need", || {
        let past = || {
            people().iter().any(|p| {
                p["traits"]["fear of not having enough"].as_f64().is_some_and(|f| f >= 0.9) && p["faction"].as_str().and_then(days_of_water).is_some_and(|d| d > 10.0)
            })
        };
        grow("stocking_past_need", 120, woke, past, Until::Before);
    });
}

/// Roxanne beside Mara, sent there from the opening's first save.
pub(crate) fn newcomers_roxanne_at_mara() {
    start_from("newcomers_roxanne_at_mara", || {
        let send = || {
            woke();
            follow(Some(roxanne()), part("Mara"));
        };
        grow("newcomers_roxanne_at_mara", 15, send, || apart(spot_of(roxanne()), spot_of(part("Mara"))).0 < 2.5, Until::At);
    });
}

/// Roxanne told Mara of the tap; just before newcomers roll at the edge.
pub(crate) fn newcomers_word_told() {
    start_from("newcomers_word_told", || {
        let tell = || {
            newcomers_roxanne_at_mara();
            let key = op("world")["things"].as_array().and_then(|t| t.iter().find(|t| t["name"] == "tap")).and_then(|t| t["key"].as_u64()).expect("the tap's key");
            op_with("talk", json!({"id": roxanne(), "to": part("Mara"), "said": {"Tell": {"key": key}}}));
            step(SECOND);
        };
        grow("newcomers_word_told", 70, tell, someone_new(), Until::Before);
    });
}

/// The first living stranger of no faction.
fn a_stranger() -> u64 {
    people().into_iter().find(|p| p["dead"] != true && p["faction"].is_null()).and_then(|p| p["id"].as_u64()).expect("a living stranger")
}

/// The player beside a stranger of no faction.
pub(crate) fn beside_a_stranger() {
    start_from("beside_a_stranger", || {
        let go = || {
            woke();
            follow(None, a_stranger());
        };
        // Within talking reach (3 m): a follower stands once 2.5 m off.
        grow("beside_a_stranger", 15, go, || apart(player_spot(), spot_of(a_stranger())).0 < 3.0, Until::At);
    });
}

/// A stranger who joined the player's bunker, Mara beside them.
pub(crate) fn joined_with_mara_near() {
    start_from("joined_with_mara_near", || {
        // The joined stranger is the newest of the player's bunker.
        let joined = || people().into_iter().filter(|p| p["faction"] == "player" && p["dead"] != true).filter_map(|p| p["id"].as_u64()).max().expect("someone of the player's bunker");
        let join = || {
            beside_a_stranger();
            let stranger = the_stranger();
            gifts(player(), stranger, 3);
            ask_to_join(stranger);
            assert!(within(10, || faction_of(stranger) == "player"), "the stranger did not join");
            walk_to(Some(part("Mara")), spot_of(stranger));
        };
        grow("joined_with_mara_near", 15, join, || apart(spot_of(part("Mara")), spot_of(joined())).0 < 2.5, Until::At);
    });
}
