//! Rust's equipment slots (design.md): whatever anyone in the running
//! game wears sits in the slots its item takes (the road sign vest the
//! chest's armor layer), and no two worn items share a slot.

use crate::endpoint::people;

#[test]
fn what_people_wear_sits_in_its_equipment_slots() {
    let list = people();
    assert!(list.iter().all(|p| p["worn"].is_array()), "the people op gives no worn");
    let wearing: Vec<_> = list.iter().filter(|p| p["worn"].as_array().is_some_and(|w| !w.is_empty())).collect();
    eprintln!("{} people, {} wearing something", list.len(), wearing.len());
    for p in &wearing {
        let mut taken = Vec::new();
        for item in p["worn"].as_array().into_iter().flatten() {
            let slots: Vec<&str> = item["slots"].as_array().into_iter().flatten().filter_map(|s| s.as_str()).collect();
            eprintln!("{} wears {} in {slots:?}", p["id"], item["item"]);
            if item["item"] == "road sign vest" {
                assert_eq!(slots, ["ChestArmor"], "{}: the vest is chest armor", p["id"]);
            }
            for slot in slots {
                assert!(!taken.contains(&slot), "{}: two items in {slot}", p["id"]);
                taken.push(slot);
            }
        }
    }
}
