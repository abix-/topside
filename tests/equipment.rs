//! Equipment is one row of 7 plain slots (design.md): whatever anyone in
//! the running game wears sits in one of the 7 slots, one item a slot.

use crate::endpoint::people;

#[test]
fn what_people_wear_sits_in_the_row_of_plain_slots() {
    crate::opening::you_wake();
    let list = people();
    assert!(list.iter().all(|p| p["worn"].is_array()), "the people op gives no worn");
    let wearing: Vec<_> = list.iter().filter(|p| p["worn"].as_array().is_some_and(|w| !w.is_empty())).collect();
    eprintln!("{} people, {} wearing something", list.len(), wearing.len());
    for p in &wearing {
        let mut taken = Vec::new();
        for item in p["worn"].as_array().into_iter().flatten() {
            let slot = item["slot"].as_u64().expect("each worn item has its slot");
            eprintln!("{} wears {} in slot {slot}", p["id"], item["item"]);
            assert!(slot < 7, "{}: slot {slot} past the row of 7", p["id"]);
            assert!(!taken.contains(&slot), "{}: two items in slot {slot}", p["id"]);
            taken.push(slot);
        }
    }
}
