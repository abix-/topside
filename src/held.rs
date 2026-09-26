//! What a person holds, drawn in their hand (design.md: every item is made
//! in layers): the held item's model with each of its layers' models on
//! it, `<item>` and `<item> <layer>` (scripts/models.py exports them in one
//! frame, the grip at the origin, so they stack). The player's is at the
//! fist in front of their eyes and moves with the fire timer (the 3D
//! game's first person swing); an NPC's is in their rig's right hand.

use bevy::prelude::*;
use modforge::combat::Swing;

use crate::actor::{Dead, Hotbar};
use crate::combat::Firing;
use crate::content::{ItemRegistryRes, LayerRegistryRes};
use crate::model::Model;
use crate::player::{Eyes, Player};

/// The drawn thing in a hand, and which item with which layers it shows.
#[derive(Component, Debug)]
pub struct Held(String);

/// The bone an NPC holds things in (the Quaternius rig's).
const HAND_BONE: &str = "hand_r";
/// Where the fist sits in front of the eyes: lower right, the way every
/// first person game holds things (the 3D game's). The camera looks along
/// its -z.
const HAND: Vec3 = Vec3::new(0.3, -0.35, -0.45);
/// At rest a melee weapon's tip points up, a little forward and a little
/// inward, the way a bat is carried; a gun points forward.
const MELEE_REST: Vec3 = Vec3::new(-0.3, 0.8, -0.5);
const GUN_REST: Vec3 = Vec3::new(0.0, 0.0, -1.0);
/// A melee swing arcs this far down and across (degrees); a gun kicks this
/// far back (metres).
const SWING_DEGREES: f32 = 110.0;
const KICK: f32 = 0.08;

/// The models of the stack in a hotbar's held slot: the item's, then each
/// layer's on it; with the key that tells one held thing from another.
fn held_models(hotbar: &Hotbar, items: &ItemRegistryRes, layers: &LayerRegistryRes) -> Option<(String, Vec<String>)> {
    let stack = hotbar.0.held_stack()?;
    let base = items.0.def(&stack.item)?.picture.clone()?;
    let models = crate::model::stacked(&base, stack, &layers.0);
    Some((models.join("|"), models))
}

/// Every person's hand shows what they hold, swapped when it changes: the
/// player's at the fist before their eyes, an NPC's in their right hand
/// (once their model has loaded).
pub fn hold(
    mut commands: Commands,
    items: Res<ItemRegistryRes>,
    layers: Res<LayerRegistryRes>,
    mut holders: Query<(Entity, &Hotbar, Has<Player>, Option<&mut crate::people::Drawn>), Without<Dead>>,
    eyes: Query<(), With<Eyes>>,
    names: Query<&Name>,
    children: Query<&Children>,
    helds: Query<&Held>,
) {
    for (holder, hotbar, is_player, drawn) in &mut holders {
        let hand = children.iter_descendants(holder).find(|e| {
            if is_player {
                eyes.contains(*e)
            } else {
                names.get(*e).is_ok_and(|n| n.as_str() == HAND_BONE)
            }
        });
        let Some(hand) = hand else {
            continue;
        };
        let wanted = held_models(hotbar, &items, &layers);
        if let Some(mut drawn) = drawn {
            let key = wanted.as_ref().map(|(key, _)| key.clone());
            if drawn.held != key {
                drawn.held = key;
            }
        }
        let shown = children
            .get(hand)
            .into_iter()
            .flat_map(|c| c.iter())
            .find_map(|c| helds.get(c).ok().map(|h| (c, h.0.clone())));
        if shown.as_ref().map(|(_, key)| key) == wanted.as_ref().map(|(key, _)| key) {
            continue;
        }
        if let Some((old, _)) = shown {
            commands.entity(old).despawn();
        }
        let Some((key, models)) = wanted else {
            continue;
        };
        let at = if is_player { Transform::from_translation(HAND) } else { Transform::default() };
        commands
            .spawn((Held(key), ChildOf(hand), at, Visibility::default()))
            .with_children(|c| {
                for model in models {
                    c.spawn(Model(model));
                }
            });
    }
}

/// The player's held thing moves with their fire timer (the 3D game's
/// first person swing): the pose is how far through the weapon's delay
/// they are, no second timer. Melee swings through Gothic's moves; a gun
/// kicks back and returns.
pub fn swing(
    items: Res<ItemRegistryRes>,
    players: Query<(&Hotbar, &Firing), With<Player>>,
    mut helds: Query<&mut Transform, With<Held>>,
    eyes: Query<&Children, With<Eyes>>,
) {
    let (Ok((hotbar, firing)), Ok(children)) = (players.single(), eyes.single()) else {
        return;
    };
    let Some(held) = children.iter().find(|c| helds.contains(*c)) else {
        return;
    };
    let Ok(mut transform) = helds.get_mut(held) else {
        return;
    };
    let is_gun = hotbar
        .0
        .held_stack()
        .and_then(|s| items.0.def(&s.item))
        .and_then(|d| d.combat.as_ref())
        .is_some_and(|c| c.ammo.is_some());
    // 0 at rest, 1 just after firing; the timer counts down over its delay.
    let timer = &firing.0;
    let through = if timer.delay > 0.0 { (timer.ready_in / timer.delay).clamp(0.0, 1.0) } else { 0.0 };
    let arc = (through * std::f32::consts::PI).sin();
    if is_gun {
        transform.translation = HAND + Vec3::Z * arc * KICK;
        transform.rotation = Quat::from_rotation_arc(Vec3::X, GUN_REST.normalize());
        return;
    }
    let rest = Quat::from_rotation_arc(Vec3::X, MELEE_REST.normalize());
    transform.translation = HAND;
    // Progress through the swing, 0 at the press, 1 back at rest.
    let progress = 1.0 - through;
    transform.rotation = match timer.swing {
        Swing::Overhead => Quat::from_rotation_x(-arc * SWING_DEGREES.to_radians()) * rest,
        Swing::Left => side_swing(rest, -1.0, progress),
        Swing::Right => side_swing(rest, 1.0, progress),
        Swing::Block => Quat::from_rotation_z(-1.2 * arc) * rest,
    };
}

/// A level swing from `side` (-1 left, 1 right): the tip winds out on that
/// side, sweeps across the front to the other side, and comes home (the 3D
/// game's).
fn side_swing(rest: Quat, side: f32, progress: f32) -> Quat {
    let toward = |tip: Vec3| Quat::from_rotation_arc(Vec3::X, tip.normalize());
    let wound = toward(Vec3::new(side * 0.9, 0.15, -0.4));
    let through = toward(Vec3::new(-side * 0.9, 0.0, -0.4));
    let smooth = |t: f32| {
        let t = t.clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    if progress < 0.2 {
        rest.slerp(wound, smooth(progress / 0.2))
    } else if progress < 0.6 {
        wound.slerp(through, smooth((progress - 0.2) / 0.4))
    } else {
        through.slerp(rest, smooth((progress - 0.6) / 0.4))
    }
}
