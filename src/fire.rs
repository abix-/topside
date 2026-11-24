//! Fire (Works 15) on modforge's rules (modforge fire.rs): matches in
//! hand and Use set a burnable thing alight; it catches what burns near
//! it and burns away; each smoke detector sounds while a fire is near;
//! an extinguisher in hand and Use, or Use at a hydrant, puts fires out.
//! Everyone alike, through the same Use.

use bevy::prelude::*;
use modforge::actions::Action;

use crate::Ground;
use crate::actor::{Dead, Facing, Hotbar};
use crate::content::{BURNS, EXTINGUISHER, EXTINGUISHER_REACH, FIRE, HOSE_REACH, HYDRANT, MATCHES, SMOKE_DETECTOR};
use crate::input::ThisTick;
use crate::interact::WorldItem;

/// How far, metres, past the reach a person sets alight or sprays.
const REACH: f32 = crate::interact::REACH + 1.0;

/// A thing burning, and for how long (saved).
#[derive(Component, Clone, Copy, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Burning(pub modforge::fire::Fire);

/// A smoke detector sounding: worked out each step, never saved.
#[derive(Component)]
pub struct Sounding;

/// Every step, before things are used: Use with matches in hand sets
/// the burnable thing nearest ahead alight; with an extinguisher, puts
/// out the fires ahead; at a hydrant, those around it. The Use is spent.
#[allow(clippy::type_complexity)]
pub fn light_and_fight(
    mut commands: Commands,
    mut people: Query<(&mut ThisTick, &Transform, &Facing, &Hotbar), Without<Dead>>,
    things: Query<(Entity, &Transform, &WorldItem, Has<Burning>), Without<ThisTick>>,
) {
    for (mut this_tick, at, facing, hotbar) in &mut people {
        if !this_tick.0.iter().any(|s| s.action == Action::Use) {
            continue;
        }
        let feet = at.translation.ground();
        let ahead = feet + facing.direction();
        let held = hotbar.0.held_stack().map(|s| s.item.clone());
        let near = |name: &dyn Fn(&str) -> bool| {
            things
                .iter()
                .filter(|(_, t, w, _)| name(&w.stack.item) && t.translation.ground().distance(feet) <= REACH)
                .min_by(|a, b| a.1.translation.ground().distance(ahead).total_cmp(&b.1.translation.ground().distance(ahead)))
        };
        let fires: Vec<(Entity, Vec2)> = things.iter().filter(|t| t.3).map(|(e, t, ..)| (e, t.translation.ground())).collect();
        let spent = match held.as_deref() {
            Some(MATCHES) => match near(&|n| BURNS.contains(&n)) {
                Some((thing, _, item, false)) => {
                    commands.entity(thing).insert(Burning(Default::default()));
                    info!("set the {} alight", item.stack.item);
                    true
                }
                _ => false,
            },
            Some(EXTINGUISHER) => {
                put_out(&mut commands, &fires, ahead, EXTINGUISHER_REACH);
                true
            }
            _ => match near(&|n| n == HYDRANT) {
                Some((_, hydrant, ..)) => {
                    put_out(&mut commands, &fires, hydrant.translation.ground(), HOSE_REACH);
                    true
                }
                None => false,
            },
        };
        if spent {
            this_tick.0.retain(|s| s.action != Action::Use);
        }
    }
}

/// The fires within `reach` of `at`, out (modforge `fire::put_out`).
fn put_out(commands: &mut Commands, fires: &[(Entity, Vec2)], at: Vec2, reach: f32) {
    let places: Vec<Vec2> = fires.iter().map(|f| f.1).collect();
    for i in modforge::fire::put_out(at, reach, &places) {
        commands.entity(fires[i].0).remove::<Burning>();
    }
}

/// Every step: each fire burns; what burns near one that has burned a
/// while catches; one burned away is gone; each smoke detector sounds
/// while a fire is near it.
#[allow(clippy::type_complexity)]
pub fn burn(
    mut commands: Commands,
    time: Res<Time<Fixed>>,
    mut burning: Query<(Entity, &Transform, &mut Burning, &WorldItem)>,
    things: Query<(Entity, &Transform, &WorldItem, Has<Sounding>), Without<Burning>>,
) {
    let dt = time.delta_secs();
    let mut spreading = Vec::new();
    let mut fires = Vec::new();
    for (thing, at, mut fire, item) in &mut burning {
        let place = at.translation.ground();
        fires.push(place);
        if fire.0.burn(&FIRE, dt) {
            info!("the {} burned away", item.stack.item);
            commands.entity(thing).despawn();
            continue;
        }
        spreading.push((place, fire.0));
    }
    for (thing, at, item, sounding) in &things {
        let place = at.translation.ground();
        if item.stack.item == SMOKE_DETECTOR {
            let sounds = modforge::fire::sounds(&FIRE, place, fires.iter().copied());
            if sounds && !sounding {
                info!("a smoke detector sounds");
                commands.entity(thing).insert(Sounding);
            } else if !sounds && sounding {
                commands.entity(thing).remove::<Sounding>();
            }
            continue;
        }
        if BURNS.contains(&item.stack.item.as_str()) && spreading.iter().any(|(f, fire)| fire.catches(&FIRE, f.distance(place))) {
            commands.entity(thing).insert(Burning(Default::default()));
        }
    }
}
