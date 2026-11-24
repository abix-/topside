//! The type of the storm coming (todo "Storm 3"; topside-town "The
//! storms"): each type told apart by its colour and changing different
//! things, picked by modforge `storm::pick_type` from content's
//! `STORM_TYPES`. The type is worked out from the storm's number and the
//! world's seed, both saved, so nothing more is kept.

use modforge::storm::StormType;

use crate::WorldSeed;
use crate::storm::StormRes;

/// The type of the storm coming next: the first is storm 0.
pub fn coming(storms: &StormRes, seed: &WorldSeed) -> Option<&'static StormType> {
    modforge::storm::pick_type(&crate::content::STORM_TYPES, storms.shifts, seed.0)
}

/// A storm type's colour, [red, green, blue, cover], as Bevy's.
pub fn colour(rgba: [f32; 4]) -> bevy::prelude::Color {
    bevy::prelude::Color::srgba(rgba[0], rgba[1], rgba[2], rgba[3])
}
