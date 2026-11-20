//! Power (todo 29) on modforge's rules (modforge power.rs): a generator
//! is a box (a WorldItem with a `Container`) that burns the fuel put in
//! it; every consumer within its wiring's reach is on its grid, and the
//! grid's supply goes to them in ThingId order. A powered research
//! station researches; one without power does not (research.rs). The
//! reach, outputs and draws are content.

use bevy::prelude::*;
use modforge::power::Burning;

use crate::content::{FUEL, GENERATOR, GENERATOR_DEF, RESEARCH_DRAW, WIRE_REACH};
use crate::interact::{Container, Interactable, ThingId, WorldItem};
use modforge::crafting::StationKind;
use modforge::hud::InteractKind;

/// A generator's burning (saved).
#[derive(Component, Default, Clone, Copy, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Generator(pub Burning);

/// Whether a consumer has power now: worked out every step from the
/// generators, never saved.
#[derive(Component, Default, Clone, Copy)]
pub struct Powered(pub bool);

/// What a station draws, if it needs power.
fn draw(kind: &InteractKind) -> Option<f32> {
    match kind {
        InteractKind::Station(StationKind::ResearchStation) => Some(RESEARCH_DRAW),
        _ => None,
    }
}

/// Every step: each generator burns its fuel; each consumer is powered
/// if a running generator's wiring reaches it and its supply is left.
#[allow(clippy::type_complexity)]
pub fn run(
    mut commands: Commands,
    time: Res<Time<Fixed>>,
    mut generators: Query<(Entity, &WorldItem, &Transform, &mut Container, Option<&mut Generator>, &ThingId)>,
    consumers: Query<(Entity, &Interactable, &Transform, &ThingId, Option<&Powered>)>,
) {
    // The generators, in ThingId order: where each runs and its supply.
    let mut running: Vec<(u64, Vec3, f32)> = Vec::new();
    for (entity, item, at, mut fuel, generator, id) in &mut generators {
        if item.stack.item != GENERATOR {
            continue;
        }
        let Some(mut generator) = generator else {
            commands.entity(entity).insert(Generator::default());
            continue;
        };
        let (used, on) = generator.0.run(&GENERATOR_DEF, fuel.0.count_of(FUEL), time.delta_secs());
        for _ in 0..used {
            fuel.0.take_one(FUEL);
        }
        if on {
            running.push((id.0, at.translation, GENERATOR_DEF.output));
        }
    }
    running.sort_by_key(|(id, ..)| *id);
    // The consumers, in ThingId order, each on the first grid reaching it
    // with its draw left.
    let mut wanting: Vec<(u64, Entity, Vec3, f32, bool)> = consumers
        .iter()
        .filter_map(|(entity, kind, at, id, was)| draw(&kind.kind).map(|d| (id.0, entity, at.translation, d, was.is_some_and(|p| p.0))))
        .collect();
    wanting.sort_by_key(|(id, ..)| *id);
    let mut left: Vec<f32> = running.iter().map(|(.., supply)| *supply).collect();
    for (_, entity, at, need, was) in wanting {
        let reaches: Vec<f32> = running.iter().zip(&left).map(|((_, g, _), l)| if g.distance(at) <= WIRE_REACH { *l } else { 0.0 }).collect();
        // modforge's share: powered only if the whole draw is left.
        let grid = reaches.iter().position(|l| modforge::power::share(*l, &[need])[0]);
        if let Some(grid) = grid {
            left[grid] -= need;
        }
        let on = grid.is_some();
        if on != was {
            commands.entity(entity).insert(Powered(on));
        }
    }
}

/// Whether the research station nearest a point has power.
pub fn station_powered(at: Vec3, kind: StationKind, stations: &Query<(&Interactable, &Transform, Option<&Powered>)>) -> bool {
    stations
        .iter()
        .filter(|(i, ..)| i.kind == InteractKind::Station(kind))
        .min_by(|a, b| a.1.translation.distance(at).total_cmp(&b.1.translation.distance(at)))
        .is_some_and(|(.., p)| p.is_some_and(|p| p.0))
}
