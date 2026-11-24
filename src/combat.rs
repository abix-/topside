//! Health, protection, and the one way a hit lands on an actor
//! (combat.md "How a hit is resolved"). modforge decides every hit
//! (`resolve_hit`) and what armor a person has (`Protection::worn`,
//! one total, no body parts); topside only finds who was hit and
//! carries out death.

use avian3d::prelude::*;
use bevy::prelude::*;
use modforge::combat::{
    BULLET_SPEED, BodyArea, FireTimer, Health as HealthStats, Hit, HitResult, MultiDamage, Protection as ProtectionStats,
    RollTimer, Swing, Trigger, pellet_directions_2d, resolve_hit,
};
use modforge::item::{self, ItemRegistry};
use modforge::memory::Did;

use modforge::actor::ActorId;
use modforge::item::numbers_of;

use crate::actor::{Actor, ActorMapRes, Dead, Facing, Hotbar, Inventory, Persona};
use crate::{Ground, on_ground};
use crate::content::{DamageRegistryRes, ItemRegistryRes, LayerRegistryRes};
use crate::input::{Action, ActionQueue, ThisTick};
use crate::interact::{Container, spawn_item};
use crate::player::{PLAYER_SIZE, Player};

/// Health every actor starts with (the 3D game's number).
pub const START_HEALTH: f32 = 100.0;

#[derive(Component, Debug, PartialEq, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Health(pub HealthStats);

impl Default for Health {
    fn default() -> Self {
        Self(HealthStats::new(START_HEALTH))
    }
}

/// A person's own protection before anything worn: armor and
/// resistances from what they are.
#[derive(Component, Default, Debug, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Protection(pub ProtectionStats);

/// What a person wears.
#[derive(Component, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Equipment(pub item::Equipment);

/// The health what a person wears adds to their most (modforge
/// `gear::worn_health`), kept so a change takes off the old before adding
/// the new.
#[derive(Component, Default, Clone, Copy, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct WornHealth(pub f32);

/// Each person whose wear changed: their most health moved by what it now
/// adds against what it added, their health kept under it (todo "Tiered
/// loot": health on gear).
pub fn worn_health(mut commands: Commands, mut people: Query<(Entity, &Equipment, &mut Health, Option<&WornHealth>), Changed<Equipment>>) {
    for (entity, worn, mut health, was) in &mut people {
        let now = modforge::gear::worn_health(&worn.0);
        let was = was.map_or(0.0, |w| w.0);
        if now == was {
            continue;
        }
        health.0.max = (health.0.max + now - was).max(1.0);
        health.0.current = health.0.current.min(health.0.max);
        commands.entity(entity).insert(WornHealth(now));
    }
}

/// The one path a hit lands on an actor: its protection is its own
/// plus every piece it wears on the `area` hit (combat.md "How deadly"),
/// then modforge's damage function decides. It goes into the actor's memory (life.md
/// "What a person did"): a hit by a person (`by`), and the hit that kills,
/// with who gave it and the damage's name (a weapon, hunger, the storm).
/// Returns the result and whether this hit killed.
#[allow(clippy::too_many_arguments)]
pub fn hit_actor(
    hit: &Hit<'_>,
    own: &Protection,
    worn: &Equipment,
    items: &ItemRegistry,
    layers: &modforge::item::LayerRegistry,
    area: BodyArea,
    health: &mut Health,
    memory: Option<&mut Persona>,
    by: Option<ActorId>,
    now: u64,
) -> (HitResult, bool) {
    let alive = health.0.current > 0.0;
    let mut protection = own.0.worn(&worn.0, items, layers, area);
    let result = resolve_hit(hit, &mut protection, &mut health.0);
    let killed = alive && health.0.current <= 0.0;
    if let Some(memory) = memory {
        if by.is_some() {
            memory.0.memory.did(Did::WasHit(by, result.damage_dealt), now);
        }
        if killed {
            memory.0.memory.did(Did::Died(by, hit.def.name.clone()), now);
            // The biggest blame, for what led here (life.md "Learning to
            // stay alive").
            memory.0.memory.learned.died(now);
        }
    }
    (result, killed)
}

