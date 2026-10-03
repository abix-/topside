use glam::Vec3;
use slotmap::new_key_type;

use crate::CoordinateSystem;

new_key_type! {
  /// The ID of a character.
  pub struct CharacterId;
}

/// A non-agent character. While agents are "managed" by the archipelago,
/// characters are only as obstacles to be avoided by agents.
#[derive(Debug)]
pub struct Character<CS: CoordinateSystem> {
  /// The current position of the character.
  pub position: CS::Coordinate,
  /// The current velocity of the character.
  pub velocity: CS::Coordinate,
  /// The radius of the character.
  pub radius: f32,
  /// topside patch (todo 9n): how tall the character stands, as an agent's
  /// `height`: an agent leaves out a character whose height differs by half
  /// their two heights or more. `f32::INFINITY`, the default, leaves no
  /// one out.
  pub height: f32,
}

impl<CS: CoordinateSystem> Default for Character<CS> {
  fn default() -> Self {
    Self {
      position: CS::from_landmass(&Vec3::ZERO),
      velocity: CS::from_landmass(&Vec3::ZERO),
      radius: 0.0,
      height: f32::INFINITY,
    }
  }
}
