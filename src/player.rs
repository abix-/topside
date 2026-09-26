//! The player and movement. The player is one picture (a coloured
//! square until pictures exist) at the bunker. Movement reads each
//! actor's drained actions, never keys, so the player and every NPC
//! walk through the same system. WASD moves on the screen: W is up.

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::actor::{Dead, Facing, Home, Hotbar, Inventory, Personality, SpawnActor, Survival, spawn_person};
use crate::content::ActorRegistryRes;
use crate::combat::{Equipment, Firing, Health, Protection, Rolling};
use modforge::combat::ROLL_SPEED_MULT;
use crate::input::{Action, ActionQueue, Actions, ThisTick};
use crate::terrain::WorldRes;

/// Metres per second on foot: three times the 3D game's 5 (design.md
/// "What 2D changes").
pub const WALK_SPEED: f32 = 15.0;
/// The player's square, in metres.
pub const PLAYER_SIZE: f32 = 0.8;
/// Above the ground.
const PLAYER_Z: f32 = 1.0;
const PLAYER_COLOR: Color = Color::srgb(0.9, 0.75, 0.3);

/// The one player. Carries everything an actor needs to be told what
/// to do, and a body that walls stop.
#[derive(Component, Default)]
#[require(
    Actions,
    ThisTick,
    Inventory,
    Hotbar,
    Survival,
    Facing,
    Health,
    Protection,
    Equipment,
    Firing,
    Rolling,
    Transform,
    RigidBody = RigidBody::Kinematic,
    Collider = Collider::circle(PLAYER_SIZE / 2.0)
)]
pub struct Player;

/// Where the player wakes: the bunker, at the start and after every
/// death.
pub fn wake_point(world: &WorldRes) -> Vec3 {
    world.bunker.extend(PLAYER_Z)
}

/// The one place the player enters the world: at the bunker, a person
/// through the same `spawn_person` as every NPC, under human control.
pub fn spawn_player(mut spawn: SpawnActor, registry: Res<ActorRegistryRes>, world: Res<WorldRes>) {
    let def = registry.0.def("player").expect("the player is registered");
    let at = wake_point(&world);
    let person = spawn_person(&mut spawn, def, at.truncate(), def.faction.as_deref(), PLAYER_COLOR, NOSE_COLOR)
        .expect("the actor map has room for the player");
    spawn
        .commands
        .entity(person)
        .insert((Player, Transform::from_translation(at)));
}

const NOSE_COLOR: Color = Color::srgb(0.25, 0.2, 0.1);

/// Every actor's roll this tick (combat.md "The roll"): Roll starts one
/// along the move held, or where the person faces with no move held,
/// unless mid swing; the direction is then locked until it ends.
pub fn roll(
    time: Res<Time<Fixed>>,
    mut actors: Query<(&ThisTick, &Facing, Option<&Firing>, &mut Rolling), Without<Dead>>,
) {
    for (this_tick, facing, firing, mut rolling) in &mut actors {
        rolling.0.tick(time.delta_secs());
        if firing.is_some_and(|f| f.0.committed()) || !ActionQueue::has(&this_tick.0, Action::Roll) {
            continue;
        }
        let (x, y) = ActionQueue::movement(&this_tick.0);
        let way = Vec2::new(x, y);
        rolling.0.start(if way == Vec2::ZERO { facing.direction() } else { way });
    }
}

/// Every actor faces the point it aims at this tick, and its picture
/// turns with it.
pub fn aim(
    mut actors: Query<(&ThisTick, Option<&Firing>, Option<&Rolling>, &mut Facing, &mut Transform), Without<Dead>>,
) {
    for (this_tick, firing, rolling, mut facing, mut transform) in &mut actors {
        // Committed to a swing or a roll: it goes where it was aimed.
        if firing.is_some_and(|f| f.0.committed()) || Rolling::busy(rolling) {
            continue;
        }
        let Some((x, y)) = ActionQueue::aim(&this_tick.0) else {
            continue;
        };
        let to = Vec2::new(x, y) - transform.translation.truncate();
        if to != Vec2::ZERO {
            facing.0 = to.to_angle();
            transform.rotation = Quat::from_rotation_z(facing.0);
        }
    }
}

/// Every actor walks the way its move this tick says, at walking
/// speed, and slides along whatever stops it (Quake III's
/// PM_SlideMove, as shipped by Avian). Diagonal is no faster than
/// straight.
pub fn movement(
    time: Res<Time<Fixed>>,
    move_and_slide: MoveAndSlide,
    mut actors: Query<
        (
            Entity,
            &ThisTick,
            &Collider,
            Option<&Personality>,
            Option<&Firing>,
            Option<&Rolling>,
            &mut Transform,
        ),
        Without<Dead>,
    >,
) {
    let config = MoveAndSlideConfig::default();
    for (entity, this_tick, shape, personality, firing, rolling, mut transform) in &mut actors {
        // Committed to a swing: no walking until it ends.
        if firing.is_some_and(|f| f.0.committed()) {
            continue;
        }
        // Swift people walk faster, slow ones slower.
        let speed = WALK_SPEED * personality.map_or(1.0, |p| p.0.speed_mult());
        // Mid roll: faster, along the direction it started with.
        let velocity = match rolling.filter(|r| r.0.rolling()) {
            Some(r) => r.0.direction * speed * ROLL_SPEED_MULT,
            None => {
                let (x, y) = ActionQueue::movement(&this_tick.0);
                Vec2::new(x, y).normalize_or_zero() * speed
            }
        };
        if velocity == Vec2::ZERO {
            continue;
        }
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        let to = move_and_slide
            .move_and_slide(
                shape,
                transform.translation.truncate(),
                0.0,
                velocity,
                time.delta(),
                &config,
                &filter,
                |_| MoveAndSlideHitResponse::Accept,
            )
            .position;
        transform.translation = to.extend(transform.translation.z);
    }
}

/// A dead person's Respawn this tick (the player's Enter, a bunker
/// person's brain): back in the world at home (the player wakes in
/// their bunker), carrying nothing (the death box keeps what they had).
pub fn respawn(
    mut commands: Commands,
    world: Res<WorldRes>,
    mut dead: Query<(Entity, &ThisTick, &mut Transform, Option<&Home>), With<Dead>>,
) {
    for (entity, this_tick, mut transform, home) in &mut dead {
        if !this_tick.0.iter().any(|s| matches!(s.action, Action::Respawn)) {
            continue;
        }
        transform.translation = match home {
            Some(home) => home.0.extend(transform.translation.z),
            None => wake_point(&world),
        };
        commands
            .entity(entity)
            .remove::<(Dead, ColliderDisabled)>()
            .insert(Visibility::Inherited);
    }
}

/// The camera stays on the player.
pub fn camera_follow(
    player: Query<&Transform, (With<Player>, Without<Camera2d>)>,
    mut cameras: Query<&mut Transform, With<Camera2d>>,
) {
    let Ok(player) = player.single() else {
        return;
    };
    for mut camera in &mut cameras {
        camera.translation.x = player.translation.x;
        camera.translation.y = player.translation.y;
    }
}
