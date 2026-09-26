//! The inventory screen in Rust's layout (todo 14b, 14c): the character
//! with the row of 7 equipment slots on the left, the inventory 6 by 4 in
//! the middle over the belt of 6, the open box on the right. Read from
//! the running game's `hud` op.

use crate::endpoint::op;
use serde_json::Value as Json;

fn panel<'a>(hud: &'a Json, holder: &str) -> &'a Json {
    hud["panels"]
        .as_array()
        .and_then(|list| list.iter().find(|p| p["holder"] == holder))
        .unwrap_or_else(|| panic!("the hud op has no {holder} panel: {hud}"))
}

#[test]
fn the_inventory_screen_is_rusts_layout() {
    let hud = op("hud");
    eprintln!("{hud:#}");
    let (worn, inventory, belt, container) =
        (panel(&hud, "Equipment"), panel(&hud, "Inventory"), panel(&hud, "Hotbar"), panel(&hud, "Container"));
    assert_eq!(worn["slots"], 7, "the equipment row");
    assert_eq!(inventory["slots"], 24, "the inventory, 6 by 4");
    assert_eq!(belt["slots"], 6, "the belt");
    assert_eq!(container["slots"], 24, "the open box");
    let x = |p: &Json| p["x"].as_f64().expect("a panel's x");
    let y = |p: &Json| p["y"].as_f64().expect("a panel's y");
    assert!((x(inventory) - 0.5).abs() < 0.01, "the inventory stands in the middle");
    assert!((x(belt) - 0.5).abs() < 0.01, "the belt stands in the middle");
    assert!(y(inventory) < y(belt), "the inventory stands over the belt");
    assert!(x(worn) < x(inventory), "the equipment row stands left of the inventory");
    assert!(x(container) > x(inventory), "the open box stands right of the inventory");
}