/// When this person may swing or fire next, and what they are
/// committed to (modforge's fire timer: Quake 3's weapon timer with
/// Gothic's beat).
#[derive(Component, Default, Debug, PartialEq, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Firing(pub FireTimer);

/// A person's dodge roll (modforge's `RollTimer`, combat.md "The roll").
#[derive(Component, Default, Debug, PartialEq, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Rolling(pub RollTimer);

impl Rolling {
    /// Mid roll: committed to it, nothing else until it ends.
    pub fn busy(rolling: Option<&Rolling>) -> bool {
        rolling.is_some_and(|r| r.0.rolling())
    }

    /// In the safe first half: nothing can hurt them.
    pub fn safe(rolling: Option<&Rolling>) -> bool {
        rolling.is_some_and(|r| r.0.safe())
    }
}

/// Quake 3's g_knockback: modforge's knockback (damage over mass) times
/// this is the push in Quake units a second; a Quake unit is an inch, so
/// metres a second here (combat.md "Simple to use": a knock back).
const KNOCKBACK_SPEED: f32 = 1000.0 * 0.0254;
/// Quake 3's ground friction (pm_friction): the push loses this many
/// times itself a second.
const KNOCKBACK_FRICTION: f32 = 6.0;
/// Below this speed, metres a second, the push is over (Claude's).
const KNOCKBACK_STOP: f32 = 0.1;
/// How long a hit's flash shows, seconds (Claude's, for the operator to
/// change).
pub const FLASH_SECS: f32 = 0.15;

/// The push a hit gave this person, metres a second along the ground,
/// slowing as Quake 3's friction slows it (combat.md "Simple to use").
#[derive(Component, Default, Debug, PartialEq, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Knocked(pub Vec2);

/// A hit's flash still showing on this person: seconds left (drawn only:
/// the screen for the player, the body for anyone else).
#[derive(Component)]
pub struct Flash(pub f32);

/// Where on `target` a hit coming in at height `y` lands (combat.md "How
/// deadly": a hit lands on a part of the body): by how far up their body
/// it is (modforge `BodyArea::from_height`).
fn area_hit(y: f32, target: &Transform) -> BodyArea {
    let feet = crate::interact::feet_of(target).y;
    BodyArea::from_height((y - feet) / crate::player::BODY_HEIGHT)
}

/// Each person who looks with eyes (the player's camera): the height of
/// their eyes and how much their look rises a metre out, by the person.
/// Anyone else aims at the middle of the body they attack.
fn eye_lines(
    eyes: &Query<(&crate::player::Eyes, &ChildOf)>,
    bodies: impl Fn(Entity) -> Option<Vec3>,
) -> std::collections::HashMap<Entity, (f32, f32)> {
    eyes.iter()
        .filter_map(|(eyes, parent)| {
            let body = bodies(parent.parent())?;
            Some((parent.parent(), (body.y - crate::player::BODY_CENTRE + crate::EYE_HEIGHT, eyes.pitch.tan())))
        })
        .collect()
}

/// What every hit gives the one it lands on (combat.md "Simple to use":
/// clear feedback, a flash and a knock back): a push away from where it
/// came from, as big as modforge's knockback for it, and the flash.
fn feel_hit(commands: &mut Commands, target: Entity, from: Vec2, to: Vec2, result: &HitResult) {
    let push = (to - from).normalize_or_zero() * result.knockback * KNOCKBACK_SPEED;
    commands.entity(target).try_insert((Knocked(push), Flash(FLASH_SECS)));
}

/// Everyone knocked back slides with the push, walls stopping them, until
/// friction ends it.
pub fn knock_back(
    mut commands: Commands,
    time: Res<Time<Fixed>>,
    move_and_slide: MoveAndSlide,
    mut knocked: Query<(Entity, &Collider, &mut Transform, &mut Knocked), Without<Dead>>,
) {
    let dt = time.delta_secs();
    for (entity, shape, mut transform, mut knocked) in &mut knocked {
        if knocked.0.length() < KNOCKBACK_STOP {
            commands.entity(entity).remove::<Knocked>();
            continue;
        }
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        let to = move_and_slide
            .move_and_slide(
                shape,
                transform.translation,
                Quat::IDENTITY,
                on_ground(knocked.0, 0.0),
                time.delta(),
                &MoveAndSlideConfig::default(),
                &filter,
                |_| MoveAndSlideHitResponse::Accept,
            )
            .position;
        transform.translation = on_ground(to.ground(), transform.translation.y);
        knocked.0 *= (1.0 - KNOCKBACK_FRICTION * dt).max(0.0);
    }
}

