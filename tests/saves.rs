//! Episode 1's save points past the opening (authority.md "Running tests":
//! save points are checkpoints in a speedrun, operator 2026-10-02): each
//! just before an event a test checks, or at it, reached once organically
//! from the one before and saved (`start_from`), then loaded by every test
//! that starts there. One a long game span from the one before is grown a
//! game minute at a time (`grow`), each minute its own checkpoint, so no run
//! plays more than the test limit allows and none starts over.

use serde_json::{Value as Json, json};

use crate::endpoint::{Until, grow, grow_doing, op, op_with, people, start_from, step};
use crate::opening::{SECOND, apart, at_the_tap, choosing, player, player_spot, roxanne, spot_of, the_tap, within};

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

/// `giver` gives `to` a water bottle `times` times through the talk path's
/// Offer (the one way the game raises a feeling: a gift).
pub(crate) fn gifts(giver: u64, to: u64, times: u32) {
    for _ in 0..times {
        op_with("give", json!({"id": giver, "item": "water bottle", "count": 1, "clear": true}));
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

/// Just after the player threatens Mara at the tap (the pivot "threatens
/// Mara"), from the game's start.
pub(crate) fn tap_mara_threatened() {
    // Played on in real time, then paused for the save's one step.
    let mut paused = None;
    start_from("tap_mara_threatened", || {
        crate::opening::you_wake();
        crate::relationship::threaten_mara();
        paused = Some(crate::endpoint::pause());
    });
    drop(paused);
}

/// Two game seconds before a storm shifts the world: from the game's start,
/// a storm brought by the `storm` op (with `args`, such as the reality it
/// hands over) and played through its warning and 18 of its 20 seconds
/// (content STORM). Called paused.
fn storm_about_to_shift(name: &str, args: Json) {
    start_from(name, || {
        crate::opening::you_wake();
        op_with("storm", args);
        for _ in 0..90 {
            step(SECOND);
            if op("world")["storm"] == "storm" {
                break;
            }
        }
        assert_eq!(op("world")["storm"], "storm", "{name}: the storm never hit");
        step(17 * SECOND);
        assert_eq!(op("world")["storm"], "storm", "{name}: the storm ended before its last two seconds");
    });
}

/// Just before a storm shifts the world.
pub(crate) fn storm_about_to_shift_the_world() {
    storm_about_to_shift("storm_about_to_shift", json!({}));
}

/// Just before a storm shifts the world into Loop.
pub(crate) fn storm_about_to_shift_into_loop() {
    storm_about_to_shift("storm_about_to_shift_into_loop", json!({"reality": "Loop"}));
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

/// Mara's bunker's entry in the world op (its store's water, whether it
/// reads dry, what it held when the tap began drawing, where it stands).
fn maras_store() -> Json {
    op("world")["bunkers"][faction_of(part("Mara")).as_str()].clone()
}

/// The player drinking at the tap for at most a game minute, a drink each
/// half second, stopping once `enough`: a minute of a grown drinking chain.
fn drink_at_the_tap_a_minute(enough: &impl Fn() -> bool) {
    let tap = the_tap();
    for _ in 0..2 * 60 {
        if enough() {
            return;
        }
        op_with("use", json!({"id": player(), "at": [tap.0, tap.1]}));
        step(SECOND / 2);
    }
}

/// Mara's store a few drinks above reading dry, the player at the tap:
/// drunk down from the tap a game minute at a time (grown), until it holds
/// at most a quarter of what it held plus 5 (water x 4 <= held + 20).
pub(crate) fn tap_maras_store_nearly_dry() {
    start_from("tap_maras_store_nearly_dry", || {
        let nearly = || {
            let store = maras_store();
            let (water, held) = (store["water"].as_f64().unwrap_or(0.0), store["drawn_held"].as_f64().expect("Mara's store is drawn from (the tap placed)"));
            // A few mouthfuls above a quarter, in litres.
            water * 4.0 <= held + 4.0 * 5.0 * 0.25
        };
        grow_doing("tap_maras_store_nearly_dry", 30, at_the_tap, || drink_at_the_tap_a_minute(&nearly), &nearly, Until::At);
    });
}

/// Mara's store just read dry, the player at the tap: drunk on from nearly
/// dry, a game minute at a time (grown).
pub(crate) fn tap_maras_store_dry() {
    start_from("tap_maras_store_dry", || {
        let dry = || maras_store()["dry"] == true;
        grow_doing("tap_maras_store_dry", 30, tap_maras_store_nearly_dry, || drink_at_the_tap_a_minute(&dry), &dry, Until::At);
    });
}

/// Mara's store run dry by drinking at the tap, the player beside it:
/// walked there from the tap, grown.
pub(crate) fn tap_at_maras_dry_store() {
    start_from("tap_at_maras_dry_store", || {
        let store_at = || {
            let store = maras_store()["store"].clone();
            (store["at"][0].as_f64().expect("where Mara's store is"), store["at"][1].as_f64().unwrap_or(0.0), store["floor"].as_f64().unwrap_or(0.0))
        };
        let go = || {
            tap_maras_store_dry();
            walk_to(None, store_at());
        };
        grow("tap_at_maras_dry_store", 15, go, || apart(player_spot(), store_at()).0 < 1.5, Until::At);
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
/// The living person of `def` outside the player's bunker with the lowest
/// ActorId: the same person every run while they live (ids last through
/// saves and loads).
fn lowest_living(def: &str) -> Option<u64> {
    people().iter().filter(|p| p["dead"] != true && p["def"] == def && p["faction"] != "player").filter_map(|p| p["id"].as_u64()).min()
}

/// The raider the holding chain sends: the lowest ActorId living raider.
pub(crate) fn the_raider() -> u64 {
    lowest_living("raider").expect("a living raider")
}

/// The one person holding a place (the people op's `holding`, their saved
/// ai::Holding), and the place as a spot (x, z, height).
pub(crate) fn the_holder() -> (u64, (f64, f64, f64)) {
    let holding: Vec<(u64, (f64, f64, f64))> = people()
        .iter()
        .filter(|p| p["dead"] != true)
        .filter_map(|p| {
            let at = p["holding"].as_array()?;
            let f = |i: usize| at.get(i).and_then(Json::as_f64).unwrap_or(f64::NAN);
            Some((p["id"].as_u64()?, (f(0), f(2), f(1))))
        })
        .collect();
    assert_eq!(holding.len(), 1, "not exactly one person holding a place: {holding:?}");
    holding[0]
}

/// A raider walking for a bunker person's held place, just before they
/// come within 8 m. The holder is the lowest ActorId living person of
/// another bunker, told to hold where they stand; read again as the one
/// holding, so a run carrying on from a minute reads the same two.
pub(crate) fn holding_raider_coming() {
    start_from("holding_raider_coming", || {
        let send_the_raider = || {
            choosing();
            let holder = lowest_living("bunker person").expect("a living person of another bunker");
            let place = spot_of(holder);
            op_with("command", json!({"id": holder, "do": "hold", "x": place.0, "y": place.2, "z": place.1}));
            walk_to(Some(the_raider()), place);
        };
        // The raider has come: within 8 m now, or its walk there done. A
        // walker at about 15 m/s passes through in seconds, between two
        // checks a game minute apart; its walk ending stays true, so the
        // minute before it is the save point, and the test plays the
        // coming from there.
        let came = || {
            let (_, place) = the_holder();
            let raider = the_raider();
            // Each game minute, where the raider is and what they do, so a
            // chain that never comes shows why.
            let r = op_with("npc", json!({"id": raider}));
            eprintln!(
                "holding_raider_coming tick {}: raider {raider} at {} feet {} dead {} doing {} about {} commands {} way {}; the held place {place:?}",
                r["tick"], r["at"], r["feet"], r["dead"], r["doing"], r["about"], r["commands"], r["way"]
            );
            apart(spot_of(raider), place).0 < 8.0 || r["commands"].is_null()
        };
        grow("holding_raider_coming", 30, send_the_raider, came, Until::Before);
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
        grow("stocking_past_need", 120, choosing, past, Until::Before);
    });
}

/// Roxanne beside Mara, sent there from the opening's first save.
pub(crate) fn newcomers_roxanne_at_mara() {
    start_from("newcomers_roxanne_at_mara", || {
        let send = || {
            choosing();
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

/// The first group of newcomers come: grown from just before they roll to
/// the minute they are in the world (a group rolls at once).
pub(crate) fn newcomers_first_group() {
    start_from("newcomers_first_group", || {
        grow("newcomers_first_group", 5, newcomers_word_told, someone_new(), Until::At);
    });
}

/// After the first group, Roxanne tells Mara of the tap again.
pub(crate) fn newcomers_told_again() {
    start_from("newcomers_told_again", || {
        newcomers_first_group();
        let key = op("world")["things"].as_array().and_then(|t| t.iter().find(|t| t["name"] == "tap")).and_then(|t| t["key"].as_u64()).expect("the tap's key");
        op_with("talk", json!({"id": roxanne(), "to": part("Mara"), "said": {"Tell": {"key": key}}}));
        step(SECOND);
    });
}

/// Word told again after the first group, played on to a game minute
/// before the edge may send its next group (the world op's
/// `edge.next_group_at`), grown.
pub(crate) fn newcomers_before_next_group() {
    start_from("newcomers_before_next_group", || {
        let almost = || {
            let (now, next) = (op("status")["tick"].as_u64().unwrap_or(0), op("world")["edge"]["next_group_at"].as_u64().expect("the world op reports when the edge's next group may come"));
            now + 60 * SECOND >= next
        };
        grow("newcomers_before_next_group", 61, newcomers_told_again, almost, Until::At);
    });
}

/// The player beside a stranger of no faction: the nearest, so the way to
/// them is over chunks made round the player.
pub(crate) fn beside_a_stranger() {
    start_from("beside_a_stranger", || {
        let go = || {
            choosing();
            follow(None, the_stranger());
        };
        // Within talking reach (3 m): a follower stands once 2.5 m off.
        grow("beside_a_stranger", 15, go, || apart(player_spot(), spot_of(the_stranger())).0 < 3.0, Until::At);
    });
}

/// Just after the player takes a stranger in (the-tap.md pivot point 6,
/// "takes a stranger in"): given gifts, asked to stay, and joined.
pub(crate) fn stranger_taken_in() {
    start_from("stranger_taken_in", || {
        beside_a_stranger();
        let stranger = the_stranger();
        gifts(player(), stranger, 3);
        ask_to_join(stranger);
        assert!(within(10, || faction_of(stranger) == "player"), "the stranger did not join");
    });
}

/// A stranger who joined the player's bunker, Mara beside them.
pub(crate) fn joined_with_mara_near() {
    start_from("joined_with_mara_near", || {
        // The joined stranger is the newest of the player's bunker.
        let joined = || people().into_iter().filter(|p| p["faction"] == "player" && p["dead"] != true).filter_map(|p| p["id"].as_u64()).max().expect("someone of the player's bunker");
        let join = || {
            stranger_taken_in();
            walk_to(Some(part("Mara")), spot_of(joined()));
        };
        grow("joined_with_mara_near", 15, join, || apart(spot_of(part("Mara")), spot_of(joined())).0 < 2.5, Until::At);
    });
}
