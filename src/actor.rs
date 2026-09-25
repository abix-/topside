//! What every actor carries (authority.md: the player and every NPC
//! share the same components).

use bevy::prelude::*;
use modforge::item;

/// Slots in a person's inventory (the 3D game's number).
pub const INVENTORY_SLOTS: usize = 20;

/// What a person carries.
#[derive(Component)]
pub struct Inventory(pub item::Inventory);

impl Default for Inventory {
    fn default() -> Self {
        Self(item::Inventory::new(INVENTORY_SLOTS))
    }
}