/// The white shell a hit person's body flashes in.
#[derive(Component)]
pub struct FlashShell;

/// The screen the player's own hits flash red on.
#[derive(Component)]
pub struct FlashScreen;

/// The player's hit flash: the whole screen, red, clear at first.
pub fn setup_flash(mut commands: Commands) {
    commands.spawn((
        FlashScreen,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::NONE),
        bevy::picking::Pickable::IGNORE,
    ));
}

/// Each hit's flash while it shows: the player's screen goes red, anyone
/// else's body white (Enter the Gungeon's enemies flash white), fading
/// out over `FLASH_SECS`.
#[allow(clippy::type_complexity)]
pub fn flash(
    mut commands: Commands,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut shell: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
    mut flashed: Query<(Entity, &mut Flash, Has<Player>, Option<&Children>)>,
    shells: Query<(), With<FlashShell>>,
    mut screen: Query<&mut BackgroundColor, With<FlashScreen>>,
) {
    let (mesh, material) = shell
        .get_or_insert_with(|| {
            let radius = crate::player::PLAYER_SIZE / 2.0 + 0.05;
            (
                meshes.add(Capsule3d::new(radius, crate::player::BODY_HEIGHT - crate::player::PLAYER_SIZE)),
                materials.add(StandardMaterial { base_color: Color::WHITE, unlit: true, ..default() }),
            )
        })
        .clone();
    let mut red = 0.0;
    for (entity, mut flash, is_player, children) in &mut flashed {
        let shown: Vec<Entity> = children.into_iter().flatten().copied().filter(|c| shells.contains(*c)).collect();
        flash.0 -= time.delta_secs();
        if flash.0 <= 0.0 {
            for c in shown {
                commands.entity(c).despawn();
            }
            commands.entity(entity).remove::<Flash>();
            continue;
        }
        if is_player {
            red = (flash.0 / FLASH_SECS).max(red);
        } else if shown.is_empty() {
            commands.entity(entity).with_child((FlashShell, Mesh3d(mesh.clone()), MeshMaterial3d(material.clone())));
        }
    }
    for mut colour in &mut screen {
        colour.0 = Color::srgba(1.0, 0.0, 0.0, 0.4 * red);
    }
}

/// How far the overhead steps in, in metres.
pub const OVERHEAD_STEP: f32 = 0.5;

