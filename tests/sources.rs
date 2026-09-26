//! A livable world (design.md: rivers drinkable, wells scattered, food
//! sources): people eat and drink from them, read from what each
//! person's memory says they ate in the running game's `people` op.

use std::collections::BTreeMap;

use crate::endpoint::people;

/// What a source is called (content: kind `Source`).
const SOURCES: [&str; 3] = ["well", "water", "berry bush"];

#[test]
fn people_eat_and_drink_from_wells_rivers_and_bushes() {
    let list = people();
    let mut uses: BTreeMap<String, u64> = BTreeMap::new();
    let mut users = 0;
    for p in &list {
        let mut used = false;
        for source in SOURCES {
            let n = p["ate_what"][source].as_u64().unwrap_or(0);
            if n > 0 {
                *uses.entry(source.to_string()).or_default() += n;
                used = true;
            }
        }
        users += used as u32;
    }
    eprintln!("people op: {} people, {users} used a source; uses {uses:?}", list.len());
    assert!(users > 0, "nobody in the running game has eaten or drunk from a well, a river, or a bush");
}
