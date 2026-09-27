//! The episode running now (episodes.md), read from the running game's
//! `world` and `people` ops: its cast from the bunkers' people.

use crate::endpoint::{op, op_with, people};

/// The tap (docs/episodes/the-tap.md): Dell made to drink at it (the `use`
/// op); Dell's memory holds the drink, Mara's bunker's store has one water
/// less, and the first drink is a pivot point reached. Takes seconds.
#[test]
fn a_drink_at_the_tap_takes_from_maras_store_and_is_the_first_drink() {
    let w = op("world");
    if w["episode"] != "The Tap" {
        eprintln!("the episode is {}, not The Tap; the tap is not read this run", w["episode"]);
        return;
    }
    let cast = |part: &str| {
        w["cast"].as_array().and_then(|c| c.iter().find(|p| p["part"] == part)).and_then(|p| p["id"].as_u64()).unwrap_or_else(|| panic!("no {part} cast"))
    };
    let (dell, mara) = (cast("Dell"), cast("Mara"));
    let tap = w["things"].as_array().and_then(|t| t.iter().find(|t| t["name"] == "tap")).expect("The Tap has brought the tap into the world");
    let at = tap["at"].clone();
    let faction = |id: u64| people().into_iter().find(|p| p["id"].as_u64() == Some(id)).map(|p| p["faction"].clone()).expect("the person exists");
    let maras = faction(mara);
    let water = |w: &serde_json::Value| w["bunkers"][maras.as_str().unwrap_or_default()]["water"].as_u64().unwrap_or(0);
    let drank = |id: u64| people().into_iter().find(|p| p["id"].as_u64() == Some(id)).and_then(|p| p["ate_what"]["tap"].as_u64()).unwrap_or(0);
    let (water_before, drank_before) = (water(&w), drank(dell));
    eprintln!("tick {}: tap at {at}; Mara ({mara}) of {maras}, water {water_before}; Dell ({dell}) drank at the tap {drank_before} times", w["tick"]);
    eprintln!("use op answered {}", op_with("use", serde_json::json!({"id": dell, "at": at})));
    let mut after = op("world");
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(250));
        after = op("world");
        if drank(dell) > drank_before && after["reached"].as_array().is_some_and(|r| r.iter().any(|p| p["pivot"] == "the first drink")) {
            break;
        }
    }
    eprintln!("tick {}: Mara's water {}, Dell drank at the tap {} times, reached {}", after["tick"], water(&after), drank(dell), after["reached"]);
    assert!(drank(dell) > drank_before, "Dell was made to drink at the tap and their memory holds no drink there");
    assert!(water(&after) < water_before, "a drink at the tap took no water from Mara's bunker's store");
    assert!(
        after["reached"].as_array().is_some_and(|r| r.iter().any(|p| p["pivot"] == "the first drink")),
        "the first drink at the tap is not reached"
    );
}

/// The Tap's cast (docs/episodes/the-tap.md "The people"): Dell from the
/// player's bunker, Mara from another bunker, Mara's child from Mara's.
#[test]
fn the_taps_cast_comes_from_the_bunkers() {
    let w = op("world");
    eprintln!("world op, tick {}: episode {}, cast {}", w["tick"], w["episode"], w["cast"]);
    if w["episode"] != "The Tap" {
        eprintln!("the episode is {}, not The Tap; its cast is not read this run", w["episode"]);
        return;
    }
    let list = people();
    let faction_of = |part: &str| {
        let id = w["cast"]
            .as_array()
            .and_then(|c| c.iter().find(|p| p["part"] == part))
            .and_then(|p| p["id"].as_u64())
            .unwrap_or_else(|| panic!("The Tap has no {part} cast"));
        let person = list.iter().find(|p| p["id"].as_u64() == Some(id)).unwrap_or_else(|| panic!("{part} ({id}) is not among the people"));
        eprintln!("{part}: person {id} of {}", person["faction"]);
        person["faction"].clone()
    };
    assert_eq!(faction_of("Dell"), "player", "Dell is from the player's bunker");
    let mara = faction_of("Mara");
    assert_ne!(mara, "player", "Mara is from another bunker");
    assert_eq!(faction_of("Mara's child"), mara, "Mara's child is from Mara's bunker");
}
