//! The player and movement. The player is a person like any NPC, seen
//! through their own eyes (authority.md "Presentation": first person).
//! Movement reads each actor's drained actions, never keys, so the player
//! and every NPC walk through the same system; `Move` is a direction on
//! the ground for everyone (input.rs turns WASD into one).

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::actor::{Dead, Facing, Home, Hotbar, Inventory, Persona, SpawnActor, spawn_person};
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

/// What every person's body and every item's carries so Avian makes no
/// contact pairs for it (todo 9p): a member of every layer, so every
/// spatial query (move and slide, sight, aim, a hit) still finds it, and
/// filtering none, so the broad phase pairs it with nothing. Nothing reads
/// contacts; kinematic and static bodies are never solved (Rapier's
/// ActiveCollisionTypes leaves these contacts out the same way).
pub fn makes_no_contacts() -> CollisionLayers {
    CollisionLayers::new(LayerMask::ALL, LayerMask::NONE)
}

/// The one player. Carries everything an actor needs to be told what
/// to do, and a body that walls stop.
#[derive(Component, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
#[require(
    Actions,
    ThisTick,
    Inventory,
    Hotbar,
    Facing,
    Health,
    Protection,
    Equipment,
    Firing,
    Rolling,
    Fall,
    Transform,
    // Worked out each frame from where they look: never saved.
    crate::talk::LookingAt,
    RigidBody = RigidBody::Kinematic,
    Collider = body(),
    CollisionLayers = makes_no_contacts()
)]
pub struct Player;

/// The player's eyes (the first person camera): how far they look up or
/// down. Where they face on the ground is their `Facing`.
#[derive(Component, Default)]
pub struct Eyes {
    pub pitch: f32,
}

/// The eyes' turn under the player for a pitch: the one place it is
/// worked out.
pub fn eye_rotation(pitch: f32) -> Quat {
    Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2) * Quat::from_rotation_x(pitch)
}

/// Where the player wakes: their room on the bunker's deepest level
/// (design.md "The bunker you start in"), at the start and after every
/// death.
pub fn wake_point(world: &WorldRes) -> Vec3 {
    crate::structure::on_floor(world.bunker, crate::content::PLAYER_ROOM_FLOOR) + Vec3::Y * BODY_CENTRE
}

