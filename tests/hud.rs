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

/// Todo 14e: a clicked slot is selected and the item information panel
/// (Rust's) shows its item: name, description, INFORMATION, and ACTIONS
/// ending in Drop.
#[test]
fn the_selected_slot_shows_its_item_information() {
    let hud = op("hud");
    let selected = &hud["selected"];
    eprintln!("{selected:#}");
    assert!(!selected.is_null(), "nothing is selected yet: open the inventory and click an item");
    let filled = hud["filled"].as_array().expect("the hud op gives the filled slots");
    let slot = filled
        .iter()
        .find(|s| s["holder"] == selected["holder"] && s["index"] == selected["index"])
        .expect("the selected slot is one that holds something");
    let item = slot["item"].as_str().expect("its item");
    assert_eq!(selected["name"], item.to_uppercase(), "the panel names the selected item");
    assert!(selected["description"].as_str().is_some_and(|d| !d.is_empty()), "it has a description");
    assert!(selected["information"].is_array(), "it has its INFORMATION");
    let actions = selected["actions"].as_array().expect("its ACTIONS");
    assert_eq!(actions.last(), Some(&Json::from("Drop")), "the last action is Drop");
}

/// Todo 14d: every filled slot shows its item's icon, and each of its
/// layers', pictures scripts/models.py rendered from the models.
#[test]
fn every_filled_slot_shows_icons_rendered_from_the_models() {
    let hud = op("hud");
    let filled = hud["filled"].as_array().expect("the hud op gives the filled slots");
    let icons = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/icons");
    for slot in filled {
        eprintln!("{} {} holds {}: icons {}", slot["holder"], slot["index"], slot["item"], slot["icons"]);
        let shown = slot["icons"].as_array().expect("a slot's icons");
        assert!(!shown.is_empty(), "{} {} holds {} with no icon", slot["holder"], slot["index"], slot["item"]);
        for name in shown {
            let file = icons.join(format!("{}.png", name.as_str().expect("an icon's name")));
            assert!(file.exists(), "{} is shown but {} does not exist", name, file.display());
        }
    }
    assert!(!filled.is_empty(), "the player carries nothing yet: no slot to read an icon from");
}
