//! Saving and loading (todo 31a), read from the running game: a save is a
//! savestate. The `save` op saves the game as it is and answers with that
//! moment (the tick, the episode, every person by ActorId and every thing
//! by its id, where and how they are); the `load` op answers with the
//! moment it loaded. Each takes seconds.

use crate::endpoint::op_with;
use serde_json::{Value as Json, json};

/// The test's own save, kept in the repo (saves/<name>.save).
const NAME: &str = "save_test";

fn save() -> Json {
    let saved = op_with("save", json!({"name": NAME}));
    assert!(saved["tick"].is_u64(), "the save op answered no moment: {saved}");
    saved
}

fn load() -> Json {
    let loaded = op_with("load", json!({"name": NAME, "full": true}));
    assert!(loaded["tick"].is_u64(), "the load op answered no moment: {loaded}");
    loaded
}

/// Where two moments differ, each difference named: the tick, the episode,
/// a person or thing missing from one, or one of theirs that changed.
fn differences(a: &Json, b: &Json) -> Vec<String> {
    let mut out = Vec::new();
    for key in ["tick", "seed", "chunks_made", "mesh", "next_thing_id", "episode", "cast", "reached"] {
        if a[key] != b[key] {
            out.push(format!("{key}: {} then {}", a[key], b[key]));
        }
    }
    for list in ["people", "things"] {
        let by_id = |m: &Json| -> std::collections::BTreeMap<u64, Json> {
            m[list].as_array().into_iter().flatten().map(|x| (x["id"].as_u64().unwrap_or(0), x.clone())).collect()
        };
        let (first, second) = (by_id(a), by_id(b));
        for (id, x) in &first {
            match second.get(id) {
                None => out.push(format!("{list} {id} gone: {x}")),
                Some(y) if y != x => out.push(format!("{list} {id}: {x} then {y}")),
                _ => {}
            }
        }
        for (id, y) in &second {
            if !first.contains_key(id) {
                out.push(format!("{list} {id} new: {y}"));
            }
        }
    }
    out
}

/// The repo's saves folder, where the game keeps them (it runs from the
/// repo).
fn saves() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("saves")
}

/// A copy of this test's save under `name`, its header line changed by
/// `change`, loaded: the load's error.
fn load_changed(name: &str, change: impl Fn(&mut Json)) -> Json {
    save();
    let text = std::fs::read_to_string(saves().join(format!("{NAME}.save"))).expect("the save is in saves/");
    let (first, rest) = text.split_once('\n').expect("a header line");
    let mut header: Json = serde_json::from_str(first.strip_prefix("// topside save ").expect("the header line")).expect("the header is JSON");
    change(&mut header);
    std::fs::write(saves().join(format!("{name}.save")), format!("// topside save {header}\n{rest}")).expect("writing the changed save");
    // The whole answer: a refusal is the op's error, not its result.
    let answer: Json = ureq::post(crate::endpoint::url())
        .send_json(json!({"op": "load", "args": {"name": name}}))
        .ok()
        .and_then(|r| r.into_body().read_json().ok())
        .expect("the game answers");
    let _ = std::fs::remove_file(saves().join(format!("{name}.save")));
    answer
}

/// Things lying in `after` that stand where a thing of the same kind
/// stood in `saved` under another id: made a second time.
fn made_twice(saved: &Json, after: &Json) -> Vec<String> {
    let things = |m: &Json| m["things"].as_array().cloned().unwrap_or_default();
    let before = things(saved);
    things(after)
        .into_iter()
        .filter(|t| before.iter().any(|s| s["id"] != t["id"] && s["item"] == t["item"] && s["at"] == t["at"] && s["open"] == t["open"]))
        .map(|t| t.to_string())
        .collect()
}

/// A save loads into the world it was made in, whatever world the game is
/// in: the storm shifts the game to another world (a new seed), and the
/// load rolls the save's again, every person and thing as saved; its
/// chunks made again over the next ticks make none of their own things a
/// second time.
#[test]
fn a_save_loads_its_own_world_after_the_storm_shifted_the_game() {
    let (_unpause, saved) = paused_save();
    op_with("storm", json!({}));
    // Stepped a game second at a time until the storm has shifted the
    // world, at most a game minute.
    let shifted = (0..60)
        .map(|_| op_with("step", json!({"ticks": 60}))["seed"].clone())
        .find(|seed| seed.is_u64() && *seed != saved["seed"])
        .expect("the storm shifted the world to a new seed within a game minute");
    eprintln!("saved in seed {}, the storm shifted the game to seed {shifted}", saved["seed"]);
    let loaded = load();
    let diff = differences(&saved, &loaded);
    for d in diff.iter().take(20) {
        eprintln!("  {d}");
    }
    assert!(diff.is_empty(), "the load is not the save: {} differences", diff.len());
    let after = op_with("step", json!({"ticks": 120, "full": true}));
    let twice = made_twice(&saved, &after);
    for t in twice.iter().take(20) {
        eprintln!("  made twice: {t}");
    }
    assert!(twice.is_empty(), "{} things made a second time after the load", twice.len());
}

