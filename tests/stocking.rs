//! Wanting more than enough (todo 11ag, operator 2026-10-02), the system
//! alone: a person stocks past what they need by their personality, the
//! most fearful of not having enough up to twice what is needed; one who is
//! not afraid stops at what is needed. What is needed is water for 10 days
//! of the bunker's drinking (operator, 2026-10-02, the-tap.md "Too much
//! water"), read as each bunker's `days_of_water` in the world op; how
//! afraid a person is, as their "fear of not having enough" in the people
//! op's traits (0 to 1). From the opening's first save, paused, moved only
//! by `step`.

use crate::endpoint::{op, pause, people};
use crate::opening::{within, woke};

/// Each bunker's people by faction, and the most anyone of them fears not
/// having enough.
fn bunkers_by_fear() -> Vec<(String, f64)> {
    let mut most: std::collections::BTreeMap<String, f64> = Default::default();
    for p in people() {
        let (Some(faction), Some(fear)) = (p["faction"].as_str(), p["traits"]["fear of not having enough"].as_f64()) else { continue };
        if !faction.starts_with("bunker") && faction != "player" {
            continue;
        }
        let e = most.entry(faction.to_string()).or_insert(0.0);
        *e = e.max(fear);
    }
    most.into_iter().collect()
}

fn days(faction: &str) -> Option<f64> {
    op("world")["bunkers"][faction]["days_of_water"].as_f64()
}

/// A bunker with someone very afraid of not having enough keeps stocking
/// past 10 days of water, and not past 20; a bunker where no one is afraid
/// stops at 10.
#[test]
fn the_fearful_stock_past_need_and_the_unafraid_stop_at_it() {
    let _p = pause();
    woke();
    let fears = bunkers_by_fear();
    assert!(!fears.is_empty(), "no bunker people report a fear of not having enough in their traits");
    let fearful = fears.iter().find(|(_, f)| *f >= 0.9).map(|(b, _)| b.clone()).expect("a bunker with someone whose fear of not having enough is 0.9 or more");
    let calm = fears.iter().find(|(_, f)| *f <= 0.1).map(|(b, _)| b.clone()).expect("a bunker where no one's fear of not having enough is over 0.1");
    assert!(days(&fearful).is_some() && days(&calm).is_some(), "the world op reports no days of water for {fearful} or {calm}");
    // Twenty game minutes, a game day (survival Day), for both to stock.
    let stocked = within(1200, || days(&fearful).is_some_and(|d| d > 10.0));
    eprintln!("after stocking: {fearful} holds {:?} days, {calm} {:?}", days(&fearful), days(&calm));
    assert!(stocked, "{fearful}, with someone very afraid, never stocked past 10 days: {:?}", days(&fearful));
    assert!(days(&fearful).is_some_and(|d| d <= 20.0), "{fearful} stocked past twice what is needed: {:?} days", days(&fearful));
    // 10.5, not 10: the last trip under 10 days may carry its load a little
    // past it; a bunker that keeps stocking goes on well beyond.
    assert!(days(&calm).is_some_and(|d| d <= 10.5), "{calm}, where no one is afraid, stocked past what is needed: {:?} days", days(&calm));
}
