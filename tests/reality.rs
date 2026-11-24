//! Realities (realities.md): the storm hands over a reality, and the
//! running game's `world` op reports the one it is in now with its
//! rules.

use crate::endpoint::{is_bunker_person, op, op_with, people};

/// The realities built so far and their rules: name, danger, night,
/// death sticks (docs/realities/).
const REALITIES: [(&str, &str, bool, bool); 3] = [
    ("Mixed world", "hard", true, true),
    ("Endless Day", "deadly", false, true),
    ("Loop", "deadly", true, false),
];

#[test]
fn the_game_is_in_a_registered_reality_with_its_rules() {
    crate::opening::you_wake();
    let w = op("world");
    let storms = w["storms"].as_u64().unwrap_or(0);
    let reality = &w["reality"];
    eprintln!("world op, tick {}: {storms} storms; reality {reality}", w["tick"]);
    let name = reality["name"].as_str().expect("the world op reports a reality");
    let &(_, danger, night, death_sticks) = REALITIES
        .iter()
        .find(|(n, ..)| *n == name)
        .unwrap_or_else(|| panic!("'{name}' is not one of the realities built"));
    assert_eq!(reality["danger"], danger, "{name}'s danger");
    assert_eq!(reality["night"], night, "whether {name} has night");
    assert_eq!(reality["death_sticks"], death_sticks, "whether death sticks in {name}");
    if storms == 0 {
        assert_eq!(name, "Mixed world", "before any storm the world is the first reality");
    }
}

/// A reality without night never has one: the `world` op's night is the
/// same night everyone tires by.
#[test]
fn no_night_in_a_reality_without_night() {
    crate::opening::you_wake();
    let w = op("world");
    let reality = &w["reality"];
    eprintln!(
        "world op, tick {}: reality {}, night hours {}, night {}",
        w["tick"], reality["name"], w["night_hours"], w["night"]
    );
    if reality["night"] == true {
        assert_eq!(w["night"], w["night_hours"], "{} has night, and it should be night in the night hours", reality["name"]);
        panic!("{} has night; the rule for a reality without one is not read this run", reality["name"]);
    }
    assert_eq!(w["night"], false, "{} has no night, yet the world op reports night", reality["name"]);
    if w["night_hours"] != true {
        panic!("not the night hours; no night here proves nothing yet this run");
    }
}

/// While paused, the `world` op a game second at a time until `done`
/// holds, at most `secs` game seconds.
fn world_until(secs: u64, done: impl Fn(&serde_json::Value) -> bool) -> serde_json::Value {
    let mut w = op("world");
    for _ in 0..secs {
        if done(&w) {
            break;
        }
        crate::endpoint::step(crate::opening::SECOND);
        w = op("world");
    }
    w
}

/// Each episode everything topside dies and only bunker people come back
/// (operator, 2026-09-27): after a storm into Loop, nobody the storm took
/// is alive again; then a topside person killed there (the `kill` op)
/// comes back alive with the death in their memory. Brings the storm and
/// the death with ops, so it takes about half a minute.
#[test]
fn in_loop_the_killed_come_back_and_the_storm_takes_everything_topside() {
    // From just before a storm into Loop shifts the world, stepped to the
    // shift.
    let _p = crate::endpoint::pause();
    crate::saves::storm_about_to_shift_into_loop();
    let before = op("world")["storms"].as_u64().unwrap_or(0);
    let w = world_until(10, |w| w["storms"].as_u64().unwrap_or(0) > before);
    assert_eq!(w["reality"]["name"], "Loop", "the storm op named Loop, yet the world is in {} (tick {})", w["reality"]["name"], w["tick"]);
    let list = people();
    let topside: Vec<_> = list.iter().filter(|p| !is_bunker_person(p)).collect();
    let of_storm = |p: &&serde_json::Value| p["deaths"].as_array().is_some_and(|d| d.iter().any(|d| d["of"] == "storm"));
    let storm_back = topside.iter().filter(|p| p["dead"] != true && of_storm(p)).count();
    eprintln!("tick {}: Loop, {} topside people, {storm_back} alive after the storm took them", w["tick"], topside.len());
    assert_eq!(storm_back, 0, "the storm takes everything topside for good, yet some are alive again");
    let victim = topside
        .iter()
        .find(|p| p["dead"] != true && !of_storm(p))
        .expect("the new world has a living topside person");
    let id = victim["id"].as_u64().expect("people report an id");
    eprintln!("kill op answered {}", op_with("kill", serde_json::json!({"id": id})));
    let mut back = None;
    for _ in 0..40 {
        crate::endpoint::step(15);
        let now = people().into_iter().find(|p| p["id"].as_u64() == Some(id));
        if let Some(p) = now
            && p["dead"] != true
            && p["deaths"].as_array().is_some_and(|d| d.iter().any(|d| d["of"] == "kill"))
        {
            back = Some(p);
            break;
        }
    }
    let back = back.unwrap_or_else(|| panic!("person {id} was killed in Loop and has not come back within 10 seconds"));
    eprintln!("person {id} came back: deaths {}", back["deaths"]);
}

/// The episode running fits the reality it runs in (episodes.md); the
/// realities each fits, from its file in docs/episodes/.
#[test]
fn the_episode_fits_its_reality() {
    crate::opening::you_wake();
    const FITS: [(&str, &[&str]); 5] = [
        ("The Tap", &["Mixed world", "Sky Islands"]),
        ("The Fever", &[]),
        ("The Other You", &["Mirror"]),
        ("Nobody Dies", &["Loop"]),
        ("The Question", &["Endless Day", "Rot", "Still Storm", "Maze", "Loop"]),
    ];
    let w = op("world");
    let reality = w["reality"]["name"].as_str().expect("the world op reports a reality");
    eprintln!("world op, tick {}: {} storms; reality {reality}, episode {}", w["tick"], w["storms"], w["episode"]);
    let name = w["episode"].as_str().expect("an episode runs: The Fever fits any reality");
    let &(_, fits) = FITS.iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("'{name}' is not an episode written"));
    assert!(fits.is_empty() || fits.contains(&reality), "{name} runs in {reality}, which it does not fit");
    if w["storms"] == 0 {
        assert_eq!(name, "The Tap", "the first episode is The Tap (operator, 2026-09-27)");
    }
}
