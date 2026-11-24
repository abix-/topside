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
    (items, fires): (Res<crate::content::ItemRegistryRes>, Query<&Powered, With<Container>>),
) {
    // The generators, in ThingId order: where each runs and its supply.
    let mut running: Vec<(u64, Vec3, f32)> = Vec::new();
    for (entity, item, at, mut fuel, generator, id) in &mut generators {
        // A fire (a fireplace, a wood stove) burns any wood put in it, on
        // the generator's burning (modforge `Burning::run`), and is lit
        // while it does (topside-town "Everything works").
        if crate::content::FIRES.contains(&item.stack.item.as_str()) {
            let Some(mut burning) = generator else {
                commands.entity(entity).insert(Generator::default());
                continue;
            };
            let wood = items.0.count_in(&fuel.0, crate::content::WOOD);
            let (used, lit) = burning.0.run(&crate::content::FIRE_DEF, wood, time.delta_secs());
            for _ in 0..used {
                let kind = fuel.0.slots.iter().flatten().find(|s| items.0.answers(&s.item, crate::content::WOOD)).map(|s| s.item.clone());
                if let Some(kind) = kind {
                    fuel.0.take_one(&kind);
                }
            }
            if fires.get(entity).map(|p| p.0).ok() != Some(lit) {
                commands.entity(entity).insert(Powered(lit));
            }
            continue;
        }
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

/// What a switch or a light is wired to: a light to its switch, a switch
/// to its house's breaker box (saved).
#[derive(Component, Clone, Copy, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct WiredTo(#[entities] pub Entity);

/// The town's power (topside-town "Everything works"; Project Zomboid's
/// house power): the substation, and in every house a breaker box and each
/// room's switch with its light, each through the one way a thing comes
/// into the world. Once a world, as its people; after a load they are in
/// the save.
pub fn populate_town_power(
    mut commands: Commands,
    world: Res<crate::terrain::WorldRes>,
    items: Res<crate::content::ItemRegistryRes>,
    mut ids: ResMut<crate::interact::ThingIssuer>,
    mut things_made: ResMut<crate::terrain::ThingsMade>,
) {
    if things_made.power {
        return;
    }
    things_made.power = true;
    let Some(town) = world.world.town.as_ref() else {
        return;
    };
    let mut make = |commands: &mut Commands, name: &str, at: Vec2, kind: Option<InteractKind>| -> Option<Entity> {
        let def = items.0.def(name)?;
        let (made, _) = crate::interact::spawn_item(commands, &mut ids, def, modforge::item::create(def, 1, &[], 0.0, 0), crate::on_ground(at, 0.0));
        match kind {
            Some(kind) => commands.entity(made).insert(Interactable { kind }),
            // Not picked up or used: it shines or it does not.
            None => commands.entity(made).remove::<Interactable>(),
        };
        Some(made)
    };
    make(&mut commands, crate::content::SUBSTATION, town.substation(), None);
    for ((breaker_at, rooms), heating) in town.wiring().into_iter().zip(town.heating()) {
        let Some(breaker) = make(&mut commands, crate::content::BREAKER_BOX, breaker_at, Some(InteractKind::Switch { on: true })) else {
            continue;
        };
        // Its heating: the fires are boxes wood is put in; the furnace and
        // the air conditioner run off the thermostat, as a light off its
        // switch.
        let thermostat_at = heating.iter().find(|(kind, _)| *kind == crate::content::THERMOSTAT).map(|(_, at)| *at);
        let thermostat = thermostat_at.and_then(|at| make(&mut commands, crate::content::THERMOSTAT, at, Some(InteractKind::Switch { on: false })));
        if let Some(thermostat) = thermostat {
            commands.entity(thermostat).insert(WiredTo(breaker));
        }
        for (kind, at) in heating {
            if crate::content::FIRES.contains(&kind) {
                make(&mut commands, kind, at, Some(InteractKind::Container));
            } else if kind != crate::content::THERMOSTAT
                && let (Some(made), Some(thermostat)) = (make(&mut commands, kind, at, None), thermostat)
            {
                commands.entity(made).insert(WiredTo(thermostat));
            }
        }
        for (switch_at, light_at) in rooms {
            let switch = make(&mut commands, crate::content::LIGHT_SWITCH, switch_at, Some(InteractKind::Switch { on: false }));
            let light = make(&mut commands, crate::content::CEILING_LIGHT, light_at, None);
            if let (Some(switch), Some(light)) = (switch, light) {
                commands.entity(switch).insert(WiredTo(breaker));
                commands.entity(light).insert(WiredTo(switch));
            }
        }
    }
}

/// Every step: each light (and each furnace and air conditioner, off its
/// thermostat) is lit when modforge says (`power::lit`): its
/// switch on, its breaker on, and the breaker on a grid with power, the
/// substation's lines or a running generator's wiring.
#[allow(clippy::type_complexity)]
pub fn lights(
    mut commands: Commands,
    lights: Query<(Entity, &WorldItem, &WiredTo, Option<&Powered>), Without<Interactable>>,
    switches: Query<(&Interactable, &Transform, Option<&WiredTo>)>,
    grid: Query<(&WorldItem, &Transform, Option<&Container>)>,
) {
    let on = |kind: &InteractKind| matches!(kind, InteractKind::Switch { on: true });
    // Where power is: the substation (its lines reach the town) and each
    // running generator (its wiring's reach).
    let sources: Vec<(Vec3, f32)> = grid
        .iter()
        .filter_map(|(item, at, fuel)| match item.stack.item.as_str() {
            crate::content::SUBSTATION => Some((at.translation, crate::content::LINES_REACH)),
            // A generator runs while it has fuel (modforge `Burning::run`).
            GENERATOR if fuel.is_some_and(|f| f.0.count_of(FUEL) > 0) => Some((at.translation, WIRE_REACH)),
            _ => None,
        })
        .collect();
    for (light, item, to, was) in &lights {
        let Ok((switch, _, Some(breaker))) = switches.get(to.0) else {
            continue;
        };
        let Ok((breaker_kind, breaker_at, _)) = switches.get(breaker.0) else {
            continue;
        };
        let has_grid = sources.iter().any(|(at, reach)| at.distance(breaker_at.translation) <= *reach);
        let lit = modforge::power::lit(has_grid, on(&breaker_kind.kind), on(&switch.kind));
        if was.map(|p| p.0) != Some(lit) {
            commands.entity(light).insert(Powered(lit));
            // A furnace or an air conditioner runs; only a light shines.
            if item.stack.item != crate::content::CEILING_LIGHT {
                continue;
            }
            if lit {
                commands.entity(light).insert(PointLight {
                    intensity: crate::content::CEILING_LIGHT_LUMENS,
                    ..default()
                });
            } else {
                commands.entity(light).remove::<PointLight>();
            }
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
