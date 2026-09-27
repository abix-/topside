//! Realities (realities.md): the storm hands over a reality, and the
//! running game's `world` op reports the one it is in now with its
//! rules.

use crate::endpoint::{is_bunker_person, op, people};

/// The realities built so far and their rules: name, danger, night,
/// death sticks (docs/realities/).
const REALITIES: [(&str, &str, bool, bool); 3] = [
    ("Mixed world", "hard", true, true),
    ("Endless Day", "deadly", false, true),
    ("Loop", "deadly", true, false),
];

#[test]
fn the_game_is_in_a_registered_reality_with_its_rules() {
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
    let w = op("world");
    let reality = &w["reality"];
    eprintln!(
        "world op, tick {}: reality {}, night hours {}, night {}",
        w["tick"], reality["name"], w["night_hours"], w["night"]
    );
    if reality["night"] == true {
        assert_eq!(w["night"], w["night_hours"], "{} has night, and it should be night in the night hours", reality["name"]);
        eprintln!("{} has night; the rule for a reality without one is not read this run", reality["name"]);
        return;
    }
    assert_eq!(w["night"], false, "{} has no night, yet the world op reports night", reality["name"]);
    if w["night_hours"] != true {
        eprintln!("not the night hours; no night here proves nothing yet this run");
    }
}

/// Where death does not stick, a topside person who died comes back:
/// alive, with the death in their memory.
#[test]
fn death_does_not_stick_where_it_does_not() {
    let w = op("world");
    let reality = &w["reality"];
    let list = people();
    let topside: Vec<_> = list.iter().filter(|p| !is_bunker_person(p)).collect();
    let died = |p: &&serde_json::Value| p["deaths"].as_array().is_some_and(|d| !d.is_empty());
    let back = topside.iter().filter(|p| died(p) && p["dead"] != true).count();
    let dead = topside.iter().filter(|p| p["dead"] == true).count();
    eprintln!(
        "world op, tick {}: reality {}; {} topside people, {back} came back from a death, {dead} dead now",
        w["tick"],
        reality["name"],
        topside.len()
    );
    if reality["death_sticks"] == true {
        eprintln!("death sticks in {}; the rule for a reality where it does not is not read this run", reality["name"]);
        return;
    }
    assert!(back > 0, "death does not stick in {}, yet no topside person has come back from one", reality["name"]);
}

/// The episode running fits the reality it runs in (episodes.md); the
/// realities each fits, from its file in docs/episodes/.
#[test]
fn the_episode_fits_its_reality() {
    const FITS: [(&str, &[&str]); 5] = [
        ("The Tap", &["Sky Islands"]),
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
}
