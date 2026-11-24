//! The type of the storm coming (todo "Storm 3"; topside-town "The
//! storms"): each type told apart by its colour and changing different
//! things, picked by modforge `storm::pick` from content's `STORM_TYPES`,
//! a later storm sometimes two types stacked (todo "Storm effect 10").
//! The type is worked out from the storm's number and the world's seed,
//! both saved, so nothing more is kept.

use modforge::storm::StormType;

use crate::WorldSeed;
use crate::storm::StormRes;

/// The first storm that can stack a second type on its own (0 the
/// first). Claude's, for the operator to change.
pub const STACK_FROM: u32 = 5;

/// The type of the storm coming next: the first is storm 0.
pub fn coming(storms: &StormRes, seed: &WorldSeed) -> Option<StormType> {
    modforge::storm::pick(&crate::content::STORM_TYPES, storms.shifts, seed.0, STACK_FROM)
}

/// The type of the storm that has just passed, once `StormRes::shifts`
/// has counted it; worked out from the same seed before the world's seed
/// changes.
pub fn passed(storms: &StormRes, seed: &WorldSeed) -> Option<StormType> {
    modforge::storm::pick(&crate::content::STORM_TYPES, storms.shifts.saturating_sub(1), seed.0, STACK_FROM)
}

/// A storm type's colour, [red, green, blue, cover], as Bevy's.
pub fn colour(rgba: [f32; 4]) -> bevy::prelude::Color {
    bevy::prelude::Color::srgba(rgba[0], rgba[1], rgba[2], rgba[3])
}
