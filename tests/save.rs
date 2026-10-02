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
    let loaded = op_with("load", json!({"name": NAME}));
    assert!(loaded["tick"].is_u64(), "the load op answered no moment: {loaded}");
    loaded
}

/// Where two moments differ, each difference named: the tick, the episode,
/// a person or thing missing from one, or one of theirs that changed.
fn differences(a: &Json, b: &Json) -> Vec<String> {
    let mut out = Vec::new();
    for key in ["tick", "episode", "cast", "reached"] {
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
    let answer: Json = ureq::post(crate::endpoint::URL)
        .send_json(json!({"op": "load", "args": {"name": name}}))
        .ok()
        .and_then(|r| r.into_body().read_json().ok())
        .expect("the game answers");
    let _ = std::fs::remove_file(saves().join(format!("{name}.save")));
    answer
}

/// A save made in another world is refused, never loading its people
/// onto other ground.
#[test]
fn a_save_from_another_world_is_refused() {
    let answer = load_changed("save_test_other_world", |h| h["seed"] = json!(h["seed"].as_u64().unwrap_or(0) ^ 1));
    eprintln!("load answered {answer}");
    assert!(answer.to_string().contains("another world"), "a save from another world was not refused: {answer}");
}

/// A save of another format's version is refused plainly.
#[test]
fn a_save_of_another_version_is_refused() {
    let answer = load_changed("save_test_old_version", |h| h["version"] = json!(0));
    eprintln!("load answered {answer}");
    assert!(answer.to_string().contains("version 0"), "a save of another version was not refused: {answer}");
}

/// Save, load, load again: both loads hold exactly what was saved, every
/// person with the ActorId they had, every thing with its id, at the tick
/// it was saved at.
#[test]
fn a_save_loads_as_it_was_twice() {
    let saved = save();
    let people = saved["people"].as_array().map_or(0, |p| p.len());
    let things = saved["things"].as_array().map_or(0, |t| t.len());
    eprintln!("saved at tick {}: {people} people, {things} things, episode {}", saved["tick"], saved["episode"]);
    assert!(people > 0 && things > 0, "nothing to save: {people} people, {things} things");
    // Let the game move on, so a load has something to undo.
    std::thread::sleep(std::time::Duration::from_secs(2));
    for n in 1..=2 {
        let loaded = load();
        let diff = differences(&saved, &loaded);
        eprintln!("load {n}: tick {}, {} differences", loaded["tick"], diff.len());
        for d in diff.iter().take(20) {
            eprintln!("  {d}");
        }
        assert!(diff.is_empty(), "load {n} is not the save: {} differences", diff.len());
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}