/// Every actor with the attack held this tick and a melee weapon in
/// its hands makes one of Gothic's four moves (combat.md "Melee"): the
/// move held, read relative to where it faces, picks the swing;
/// modforge's timer decides whether it goes now (the beat, commitment);
/// everyone inside the swing's shape takes one hit through
/// `hit_actor`, unless they are blocking. The overhead steps in.
pub fn melee(
    mut commands: Commands,
    time: Res<Time<Fixed>>,
    items: Res<ItemRegistryRes>,
    layers: Res<LayerRegistryRes>,
    damage: Res<DamageRegistryRes>,
    eyes: Query<(&crate::player::Eyes, &ChildOf)>,
    move_and_slide: MoveAndSlide,
    mut actors: ParamSet<(
        Query<(
            Entity,
            &ThisTick,
            &Facing,
            &Hotbar,
            &Health,
            Option<&Persona>,
            &Collider,
            Option<&Rolling>,
            Option<&Actor>,
            &mut Firing,
            &mut Transform,
        ), Without<Dead>>,
        Query<
            (
                Entity,
                &Transform,
                &Firing,
                Option<&Rolling>,
                &Protection,
                &Equipment,
                &mut Health,
                Option<&mut Persona>,
                Option<&Actor>,
            ),
            Without<Dead>,
        >,
    )>,
    tick: Res<crate::SimTick>,
    mut practice: ResMut<crate::skills::Practised>,
) {
    let dt = time.delta_secs();
    // Every swing that goes this tick: who, from where, which way,
    // which move, how far, how hard, with what.
    let mut swings: Vec<(Entity, ActorId, Vec2, Vec2, Swing, f32, f32, String)> = Vec::new();
    // In ActorId order (authority.md rule 5): of two swings on one target
    // in one tick, who lands first, and so who kills, is the same after a
    // load; each swing's targets too.
    let swingers = crate::actor::in_actor_order(actors.p0().iter().map(|(entity, .., actor, _, _)| (actor.map(|a| a.0), entity)));
    let targets = crate::actor::in_actor_order(actors.p1().iter().map(|(entity, .., actor)| (actor.map(|a| a.0), entity)));
    let mut swinging = actors.p0();
    for entity in swingers {
        let Ok((entity, this_tick, facing, hotbar, health, personality, shape, rolling, actor, mut firing, mut transform)) = swinging.get_mut(entity) else {
            continue;
        };
        firing.0.tick(dt);
        // What is in the hands: the held hotbar slot (Rust), its numbers
        // worked out from its layers; guns are their own row, this is the
        // hand weapons.
        let held = hotbar.0.held_stack();
        let weapon = held
            .and_then(|s| items.0.def(&s.item).and_then(|d| numbers_of(d, s, &layers.0).combat))
            .filter(|w| w.ammo.is_none());
        // A better tier hits harder (design.md "Loot randomization").
        // Its affixes too, all together.
        let quality = modforge::quality::power(held.and_then(|s| s.quality).map(|q| q.tier), &crate::content::QUALITY)
            * held.map_or(1.0, |s| modforge::affix::power(&s.affixes));
        // A practised fighter hits harder (todo 31).
        let skill = crate::skills::hit_scale(personality);
        let personality = personality.map(|p| p.0.personality).unwrap_or_default();
        // The swing pressed earlier lands once its wind-up has passed
        // (combat.md "Simple to use": every attack shows a wind-up before
        // it lands), with what is in the hands then.
        let lands = firing.0.lands();
        // Mid roll: committed to it, no swinging.
        if !lands && Rolling::busy(rolling) {
            continue;
        }
        if !lands {
            if !ActionQueue::has(&this_tick.0, Action::Attack) {
                // Let go: the next press may chain on the beat.
                firing.0.release();
                continue;
            }
            let Some(weapon) = weapon else {
                continue;
            };
            let (x, y) = ActionQueue::movement(&this_tick.0);
            let swing = Swing::from_ground_move(Vec2::new(x, y), facing.direction());
            // The press starts the wind-up; the hit comes later.
            firing
                .0
                .pull_melee(personality.swing_delay(weapon.delay), swing, personality.can_chain());
            continue;
        }
        let Some(weapon) = weapon else {
            continue;
        };
        let swing = firing.0.swing;
        if swing == Swing::Overhead {
            let filter = SpatialQueryFilter::from_excluded_entities([entity]);
            let to = move_and_slide
                .move_and_slide(
                    shape,
                    transform.translation,
                    Quat::IDENTITY,
                    on_ground(facing.direction() * OVERHEAD_STEP / dt, 0.0),
                    time.delta(),
                    &MoveAndSlideConfig::default(),
                    &filter,
                    |_| MoveAndSlideHitResponse::Accept,
                )
                .position;
            transform.translation = on_ground(to.ground(), transform.translation.y);
        }
        swings.push((
            entity,
            actor.map_or(ActorId(0), |a| a.0),
            transform.translation.ground(),
            facing.direction(),
            swing,
            weapon.reach * personality.range_mult(),
            personality.damage_mult(health.0.fraction()) * quality * skill,
            weapon.damage.clone(),
        ));
    }
    // Land them: one hit on everyone inside each swing's shape. What each
    // attacker did goes into their memory after: (attacker, whom, how
    // much, killed).
    let mut gave: Vec<(Entity, ActorId, f32, bool)> = Vec::new();
    let lines = {
        let bodies = actors.p1();
        eye_lines(&eyes, |e| bodies.get(e).ok().map(|(_, t, ..)| t.translation))
    };
    for (attacker, attacker_id, here, facing, swing, reach, scale, damage_name) in swings {
        let Some(def) = damage.0.def(&damage_name) else {
            continue;
        };
        let mut struck_ones = actors.p1();
        for target in &targets {
            let Ok((target, at, guard, rolling, own, target_worn, mut target_health, mut memory, target_id)) = struck_ones.get_mut(*target) else {
                continue;
            };
            let offset = at.translation.ground() - here;
            if target == attacker
                || !swing.covers(offset, facing, reach)
                || guard.0.blocking()
                || Rolling::safe(rolling)
            {
                continue;
            }
            // Where it lands: where the attacker looks, or the middle of
            // the body.
            let y = lines.get(&attacker).map_or(at.translation.y, |(eye, rise)| eye + rise * offset.length());
            let area = area_hit(y, at);
            let hit = Hit {
                def,
                self_inflicted: false,
                distance: offset.length(),
                location_scale: scale * area.scale(),
            };
            let (result, killed) = hit_actor(
                &hit,
                own,
                target_worn,
                &items.0,
                &layers.0,
                area,
                &mut target_health,
                memory.as_deref_mut(),
                Some(attacker_id),
                tick.0,
            );
            feel_hit(&mut commands, target, here, at.translation.ground(), &result);
            struck(memory, attacker_id, here, result.damage_dealt, tick.0);
            practice.0.push((attacker, crate::skills::COMBAT, (result.damage_dealt * crate::skills::DAMAGE_XP).round() as u64));
            if let Some(whom) = target_id {
                gave.push((attacker, whom.0, result.damage_dealt, killed));
            }
        }
    }
    let mut attackers = actors.p1();
    for (attacker, whom, amount, killed) in gave {
        if let Ok((.., Some(mut memory), _)) = attackers.get_mut(attacker) {
            gave_hit(&mut memory, whom, amount, killed, tick.0);
        }
    }
}

