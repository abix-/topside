//! Night sends people home and morning sends them out (life.md "Home and
//! the day"), read from the running game's `world` op: at night nearly
//! every bunker person is home; by day, after the first night, some are
//! out.

use crate::endpoint::op;

/// At night, at least this share of the living bunker people is home.
const HOME_AT_NIGHT: f64 = 0.8;

#[test]
fn night_sends_people_home_and_morning_sends_them_out() {
    crate::opening::you_wake();
    let w = op("world");
    let bunkers = w["bunkers"].as_object().cloned().unwrap_or_default();
    let sum = |field: &str| bunkers.values().map(|b| b[field].as_u64().unwrap_or(0)).sum::<u64>();
    let (alive, home, asleep) = (sum("alive"), sum("at_home"), sum("asleep"));
    let night = w["night"].as_bool().unwrap_or(false);
    let day = w["day"].as_f64().unwrap_or(0.0);
    eprintln!("world op, tick {}, day {day:.2}, night {night}: {alive} bunker people alive, {home} at home, {asleep} asleep", w["tick"]);
    if night {
        assert!(
            home as f64 >= HOME_AT_NIGHT * alive as f64,
            "night, but only {home} of {alive} bunker people are home"
        );
    } else {
        assert!(day >= 1.0, "the first night has not come yet (day {day:.2}); read again after it");
        assert!(home < alive, "morning, but all {alive} bunker people are still home");
    }
}