/// A test whose save does not load fails at once, naming the save and why
/// (authority.md "Running tests", operator 2026-10-03), and never makes the
/// save again on its own: a save that loads starts it; the same save with
/// its header line taken away fails the start, naming it, and is left as it
/// was, not written over.
#[test]
fn a_tests_start_fails_at_once_when_its_save_will_not_load() {
    const START: &str = "save_test_start";
    let file = saves().join(format!("{START}.save"));
    op_with("save", json!({"name": START}));
    assert!(!crate::endpoint::start_from(START, || panic!("a start played to make its save")), "the save did not load");
    let text = std::fs::read_to_string(&file).expect("the start was saved");
    let (_, rest) = text.split_once('\n').expect("a header line");
    std::fs::write(&file, rest).expect("taking the save's header line away");
    let failed = std::panic::catch_unwind(|| crate::endpoint::start_from(START, || panic!("a start played to make its save"))).expect_err("a save that does not read started the test");
    let why = failed.downcast_ref::<String>().cloned().unwrap_or_default();
    assert!(why.contains(START) && why.contains("did not load"), "the failure does not name the save and say it did not load: {why}");
    assert_eq!(std::fs::read_to_string(&file).expect("the save is still there"), rest, "the save that did not load was written over");
    let _ = std::fs::remove_file(&file);
}

/// Every save carries over (operator, 2026-10-03): a save whose header says
/// an older version and other content, as older saves do, loads.
#[test]
fn a_save_from_an_older_game_loads() {
    let answer = load_changed("save_test_old_version", |h| {
        h["version"] = json!(0);
        h["content"] = json!(1);
    });
    eprintln!("load answered {answer}");
    assert!(answer["ok"] == true, "a save marked an older version and other content did not load: {answer}");
    crate::endpoint::loaded();
}

/// Lets the game run again when the test ends, passed or failed.
struct Unpause;

impl Drop for Unpause {
    fn drop(&mut self) {
        op_with("pause", json!({"on": false}));
    }
}

/// How many ticks each run plays from the save: ten game seconds.
const PLAYED: u64 = 600;

/// Paused, the moment saved, and the game left paused until the test ends.
fn paused_save() -> (Unpause, Json) {
    let unpause = Unpause;
    let paused = op_with("pause", json!({"on": true}));
    assert_eq!(paused["paused"], true, "the game did not pause: {paused}");
    (unpause, save())
}

/// Played `PLAYED` ticks from where the game stands: the moment there.
fn play() -> Json {
    op_with("step", json!({"ticks": PLAYED, "full": true}))
}

/// Prints where two runs ended apart, and answers whether they ended the
/// same.
fn same_end(what: &str, first: &Json, again: &Json) -> bool {
    let diff = differences(first, again);
    eprintln!("{what}: tick {}, {} differences", again["tick"], diff.len());
    for d in diff.iter().take(20) {
        eprintln!("  {d}");
    }
    diff.is_empty()
}

/// What every test from a save depends on (todo 9k: a test loads its save
/// and drives the game only by stepping): the same save loaded and played
/// the same ticks ends at the same moment every time, every person and
/// thing where it was and as it was.
#[test]
fn a_save_loaded_and_played_the_same_ticks_ends_the_same() {
    let (_unpause, saved) = paused_save();
    load();
    let first = play();
    assert_eq!(first["tick"].as_u64(), saved["tick"].as_u64().map(|t| t + PLAYED), "the run did not play {PLAYED} ticks: {}", first["tick"]);
    load();
    let again = play();
    assert!(same_end("the second run from the save", &first, &again), "the same save loaded and played {PLAYED} ticks ended differently");
}

/// A savestate is exact (Factorio's saves, which its replays depend on):
/// the game played on from the moment it was saved, and the save loaded
/// and played the same ticks, end at the same moment.
#[test]
fn a_save_played_on_and_loaded_and_played_end_the_same() {
    let (_unpause, saved) = paused_save();
    let live = play();
    assert_eq!(live["tick"].as_u64(), saved["tick"].as_u64().map(|t| t + PLAYED), "the game did not play {PLAYED} ticks: {}", live["tick"]);
    load();
    let loaded = play();
    assert!(same_end("the run from the save against the game played on", &live, &loaded), "the game played on and its save played the same ticks ended differently");
}

/// Following and leading survive a load (the pivot 2 save loaded with the
/// player no longer following Roxanne): from pivot point 1's save, "Show
/// me." chosen, the player following Roxanne and she leading them; saved
/// and loaded, both still hold.
#[test]
fn following_and_leading_survive_a_load() {
    use crate::opening::{SECOND, choose, choosing, roxanne, within};
    let _p = crate::endpoint::pause();
    choosing();
    choose(&["Show me.", "Let's go."]);
    let her = roxanne();
    let player = crate::opening::player();
    let held = || {
        crate::endpoint::op("player")["following"].as_u64() == Some(her) && op_with("npc", json!({"id": her}))["leading"]["who"].as_u64() == Some(player)
    };
    assert!(within(30, held), "setup: the player not following Roxanne, or she not leading them, after \"Show me.\": following {}, leading {}", crate::endpoint::op("player")["following"], op_with("npc", json!({"id": her}))["leading"]);
    save();
    load();
    // The player op is written once a game second.
    crate::endpoint::step(SECOND);
    assert!(held(), "after a load: the player following {}, Roxanne leading {}", crate::endpoint::op("player")["following"], op_with("npc", json!({"id": her}))["leading"]);
}

/// Save, load, load again: both loads hold exactly what was saved, every
/// person with the ActorId they had, every thing with its id, at the tick
/// it was saved at.
#[test]
fn a_save_loads_as_it_was_twice() {
    let (_unpause, saved) = paused_save();
    let people = saved["people"].as_array().map_or(0, |p| p.len());
    let things = saved["things"].as_array().map_or(0, |t| t.len());
    eprintln!("saved at tick {}: {people} people, {things} things, episode {}", saved["tick"], saved["episode"]);
    assert!(people > 0 && things > 0, "nothing to save: {people} people, {things} things");
    for n in 1..=2 {
        // The game played on, so a load has something to undo.
        play();
        let loaded = load();
        assert!(same_end(&format!("load {n} against the save"), &saved, &loaded), "load {n} is not the save");
    }
}