/// What a person who landed a hit remembers: whom they hit, and whom
/// they killed.
fn gave_hit(memory: &mut Persona, whom: ActorId, amount: f32, killed: bool, now: u64) {
    memory.0.memory.did(Did::Hit(whom, amount), now);
    if killed {
        memory.0.memory.did(Did::Killed(whom), now);
    }
}

/// Someone who takes a hit knows who gave it and where they stood
/// (life.md "Perception with belief": an NPC hears a hit on itself and
/// turns on a striker it never saw).
fn struck(memory: Option<Mut<Persona>>, by: ActorId, from: Vec2, amount: f32, now: u64) {
    if let Some(mut memory) = memory {
        memory.0.memory.hurt(by, amount);
        memory.0.memory.threat(by, Vec3::new(from.x, 0.0, from.y), now);
    }
}

/// One bullet in flight (combat.md "Guns": every shot a visible bullet
/// moved in the fixed step, a ray from its last position to its new
/// one, no drop).
#[derive(Component, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
#[require(moonshine_save::prelude::Save)]
pub struct Bullet {
    #[entities]
    pub shooter: Entity,
    pub damage: String,
    pub direction: Vec2,
    /// How much it rises a metre out along the ground: where the shooter
    /// looked (a look down falls), straight with no drop.
    #[serde(default)]
    pub rise: f32,
    /// How far it has flown along the ground, for falloff and the maximum
    /// range.
    pub travelled: f32,
    pub range: f32,
    /// How much harder it hits for its gun's quality tier (1 the base).
    #[serde(default = "base_power")]
    pub power: f32,
}

fn base_power() -> f32 {
    1.0
}

/// How high above the feet a bullet leaves anyone who aims without eyes,
/// in metres: chest height, flat (combat.md "Guns": no drop). One with
/// eyes (the player) shoots from them, where they look.
const BULLET_HEIGHT: f32 = 1.3;

