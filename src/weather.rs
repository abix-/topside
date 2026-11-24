//! Weather (todo 21) on modforge's rules (modforge weather.rs): each game
//! hour each biome rolls its weather from its own weather list (its
//! `BiomeDef`, biome.rs). Where a person stands, it changes the air and the
//! radiation they are exposed to in the open (exposure.rs), the same for
//! everyone; the player sees it as fog closing in, coloured by the
//! weather, and named on the vitals line. The colours are Claude's.

use bevy::prelude::*;
use modforge::weather::Weather;

use crate::Ground;

/// The weather at a point on the ground now: an event's while it lasts
/// (src/event.rs, everywhere), else from its biome's weather list (todo
/// 32).
pub fn at(world: &crate::terrain::WorldRes, biomes: &crate::content::BiomeRegistryRes, events: &crate::event::EventsRes, tick: u64, day: crate::survival::Day, point: Vec2) -> Weather {
    if let Some(weather) = events.weather(crate::survival::Day::secs(&crate::SimTick(tick))) {
        return weather;
    }
    let biome = crate::biome::at(world, biomes, point);
    let hour = ((day.0 / 24.0) * 60.0).round().max(1.0) as u64;
    let name = biome.map_or("", |b| b.name.as_str());
    modforge::weather::roll(&crate::biome::weathers(biome), world.seed, tick / hour, name)
}

fn colour(weather: Weather) -> Color {
    match weather {
        Weather::Rain => Color::srgb(0.45, 0.48, 0.52),
        Weather::Fog => Color::srgb(0.7, 0.72, 0.74),
        Weather::RadiationStorm => Color::srgb(0.45, 0.6, 0.3),
        Weather::HeatWave => Color::srgb(0.85, 0.75, 0.55),
        Weather::ColdSnap => Color::srgb(0.85, 0.9, 0.95),
        Weather::Clear => Color::srgb(0.7, 0.75, 0.8),
    }
}

/// Every frame: the player's eyes see the weather where they stand: fog
/// at its distance, or none.
pub fn sky(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    day: Res<crate::survival::Day>,
    world: Option<Res<crate::terrain::WorldRes>>,
    biomes: Res<crate::content::BiomeRegistryRes>,
    events: Res<crate::event::EventsRes>,
    player: Query<&Transform, With<crate::player::Player>>,
    eyes: Query<(Entity, Option<&DistanceFog>), With<crate::player::Eyes>>,
) {
    let (Some(world), Ok(at)) = (world, player.single()) else {
        return;
    };
    let weather = self::at(&world, &biomes, &events, tick.0, *day, at.translation.ground());
    // A heat wave or a cold snap is a haze far off; clear is no fog.
    let far = match weather {
        Weather::Clear => None,
        Weather::HeatWave | Weather::ColdSnap => Some(HAZE),
        _ => weather.sight(),
    };
    for (entity, fog) in &eyes {
        match far {
            Some(far) => {
                let want = DistanceFog { color: colour(weather), falloff: FogFalloff::Linear { start: far * 0.2, end: far }, ..default() };
                if fog.is_none_or(|f| f.color != want.color) {
                    commands.entity(entity).insert(want);
                }
            }
            None if fog.is_some() => {
                commands.entity(entity).remove::<DistanceFog>();
            }
            None => {}
        }
    }
}

/// How far off a heat wave's or a cold snap's haze closes, metres.
const HAZE: f32 = 300.0;
