//! The player and movement. The player is a person like any NPC, seen
//! through their own eyes (authority.md "Presentation": first person).
//! Movement reads each actor's drained actions, never keys, so the player
//! and every NPC walk through the same system; `Move` is a direction on
//! the ground for everyone (input.rs turns WASD into one).

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::actor::{Dead, Facing, Home, Hotbar, Inventory, Personality, SpawnActor, Survival, spawn_person};
use crate::combat::{Equipment, Firing, Health, Protection, Rolling};
use crate::content::ActorRegistryRes;
use crate::input::{Action, ActionQueue, Actions, ThisTick};
use crate::terrain::WorldRes;
use crate::{Ground, on_ground, turned};
use modforge::combat::ROLL_SPEED_MULT;

/// Metres per second on foot: three times the 3D game's 5 (design.md
/// "What 2D changes").
pub const WALK_SPEED: f32 = 15.0;
/// A person's body across, in metres: what stops them.
pub const PLAYER_SIZE: f32 = 0.8;
/// A person's height, feet to crown, in metres.
pub const BODY_HEIGHT: f32 = 1.8;
/// A person stands with the middle of their body this high: where their
/// `Transform` is, the middle of the body that walls stop.
pub const BODY_CENTRE: f32 = BODY_HEIGHT / 2.0;
/// How far up and down the player can look, in radians.
const PITCH_LIMIT: f32 = 1.54;

/// A person's body: upright, `PLAYER_SIZE` across and `BODY_HEIGHT` tall.
pub fn body() -> Collider {
    Collider::capsule(PLAYER_SIZE / 2.0, BODY_HEIGHT - PLAYER_SIZE)
}

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
    Collider = body()
)]
pub struct Player;

/// The player's eyes (the first person camera): how far they look up or
/// down. Where they face on the ground is their `Facing`.
#[derive(Component, Default)]
pub struct Eyes {
    pub pitch: f32,
}

/// Where the player wakes: the bunker, at the start and after every
/// death.
pub fn wake_point(world: &WorldRes) -> Vec3 {
    on_ground(world.bunker, BODY_CENTRE)
}

/// The one place the player enters the world: at the bunker, a person
/// through the same `spawn_person` as every NPC, under human control.
pub fn spawn_player(mut spawn: SpawnActor, registry: Res<ActorRegistryRes>, world: Res<WorldRes>) {
    let def = registry.0.def("player").expect("the player is registered");
    let at = wake_point(&world);
    let person = spawn_person(&mut spawn, def, at.ground(), def.faction.as_deref())
        .expect("the actor map has room for the player");
    spawn
        .commands
        .entity(person)
        .insert((Player, Transform::from_translation(at)));
}

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

/// Every actor turns by this tick's look (the player's mouse): the yaw
/// turns where they face on the ground, the pitch tilts their eyes.
pub fn look(
    mut actors: Query<(&ThisTick, &mut Facing, &mut Transform, Option<&Children>), Without<Dead>>,
    mut eyes: Query<(&mut Eyes, &mut Transform), Without<Facing>>,
) {
    for (this_tick, mut facing, mut transform, children) in &mut actors {
        let (yaw, pitch) = ActionQueue::look(&this_tick.0);
        if yaw == 0.0 && pitch == 0.0 {
            continue;
        }
        facing.0 += yaw;
        transform.rotation = turned(facing.0);
        for child in children.into_iter().flat_map(|c| c.iter()) {
            if let Ok((mut eye, mut at)) = eyes.get_mut(child) {
                eye.pitch = (eye.pitch + pitch).clamp(-PITCH_LIMIT, PITCH_LIMIT);
                at.rotation = Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2) * Quat::from_rotation_x(eye.pitch);
            }
        }
    }
}

/// Every actor faces the ground point it aims at this tick (an NPC's
/// brain), and its body turns with it.
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
        let to = Vec2::new(x, y) - transform.translation.ground();
        if to != Vec2::ZERO {
            facing.0 = to.to_angle();
            transform.rotation = turned(facing.0);
        }
    }
}

/// Every actor walks the way its move this tick says, at walking
/// speed, along the ground, and slides along whatever stops it (Quake
/// III's PM_SlideMove, as shipped by Avian). Diagonal is no faster than
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
                transform.translation,
                Quat::IDENTITY,
                on_ground(velocity, 0.0),
                time.delta(),
                &config,
                &filter,
                |_| MoveAndSlideHitResponse::Accept,
            )
            .position;
        // Along the ground only: the body keeps its height.
        transform.translation = on_ground(to.ground(), transform.translation.y);
    }
}

/// A dead person's Respawn this tick (the player's Enter, a bunker
/// person's brain): back in the world at home (the player wakes in
/// their bunker), carrying nothing (the death box keeps what they had).
pub fn respawn(
    mut commands: Commands,
    world: Res<WorldRes>,
    tick: Res<crate::SimTick>,
    mut dead: Query<
        (Entity, &ThisTick, &mut Transform, Option<&Home>, Option<&mut crate::actor::Memory>, &mut Health, &mut Survival),
        With<Dead>,
    >,
) {
    for (entity, this_tick, mut transform, home, memory, mut health, mut needs) in &mut dead {
        if !this_tick.0.iter().any(|s| matches!(s.action, Action::Respawn)) {
            continue;
        }
        // What they come back with: modforge's rule.
        modforge::survival::respawn(&mut health.0, &mut needs.0);
        if let Some(mut memory) = memory {
            memory.0.did(modforge::memory::Did::Respawned, tick.0);
        }
        transform.translation = match home {
            Some(home) => on_ground(home.0, BODY_CENTRE),
            None => wake_point(&world),
        };
        commands
            .entity(entity)
            .remove::<(Dead, ColliderDisabled)>()
            .insert(Visibility::Inherited);
    }
}