/// Every actor with the trigger held and a gun in its hands fires
/// through modforge's timer: a shot takes one round of the gun's ammo
/// from the inventory (none left is the empty click) and sends one
/// bullet per pellet, spread by modforge's seeded pattern.
/// The fire timer is ticked once a step by `melee`, which runs before.
pub fn fire(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    items: Res<ItemRegistryRes>,
    layers: Res<LayerRegistryRes>,
    eyes: Query<(&crate::player::Eyes, &ChildOf)>,
    bodies: Query<&Transform, With<Actor>>,
    mut shooters: Query<
        (
            Entity,
            &ThisTick,
            &Facing,
            &Hotbar,
            &Transform,
            Option<&Rolling>,
            &mut Firing,
            Option<&mut Inventory>,
            Option<&crate::actor::Actor>,
        ),
        Without<Dead>,
    >,
) {
    let lines = eye_lines(&eyes, |e| bodies.get(e).ok().map(|t| t.translation));
    for (entity, this_tick, facing, hotbar, transform, rolling, mut firing, inventory, actor) in &mut shooters {
        // Mid roll: committed to it, no shooting.
        if Rolling::busy(rolling) {
            continue;
        }
        let held = hotbar.0.held_stack();
        let Some(gun) = held.and_then(|s| items.0.def(&s.item).and_then(|d| numbers_of(d, s, &layers.0).combat)) else {
            continue;
        };
        let power = modforge::quality::power(held.and_then(|s| s.quality).map(|q| q.tier), &crate::content::QUALITY)
            * held.map_or(1.0, |s| modforge::affix::power(&s.affixes));
        // Hand weapons are `melee`'s.
        let Some(ammo) = gun.ammo.as_deref() else {
            continue;
        };
        if !ActionQueue::has(&this_tick.0, Action::Attack) {
            continue;
        }
        let mut inventory = inventory;
        let round = inventory.as_mut().and_then(|inv| {
            inv.0
                .slots
                .iter()
                .position(|s| s.as_ref().is_some_and(|s| s.item == ammo))
        });
        if firing.0.pull(gun.delay, round.is_some()) != Trigger::Fire {
            continue;
        }
        if let (Some(inv), Some(slot)) = (inventory.as_mut(), round) {
            inv.0.remove(slot, 1);
        }
        let muzzle = transform.translation.ground() + facing.direction() * (PLAYER_SIZE / 2.0 + 0.1);
        // Seeded by who fires, by their lasting id, so a loaded game
        // spreads the same (the player is zero).
        let seed = tick.0 ^ actor.map_or(0, |a| a.0.0);
        let feet = crate::interact::feet_of(transform).y;
        let (height, rise) = lines.get(&entity).copied().unwrap_or((feet + BULLET_HEIGHT, 0.0));
        for direction in pellet_directions_2d(facing.direction(), gun.spread_degrees, gun.pellets, seed) {
            commands.spawn((
                Bullet {
                    shooter: entity,
                    damage: gun.damage.clone(),
                    direction,
                    rise,
                    travelled: 0.0,
                    range: gun.reach,
                    power,
                },
                crate::model::Model("pellet".to_string()),
                Transform::from_translation(on_ground(muzzle, height)),
            ));
        }
    }
}

