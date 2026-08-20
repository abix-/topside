//! The one interact path: a raycast from the camera finds the
//! nearest interactable, feeds the prompt into the HUD, and E
//! dispatches to modforge's interact function.

use bevy::prelude::*;
use modforge::hud::InteractKind;
use modforge::structure::Aabb;

use crate::player::Player;
use crate::structure::Door;

const INTERACT_RANGE: f32 = 3.0;

#[derive(Component)]
pub struct Interactable {
    pub kind: InteractKind,
    pub aabb: Aabb,
}

/// Nearest interactable the player is looking at this frame.
#[derive(Resource, Default)]
pub struct LookingAt {
    pub entity: Option<Entity>,
    pub kind: Option<InteractKind>,
    pub distance: f32,
}

/// Ray-AABB slab test. Returns the distance along the ray to the
/// nearest face, or None if the ray misses.
fn ray_aabb(origin: Vec3, dir: Vec3, aabb: &Aabb) -> Option<f32> {
    let inv = Vec3::new(1.0 / dir.x, 1.0 / dir.y, 1.0 / dir.z);
    let t1 = (aabb.min - origin) * inv;
    let t2 = (aabb.max - origin) * inv;
    let tmin = t1.min(t2);
    let tmax = t1.max(t2);
    let enter = tmin.x.max(tmin.y).max(tmin.z);
    let exit = tmax.x.min(tmax.y).min(tmax.z);
    if enter <= exit && exit >= 0.0 {
        let t = if enter >= 0.0 { enter } else { exit };
        if t <= INTERACT_RANGE {
            return Some(t);
        }
    }
    None
}

pub fn update_looking_at(
    players: Query<&Transform, With<Player>>,
    interactables: Query<(Entity, &Interactable)>,
    doors: Query<(Entity, &Door)>,
    mut looking_at: ResMut<LookingAt>,
) {
    looking_at.entity = None;
    looking_at.kind = None;
    looking_at.distance = f32::MAX;

    let Ok(player) = players.single() else {
        return;
    };
    let origin = player.translation;
    let dir = player.forward().as_vec3();

    for (entity, door) in &doors {
        if let Some(t) = ray_aabb(origin, dir, &door.collider) {
            if t < looking_at.distance {
                looking_at.entity = Some(entity);
                looking_at.kind = Some(InteractKind::Door { open: door.open });
                looking_at.distance = t;
            }
        }
    }

    for (entity, interactable) in &interactables {
        if let Some(t) = ray_aabb(origin, dir, &interactable.aabb) {
            if t < looking_at.distance {
                looking_at.entity = Some(entity);
                looking_at.kind = Some(interactable.kind);
                looking_at.distance = t;
            }
        }
    }
}

pub fn interact_on_press(
    keys: Res<ButtonInput<KeyCode>>,
    looking_at: Res<LookingAt>,
    mut doors: Query<(&mut Door, &mut Transform)>,
) {
    if !keys.just_pressed(KeyCode::KeyE) {
        return;
    }
    let Some(entity) = looking_at.entity else {
        return;
    };
    let Some(kind) = looking_at.kind else {
        return;
    };
    match kind {
        InteractKind::Door { .. } => {
            if let Ok((mut door, mut transform)) = doors.get_mut(entity) {
                door.open = !door.open;
                *transform = door.transform();
            }
        }
        InteractKind::Pickup | InteractKind::Container => {
            // pickup and container handled when those systems land
        }
    }
}
