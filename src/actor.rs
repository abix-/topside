//! What every actor carries. The player and every NPC are the same
//! kind of actor on the same structs; only who drives them differs
//! (design.md). Three separate item holders, each its own thing:
//! the inventory, the hotbar, and the equipment slots. Survival
//! stats live in survival.rs, the same way.
//!
//! These are Bevy components wrapping modforge's engine-agnostic
//! types; modforge owns the rules, this file owns the binding.

use bevy::prelude::*;
use modforge::hud::HOTBAR_SLOTS;
use modforge::item;

pub const INVENTORY_SLOTS: usize = 20;

/// The main inventory.
#[derive(Component)]
pub struct Inventory(pub item::Inventory);

impl Default for Inventory {
    fn default() -> Self {
        Self(item::Inventory::new(INVENTORY_SLOTS))
    }
}

/// The hotbar: its own small inventory, always on screen.
#[derive(Component)]
pub struct Hotbar(pub item::Inventory);

impl Default for Hotbar {
    fn default() -> Self {
        Self(item::Inventory::new(HOTBAR_SLOTS))
    }
}

/// Worn and held gear, one named slot each.
#[derive(Component, Default)]
pub struct Equipment(pub item::Equipment);
