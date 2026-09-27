//! Realities (realities.md): the storm hands over a reality, and the
//! running game's `world` op reports the one it is in now with its
//! rules.

use crate::endpoint::op;

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
