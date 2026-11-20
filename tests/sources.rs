//! A livable world (design.md: rivers drinkable, wells scattered, food
//! sources): people eat and drink from them, read from what each
//! person's memory says they ate in the running game's `people` op.

use serde_json::{Value as Json, json};

use crate::endpoint::{op, op_with, pause, people, step};

/// From the game's start (tap_0_you_wake), the living person nearest a
/// well, made thirsty, and the one nearest a berry bush, made hungry, each
/// use it (the `use` op, through the one interact path): within a few game
/// seconds their memory holds what they took from it.
#[test]
fn people_eat_and_drink_from_wells_rivers_and_bushes() {
    let _p = pause();
    crate::opening::you_wake();
    let things = op("world")["sources"].as_array().cloned().unwrap_or_default();
    let list = people();
    let at = |v: &Json| (v["at"][0].as_f64().unwrap_or(f64::NAN), v["at"][1].as_f64().unwrap_or(f64::NAN));
    for (source, need) in [("well", "thirst"), ("berry bush", "hunger")] {
        let it = things.iter().find(|t| t["name"] == source).unwrap_or_else(|| panic!("no {source} in the world op's sources"));
        let (sx, sz) = at(it);
        let who = list
            .iter()
            .filter(|p| p["dead"] != true && p["def"] != "player")
            .min_by(|a, b| {
                let d = |p: &Json| (at(p).0 - sx).powi(2) + (at(p).1 - sz).powi(2);
                d(a).total_cmp(&d(b))
            })
            .and_then(|p| p["id"].as_u64())
            .expect("a living person");
        op_with("needs", json!({"id": who, need: 5}));
        step(1);
        op_with("use", json!({"id": who, "at": [sx, sz], "height": it["height"]}));
        let them = || op_with("people", json!({"ids": [who]})).as_array().and_then(|p| p.first().cloned()).unwrap_or_default();
        let took = crate::opening::within(5, || them()["ate_what"][source].as_u64().unwrap_or(0) > 0);
        let n = them();
        eprintln!("{who} at the {source} {} (height {}): ate_what {}, {need} {}", it["at"], it["height"], n["ate_what"], n[need]);
        assert!(took, "{who}, made short of {need}, used the {source} at {} and took nothing from it: ate_what {}", it["at"], n["ate_what"]);
    }
}