/// Every bullet flies one fixed step: a ray from where it is to where
/// it will be. Items lying about do not stop it; a wall does; a person
/// takes the hit. Pellets that land on one person in one step are one
/// hit (modforge's `MultiDamage`), through `hit_actor`. Nothing flies
/// past its range.
pub fn fly(
    mut commands: Commands,
    time: Res<Time<Fixed>>,
    items: Res<ItemRegistryRes>,
    layers: Res<LayerRegistryRes>,
    damage: Res<DamageRegistryRes>,
    spatial: SpatialQuery,
    sensors: Query<(), With<Sensor>>,
    rollers: Query<&Rolling>,
    tick: Res<crate::SimTick>,
    shooters: Query<(&Transform, Option<&Actor>), Without<Bullet>>,
    mut bullets: Query<(Entity, &mut Bullet, &mut Transform)>,
    mut targets: Query<
        (&Transform, &Protection, &Equipment, &mut Health, Option<&mut Persona>, Option<&Actor>),
        (Without<Dead>, Without<Bullet>),
    >,
    mut practice: ResMut<crate::skills::Practised>,
) {
    let step = BULLET_SPEED * time.delta_secs();
    // (shooter, damage def, pellets summed per target, quality power) this
    // step.
    let mut landed: Vec<(Entity, String, MultiDamage<Entity>, f32)> = Vec::new();
    // Where on each target the first pellet to reach them landed.
    let mut areas: Vec<(Entity, BodyArea)> = Vec::new();
    for (entity, mut bullet, mut transform) in &mut bullets {
        let from = transform.translation.ground();
        let reach = step.min(bullet.range - bullet.travelled);
        let filter = SpatialQueryFilter::from_excluded_entities([bullet.shooter]);
        // Items lying about and anyone in the safe half of a roll let a
        // bullet through.
        let passes = |e: Entity| sensors.contains(e) || Rolling::safe(rollers.get(e).ok());
        // Straight along its line: `rise` up a metre out along the ground.
        let line = Vec3::new(bullet.direction.x, bullet.rise, bullet.direction.y);
        let slant = line.length().max(1e-6);
        let hit = Dir3::new(line).ok().and_then(|dir| {
            spatial.cast_ray_predicate(transform.translation, dir, reach * slant, true, &filter, &|e| !passes(e))
        });
        if let Some(hit) = hit {
            let along = hit.distance / slant;
            if let Ok((body, ..)) = targets.get(hit.entity) {
                let at = bullet.travelled + along;
                if !areas.iter().any(|(e, _)| *e == hit.entity) {
                    areas.push((hit.entity, area_hit(transform.translation.y + bullet.rise * along, body)));
                }
                match landed
                    .iter_mut()
                    .find(|(s, d, ..)| *s == bullet.shooter && *d == bullet.damage)
                {
                    Some((_, _, multi, _)) => multi.add(hit.entity, at),
                    None => {
                        let mut multi = MultiDamage::default();
                        multi.add(hit.entity, at);
                        landed.push((bullet.shooter, bullet.damage.clone(), multi, bullet.power));
                    }
                }
            }
            commands.entity(entity).despawn();
            continue;
        }
        bullet.travelled += reach;
        transform.translation = on_ground(from + bullet.direction * reach, transform.translation.y + bullet.rise * reach);
        if bullet.travelled >= bullet.range - 1e-4 {
            commands.entity(entity).despawn();
        }
    }
    // In ActorId order (authority.md rule 5): the shooters' hits land
    // shooter by shooter, and each one's targets, in ActorId order, so who
    // kills is the same after a load.
    let by_shooter = crate::actor::in_actor_order(landed.iter().map(|(s, ..)| (shooters.get(*s).ok().and_then(|(_, a)| a.map(|a| a.0)), *s)));
    let mut ordered = Vec::new();
    for shooter in by_shooter {
        if let Some(i) = landed.iter().position(|(s, ..)| *s == shooter) {
            ordered.push(landed.remove(i));
        }
    }
    for (shooter, name, mut multi, power) in ordered {
        let Some(def) = damage.0.def(&name) else {
            continue;
        };
        let (from, by) = shooters
            .get(shooter)
            .map_or((Vec2::ZERO, ActorId(0)), |(t, a)| (t.translation.ground(), a.map_or(ActorId(0), |a| a.0)));
        let hits: Vec<(Entity, u32, f32)> = multi.drain().into_iter().collect();
        let order = crate::actor::in_actor_order(hits.iter().map(|(t, ..)| (targets.get(*t).ok().and_then(|(.., a)| a.map(|a| a.0)), *t)));
        for (target, pellets, distance) in order.into_iter().filter_map(|t| hits.iter().find(|(h, ..)| *h == t).copied()) {
            // A practised shooter hits harder (todo 31).
            let skill = crate::skills::hit_scale(targets.get(shooter).ok().and_then(|(.., person, _)| person));
            let Ok((at, own, worn, mut health, mut memory, target_id)) = targets.get_mut(target) else {
                continue;
            };
            let to = at.translation.ground();
            let area = areas.iter().find(|(e, _)| *e == target).map_or(BodyArea::default(), |(_, a)| *a);
            let hit = Hit {
                def,
                self_inflicted: false,
                distance,
                location_scale: pellets as f32 * area.scale() * power * skill,
            };
            let (result, killed) =
                hit_actor(&hit, own, worn, &items.0, &layers.0, area, &mut health, memory.as_deref_mut(), Some(by), tick.0);
            feel_hit(&mut commands, target, from, to, &result);
            struck(memory, by, from, result.damage_dealt, tick.0);
            practice.0.push((shooter, crate::skills::COMBAT, (result.damage_dealt * crate::skills::DAMAGE_XP).round() as u64));
            let whom = target_id.map(|a| a.0);
            if let (Some(whom), Ok((.., Some(mut memory), _))) = (whom, targets.get_mut(shooter)) {
                gave_hit(&mut memory, whom, result.damage_dealt, killed, tick.0);
            }
        }
    }
}