/// The one place the player enters the world: at the bunker, a person
/// through the same `spawn_person` as every NPC, under human control.
pub fn spawn_player(mut spawn: SpawnActor, registry: Res<ActorRegistryRes>, world: Res<WorldRes>) {
    let def = registry.0.def("player").expect("the player is registered");
    let at = wake_point(&world);
    let person = spawn_person(&mut spawn, def, at - Vec3::Y * BODY_CENTRE, def.faction.as_deref())
        .expect("the actor map has room for the player");
    spawn
        .commands
        .entity(person)
        .insert((Player, crate::talk::LookingAt::default(), crate::hud::HintsDone::default()));
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
                at.rotation = eye_rotation(eye.pitch);
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

/// How far up a body steps without climbing (a stair's step is 0.31 m).
pub const CLIMB: f32 = 0.35;
/// Metres per second, per second, down.
const GRAVITY: f32 = 9.81;

/// How fast a body is falling, metres per second down; zero standing on
/// something (the 3D game's `Fall`).
#[derive(Component, Default, Debug, Clone, Copy, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Fall(pub f32);

/// Every actor walks the way its move this tick says, at walking speed,
/// and slides along whatever stops it (Quake III's PM_SlideMove, as
/// shipped by Avian); a move blocked flat is tried again raised by
/// `CLIMB`, so steps are climbed (Quake's step slide move); then the body
/// settles on whatever is under its feet, or falls. The ground itself is
/// the lowest a body goes, but inside a building that goes below it, its
/// lowest floor (`structure::lowest_at`). Diagonal is no faster than
/// straight. The 3D game's movement, for everyone.
pub fn movement(
    time: Res<Time<Fixed>>,
    move_and_slide: MoveAndSlide,
    below: Query<&crate::structure::Below>,
    mut actors: Query<
        (
            Entity,
            &ThisTick,
            &Collider,
            Option<&Persona>,
            Option<&Firing>,
            Option<&Rolling>,
            &mut Transform,
            &mut Fall,
            Has<crate::vehicle::Vehicle>,
        ),
        Without<Dead>,
    >,
) {
    let config = MoveAndSlideConfig::default();
    let dt = time.delta_secs();
    for (entity, this_tick, shape, personality, firing, rolling, mut transform, mut fall, vehicle) in &mut actors {
        // Swift people walk faster, slow ones slower; a vehicle goes at
        // its own speed (todo 57).
        let kind = if vehicle { crate::content::CAR_DEF.speed } else { 1.0 };
        let speed = WALK_SPEED * personality.map_or(1.0, |p| p.0.personality.speed_mult()) * kind;
        // Mid roll: faster, along the direction it started with. Committed
        // to a swing: no walking until it ends.
        let velocity = if firing.is_some_and(|f| f.0.committed()) {
            Vec2::ZERO
        } else {
            match rolling.filter(|r| r.0.rolling()) {
                Some(r) => r.0.direction * speed * ROLL_SPEED_MULT,
                None => {
                    let (x, y) = ActionQueue::movement(&this_tick.0);
                    Vec2::new(x, y).normalize_or_zero() * speed
                }
            }
        };
        // Standing still on something costs nothing.
        if velocity == Vec2::ZERO && fall.0 == 0.0 {
            continue;
        }
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        let start = transform.translation;
        let slide = |from: Vec3| {
            move_and_slide
                .move_and_slide(
                    shape,
                    from,
                    Quat::IDENTITY,
                    on_ground(velocity, 0.0),
                    time.delta(),
                    &config,
                    &filter,
                    |_| MoveAndSlideHitResponse::Accept,
                )
                .position
        };
        let mut centre = if velocity == Vec2::ZERO {
            start
        } else {
            let flat = slide(start);
            let raised = slide(start + Vec3::Y * CLIMB);
            let progress = |p: Vec3| (p - start).with_y(0.0).length();
            if progress(raised) > progress(flat) + 1e-4 { raised } else { flat }
        };
        // Fall this tick, then land on the first thing under the feet: on
        // the ground the cast reaches a step down, in the air only as far
        // as the fall.
        let airborne = fall.0 > 0.0;
        fall.0 += GRAVITY * dt;
        let reach = if airborne { fall.0 * dt } else { CLIMB + (centre.y - start.y).max(0.0) + fall.0 * dt };
        match move_and_slide.cast_move(shape, centre, Quat::IDENTITY, Vec3::NEG_Y * reach, 0.0, &filter) {
            Some(hit) => {
                centre.y -= hit.distance;
                fall.0 = 0.0;
            }
            None => centre.y -= fall.0 * dt,
        }
        let lowest = crate::structure::lowest_at(&below, centre.ground()) + BODY_CENTRE;
        if centre.y <= lowest {
            centre.y = lowest;
            fall.0 = 0.0;
        }
        transform.translation = centre;
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
        (
            (Entity, &ThisTick, &mut Transform, Option<&Home>, Option<&crate::actor::SpawnedAt>),
            (&mut crate::actor::Persona, &mut Health, Has<crate::actor::Npc>),
        ),
        With<Dead>,
    >,
) {
    for ((entity, this_tick, mut transform, home, spawned), (mut person, mut health, npc)) in &mut dead {
        if !this_tick.0.iter().any(|s| matches!(s.action, Action::Respawn)) {
            continue;
        }
        // What they come back with: modforge's rule.
        modforge::survival::respawn(&mut health.0, &mut person.0.needs);
        person.0.memory.did(modforge::memory::Did::Respawned, tick.0);
        // Home if they have one; a topside person where they were spawned
        // (they come back only where death does not stick); the player
        // wakes in their bunker.
        transform.translation = match (home, spawned) {
            (Some(home), _) => home.0 + Vec3::Y * BODY_CENTRE,
            (None, Some(spawned)) if npc => spawned.0 + Vec3::Y * BODY_CENTRE,
            (None, _) => wake_point(&world),
        };
        commands
            .entity(entity)
            .remove::<(Dead, ColliderDisabled)>()
            .insert(Visibility::Inherited);
    }
}
