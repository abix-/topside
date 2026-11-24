//! Gardens (Works 18) on modforge's rules (modforge grow.rs): Use at an
//! empty garden bed with a seed carried sows it; at a ripe one, harvests
//! its food and seed back into the bags. Everyone alike, through the
//! same Use.

use bevy::prelude::*;
use modforge::actions::Action;
use modforge::memory::Did;

use crate::Ground;
use crate::actor::{Dead, Facing, Hotbar, Inventory, Persona};
use crate::content::{CROPS, GARDEN_BED};
use crate::input::ThisTick;
use crate::interact::WorldItem;

/// How far, metres, a person reaches a bed.
const REACH: f32 = crate::interact::REACH + 1.0;

/// A garden bed's crop and the day it was sown (saved).
#[derive(Component, Clone, Copy, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Growing(pub modforge::grow::Sown);

/// Every step, before things are used: Use at the garden bed nearest
/// ahead sows, or harvests a ripe crop. The Use is spent.
#[allow(clippy::type_complexity)]
pub fn tend(
    mut commands: Commands,
    (tick, day): (Res<crate::SimTick>, Res<crate::survival::Day>),
    items: Res<crate::content::ItemRegistryRes>,
    mut people: Query<(&mut ThisTick, &Transform, &Facing, &mut Inventory, &mut Hotbar, &mut Persona), Without<Dead>>,
    beds: Query<(Entity, &Transform, &WorldItem, Option<&Growing>), Without<ThisTick>>,
) {
    let today = crate::survival::today(&tick, *day);
    for (mut this_tick, at, facing, mut inventory, mut hotbar, mut mind) in &mut people {
        if !this_tick.0.iter().any(|s| s.action == Action::Use) {
            continue;
        }
        let feet = at.translation.ground();
        let ahead = feet + facing.direction();
        let bed = beds
            .iter()
            .filter(|(_, t, w, _)| w.stack.item == GARDEN_BED && t.translation.ground().distance(feet) <= REACH)
            .min_by(|a, b| a.1.translation.ground().distance(ahead).total_cmp(&b.1.translation.ground().distance(ahead)));
        let Some((bed, _, _, growing)) = bed else {
            continue;
        };
        this_tick.0.retain(|s| s.action != Action::Use);
        let did = match growing {
            Some(Growing(sown)) => {
                let crop = &CROPS[sown.crop.min(CROPS.len() - 1)];
                if modforge::grow::ripe(crop, sown.day, today) {
                    for (thing, count) in [(crop.food, crop.gives), (crop.seed, crop.seeds_back)] {
                        if let Some(def) = items.0.def(thing) {
                            let _ = inventory.0.add(modforge::item::create(def, count, &[], today, 0), def.max_stack);
                        }
                    }
                    commands.entity(bed).remove::<Growing>();
                    Did::Done(format!("harvested {} {}s", crop.gives, crop.food))
                } else {
                    Did::Failed(format!("harvest the {}s", crop.food), "they are not ripe yet".to_string())
                }
            }
            None => {
                // The first seed carried, in the bags or on the hotbar.
                let seed = CROPS.iter().position(|c| inventory.0.count_of(c.seed) > 0 || hotbar.0.slots.count_of(c.seed) > 0);
                match seed {
                    Some(crop) => {
                        let name = CROPS[crop].seed;
                        if !inventory.0.take_one(name) {
                            hotbar.0.slots.take_one(name);
                        }
                        commands.entity(bed).insert(Growing(modforge::grow::Sown { crop, day: today }));
                        Did::Done(format!("sowed {name}"))
                    }
                    None => Did::Failed("sow the garden bed".to_string(), "no seed carried".to_string()),
                }
            }
        };
        mind.0.memory.did(did, tick.0);
    }
}