/// Every actor at zero health dies the same way, the player included
/// (design.md "Death and difficulty": everyone is equal): everything it
/// carried (inventory, hotbar, worn gear) goes into one storage box
/// where it fell (modforge's `loot_all`, a box through `spawn_item`).
/// Someone with a bed (topside-town "Death and the bed": the player, the
/// bunker people, the townspeople) is then dead until they respawn in it
/// with nothing; so is a topside person where death does not stick
/// (realities.md, Loop), to wake where they were spawned. Anyone else,
/// the player too, leaves the actor map (its id never returns) and the
/// world.
pub fn die(
    mut commands: Commands,
    items: Res<ItemRegistryRes>,
    tick: Res<crate::SimTick>,
    reality: Res<crate::Reality>,
    mut map: ResMut<ActorMapRes>,
    mut ids: ResMut<crate::interact::ThingIssuer>,
    thing_ids: Res<crate::interact::ThingIds>,
    beds_at: Query<&Transform, (With<crate::interact::WorldItem>, Without<Dead>)>,
    mut dying: Query<
        (
            Entity,
            &mut Health,
            &Transform,
            Option<&crate::actor::Beds>,
            Option<&Actor>,
            Option<&mut Firing>,
            Option<&mut Inventory>,
            Option<&mut Hotbar>,
            Option<&mut Equipment>,
        ),
        Without<Dead>,
    >,
) {
    // The dying in ActorId order, never the order the query walks them
    // (authority.md rule 5): who dies first is issued the first death box's
    // id, the same after a load.
    let order = crate::actor::in_actor_order(
        dying.iter().filter(|(_, health, ..)| health.0.current <= 0.0).map(|(entity, _, _, _, actor, ..)| (actor.map(|a| a.0), entity)),
    );
    for entity in order {
        let Ok((entity, mut health, transform, wakes, actor, firing, inventory, hotbar, worn)) = dying.get_mut(entity) else {
            continue;
        };
        if let (Some(mut inventory), Some(mut hotbar), Some(mut worn)) = (inventory, hotbar, worn) {
            let loot = item::loot_all(&mut inventory.0, &mut hotbar.0.slots, &mut worn.0);
            if !loot.is_empty()
                && let Some(box_def) = items.0.def("storage box")
            {
                // A death box holds everything, however much that is.
                let mut contents = item::Inventory::new(loot.len().max(box_def.storage.unwrap_or(1)));
                for stack in loot {
                    let max = items.0.def(&stack.item).map_or(1, |d| d.max_stack);
                    contents.add(stack, max);
                }
                let salt = actor.map_or(0, |a| a.0.0);
                let (dropped, _) = spawn_item(
                    &mut commands,
                    &mut ids,
                    box_def,
                    item::create(box_def, 1, &[], tick.0 as f32, salt),
                    crate::interact::feet_of(transform),
                );
                commands.entity(dropped).insert(Container(contents));
            }
        }
        // A bed to wake in, the player's rule as everyone's (topside-town
        // "Death and the bed"; modforge `bed::wake_in`).
        // A broken bed is gone from the world: it is no bed.
        let has_bed = wakes.is_some_and(|b| modforge::bed::wake_in(&b.standing(&thing_ids, &beds_at)).is_some());
        if has_bed || !reality.0.death_sticks {
            // The death message and the choice to respawn (design.md
            // "Death and difficulty"); the world keeps running meanwhile.
            health.0.current = health.0.max;
            if let Some(mut firing) = firing {
                *firing = Firing::default();
            }
            commands
                .entity(entity)
                .insert((Dead, Visibility::Hidden, ColliderDisabled));
            continue;
        }
        leave(&mut commands, &mut map, entity, actor);
    }
}

/// A topside person is dead for good: out of the actor map's live count
/// (its id never returns) and never back, but still there, `Dead`, to
/// query, memory and all (life.md: a person who dies still exists). Dying
/// and the storm both end here.
pub fn leave(commands: &mut Commands, map: &mut ActorMapRes, entity: Entity, actor: Option<&Actor>) {
    if let Some(actor) = actor {
        map.0.remove(actor.0);
    }
    commands.entity(entity).insert((Dead, crate::actor::Gone, Visibility::Hidden, ColliderDisabled));
}
