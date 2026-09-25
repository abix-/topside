//! What every actor carries (authority.md: the player and every NPC
//! share the same components).

use bevy::prelude::*;
use modforge::item;

/// Slots in a person's inventory (the 3D game's number).
pub const INVENTORY_SLOTS: usize = 20;

/// Which way a person faces, in radians: 0 is along +x, a quarter turn
/// is +y (up the screen).
#[derive(Component, Default, Clone, Copy, Debug, PartialEq)]
pub struct Facing(pub f32);

impl Facing {
    /// The unit direction faced.
    pub fn direction(self) -> Vec2 {
        Vec2::from_angle(self.0)
    }
}

/// What a person carries.
#[derive(Component)]
pub struct Inventory(pub item::Inventory);

impl Default for Inventory {
    fn default() -> Self {
        Self(item::Inventory::new(INVENTORY_SLOTS))
    }
}
