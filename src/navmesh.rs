//! The navigation mesh (docs/pathing.md "A navigation mesh"): Recast's Rust
//! port, `bevy_rerecast`, built from the Avian colliders by
//! `avian_rerecast`, and walked with `bevy_landmass` (joined by
//! `landmass_rerecast`). One mesh per chunk, joined at the edges into one
//! connected world: each chunk's mesh is built when the chunk is made, a
//! tile of Recast's tiled mesh (its box padded by Recast's border and cut
//! back, so neighbours meet exactly), and is its own island in landmass,
//! which links islands along their shared edges. A chunk that changes is
//! built again alone. Prior art: Factorio's chunk pathfinding (Friday Facts
//! #317) and Unreal's tiled Recast mesh.

// Ordered maps and sets: whatever is iterated goes in the same order every
// run, so the mesh and the walk are a pure result of the game's state
// (authority.md "Pathing").
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Mutex;

use avian3d::prelude::*;
use avian_rerecast::prelude::*;
use bevy::math::bounding::Aabb3d;
use bevy::prelude::*;
use bevy_landmass::PointSampleDistance3d;
use bevy_landmass::prelude::*;
use bevy_rerecast::prelude::*;
use bevy_rerecast::{Navmesh, NavmeshSettings};
use landmass_rerecast::{Island3dBundle, LandmassRerecastPlugin, NavMeshHandle3d};
use modforge::walk::{CHUNK, ChunkKey, TILE};
use serde_json::{Value as Json, json};

use crate::Ground;
use crate::player::{BODY_HEIGHT, CLIMB, PLAYER_SIZE};

/// Everyone's body, as the mesh sees it: as wide and tall as a person, and
/// climbing what a person steps up.
const AGENT_RADIUS: f32 = PLAYER_SIZE / 2.0;
/// Cells in the agent's radius: a cell is 0.133 m, narrower than half a
/// step (0.28 m deep), so neighbouring cells on a stair differ by one
/// step's rise, under the climb; at 0.2 m two cells can be two steps apart,
/// over the climb, and Recast takes the flight for ledges.
const CELL_FRACTION: f32 = 3.0;
/// A chunk's mesh reaches from this far below the ground to this far above
/// (a building's deepest level and its highest: modforge MAX_LEVELS).
const DEEPEST: f32 = 64.0;
const HIGHEST: f32 = 64.0;
/// How near a doorway's middle someone standing is in the way of whoever
/// comes through it: half the doorway and two bodies (theirs, and room for
/// one to pass), metres; seen 2026-10-02, someone 1.1 m from a 1.2 m
/// doorway's middle left no way through.
const IN_THE_WAY_OF_A_DOORWAY: f32 = crate::nav::IN_DOORWAY + 2.0 * AGENT_RADIUS;
/// How far from a doorway's middle someone in its way steps out to, metres.
const STEP_OUT_OF_A_DOORWAY: f32 = IN_THE_WAY_OF_A_DOORWAY + 0.6;
/// How near ahead of a walker, heading at them, someone standing is in
/// their way (two bodies touch at 0.8 m), metres.
const IN_A_WALKERS_WAY: f32 = 1.2;
/// How far sideways someone in a walker's way steps, metres.
const STEP_ASIDE: f32 = 1.0;
/// Steps a walker makes no way, wanting to, before they step aside: half a
/// second at 60 a second.
const STUCK_STEPS: u32 = 30;
/// Steps a walker stands aside before going on: a second.
const STEP_ASIDE_FOR: u64 = 60;

pub struct NavPlugin;

impl Plugin for NavPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            NavmeshPlugins::default(),
            AvianBackendPlugin::default(),
            Landmass3dPlugin::default(),
            LandmassRerecastPlugin::default(),
        ));
        app.add_systems(Startup, setup);
        // In the fixed step, after the chunks are made: the mesh follows
        // the ticks, never the frames.
        app.add_systems(FixedUpdate, (wait_for_chunks, build_chunk_meshes, gather_meshes).chain().after(crate::actor::populate_topside_people));
        app.add_systems(FixedFirst, land_meshes);
        app.add_observer(mesh_failed);
        app.add_observer(chunks_changed_by_a_building);
        app.add_observer(chunks_changed_by_the_ground);
        app.add_systems(Update, answer_path);
    }
}

/// The archipelago everyone walks in, and every chunk's mesh: its island,
/// and when it waits to be built (again).
#[derive(Resource)]
pub struct Nav {
    pub archipelago: Entity,
    islands: BTreeMap<ChunkKey, (Handle<Navmesh>, Entity)>,
    waiting: BTreeMap<ChunkKey, u64>,
    /// Each chunk mesh being built: the tick it lands at (`land_meshes`).
    landing: BTreeMap<ChunkKey, (u64, AssetId<Navmesh>)>,
    /// Chunks whose colliders changed since their mesh was asked (a
    /// building stood up, the ground opened), written the moment it happens
    /// (`chunks_changed_by_*`) and taken once by `wait_for_chunks`.
    changed: BTreeSet<ChunkKey>,
}

impl Nav {
    /// Whether every chunk made has its mesh landed: none waiting to be
    /// built, none being built.
    pub fn settled(&self) -> bool {
        self.waiting.is_empty() && self.landing.is_empty()
    }
}

/// How far from the mesh a point may be and still be on it. Every point
/// landmass is asked about is at someone's feet, an agent's too (its own
/// entity stands at its person's feet, `Walker`): the floor is the surface
/// nearest the feet, from a step up (0.5 m) to a shelf's height down
/// (1.5 m: a thing on a shelf is reached from the floor under it). Asked at
/// the middle of the body, the first step of a flight going up beside a
/// landing was nearer than the landing, and people on it swung between the
/// two; a flight overhead was taken for the floor.
fn sample_distance() -> PointSampleDistance3d {
    PointSampleDistance3d {
        // The mesh keeps a body's radius (0.4 m) off every wall, to its
        // cells: someone standing against a wall is up to about 0.55 m off it.
        horizontal_distance: 1.0,
        distance_above: 0.5,
        distance_below: 1.5,
        vertical_preference_ratio: 1.0,
        animation_link_max_vertical_distance: 1.0,
    }
}

/// Where an agent is going.
pub fn going_to(target: &AgentTarget3d) -> Option<Vec3> {
    match target {
        AgentTarget3d::Point(p) => Some(*p),
        _ => None,
    }
}

/// A person's landmass agent: its own entity, a child standing at their
/// feet, because landmass reads an agent's place from its entity and a
/// person's own place is the middle of their body.
#[derive(Component)]
pub struct Walker(pub Entity);

/// Every agent's walk: landmass's state and where it is going.
pub type Ways<'w, 's> = Query<'w, 's, (&'static AgentState, &'static AgentTarget3d)>;

/// A person's walk on the mesh, through their `Walker`: landmass's state
/// and where they are going; None before their agent stands.
pub fn way_of<'a>(walker: Option<&Walker>, ways: &'a Ways) -> Option<(&'a AgentState, &'a AgentTarget3d)> {
    walker.and_then(|w| ways.get(w.0).ok())
}

fn setup(mut commands: Commands) {
    let archipelago = commands
        .spawn(Archipelago3d::new(ArchipelagoOptions {
            point_sample_distance: sample_distance(),
            // How far ahead the mesh's edges are steered away from, in
            // seconds: at a walk of 15 m/s, landmass's 0.25 s reaches 3.75 m,
            // every edge of a stairwell, and turned people back at the top
            // of the stairs; 0.05 s is 0.75 m. (landmass's steering stopped
            // the game with a NaN in dodgy's solving: the patched copy in
            // vendor/landmass guards it, Cargo.toml.)
            obstacle_avoidance_time_horizon: 0.05,
            ..ArchipelagoOptions::from_agent_radius(AGENT_RADIUS)
        }))
        .id();
    commands.insert_resource(Nav { archipelago, islands: BTreeMap::new(), waiting: BTreeMap::new(), landing: BTreeMap::new(), changed: BTreeSet::new() });
}

/// The chunks of a ground rectangle, and the eight round them (a building
/// standing in a chunk can reach into its neighbours).
fn chunks_round(lo: Vec2, hi: Vec2) -> impl Iterator<Item = ChunkKey> {
    let c = |v: f32| (v / (CHUNK as f32 * TILE)).floor() as i32;
    let (x0, z0, x1, z1) = (c(lo.x) - 1, c(lo.y) - 1, c(hi.x) + 1, c(hi.y) + 1);
    (z0..=z1).flat_map(move |z| (x0..=x1).map(move |x| (x, z)))
}

/// Every chunk made waits to have its mesh built; one gone (the storm
/// shifting the world) loses its island; a building standing, or the
/// ground opened over a way down, has the chunks round it built again.
fn wait_for_chunks(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    mut nav: ResMut<Nav>,
    chunks: Option<Res<crate::terrain::Chunks>>,
) {
    let Some(chunks) = chunks else {
        return;
    };
    // Asked this tick: a collider is in Avian's trees as soon as it is
    // spawned (its insert observers), so the chunk's colliders are there.
    let ready = tick.0;
    let gone: Vec<ChunkKey> = nav.islands.keys().filter(|k| !chunks.made.contains(k)).copied().collect();
    for key in gone {
        if let Some((_, island)) = nav.islands.remove(&key) {
            commands.entity(island).despawn();
        }
        nav.waiting.remove(&key);
    }
    let new: Vec<ChunkKey> = chunks.made.iter().filter(|k| !nav.islands.contains_key(k) && !nav.waiting.contains_key(k)).copied().collect();
    for key in new {
        nav.waiting.insert(key, ready);
    }
    // Taken once, whichever run comes first (in play, or the load's).
    let again: Vec<ChunkKey> = std::mem::take(&mut nav.changed).into_iter().filter(|k| nav.islands.contains_key(k)).collect();
    for key in again {
        nav.waiting.insert(key, ready);
    }
}

/// The chunk meshes due, all of them this tick: each built over its chunk's column
/// as one tile of Recast's tiled mesh, from the colliders reaching into it;
/// a new chunk gets its island.
fn build_chunk_meshes(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    mut nav: ResMut<Nav>,
    navmeshes: Res<Assets<Navmesh>>,
    spatial: SpatialQuery,
    mut generator: NavmeshGenerator,
) {
    let due: Vec<ChunkKey> = nav.waiting.iter().filter(|(_, t)| **t <= tick.0).map(|(k, _)| *k).collect();
    let size = CHUNK as f32 * TILE;
    let cell = AGENT_RADIUS / CELL_FRACTION;
    // Recast's border round a tile: the agent's radius in cells, and 3.
    let border = ((AGENT_RADIUS / cell).ceil() + 3.0) * cell;
    for key in due {
        nav.waiting.remove(&key);
        let lo = Vec3::new(key.0 as f32 * size - border, -DEEPEST, key.1 as f32 * size - border);
        let hi = Vec3::new((key.0 + 1) as f32 * size + border, HIGHEST, (key.1 + 1) as f32 * size + border);
        let reaching: bevy::platform::collections::HashSet<Entity> =
            spatial.aabb_intersections_with_aabb(ColliderAabb::new((lo + hi) / 2.0, (hi - lo) / 2.0)).into_iter().collect();
        let settings = NavmeshSettings {
            agent_radius: AGENT_RADIUS,
            agent_height: BODY_HEIGHT,
            walkable_climb: CLIMB,
            cell_size_fraction: CELL_FRACTION,
            tiling: true,
            tile_size: (size / cell).round() as u16,
            aabb: Some(Aabb3d::new((lo + hi) / 2.0, (hi - lo) / 2.0)),
            filter: Some(reaching),
            ..default()
        };
        let handle = match nav.islands.get(&key) {
            Some((handle, _)) => handle.clone(),
            None => {
                let handle = navmeshes.reserve_handle();
                let island = commands
                    .spawn(Island3dBundle {
                        island: Island,
                        archipelago_ref: ArchipelagoRef3d::new(nav.archipelago),
                        nav_mesh: NavMeshHandle3d(handle.clone()),
                    })
                    .id();
                nav.islands.insert(key, (handle.clone(), island));
                handle
            }
        };
        // Built from the world as it is this tick (`gather_meshes`, next),
        // landing `LAND_AFTER` ticks on. One already being built lands as
        // it was asked.
        if generator.regenerate(&handle, settings) {
            nav.landing.insert(key, (tick.0 + LAND_AFTER, handle.id()));
        }
    }
}

/// Ticks after a chunk's mesh is asked for that it lands: time for its
/// building in the background, so the step seldom waits for it.
const LAND_AFTER: u64 = 30;

/// Right after the meshes are asked for, the same tick: their obstacles
/// gathered from the world as it is now, and their building started in the
/// background (the patched bevy_rerecast's `gather`).
fn gather_meshes(world: &mut World) {
    let start = std::time::Instant::now();
    bevy_rerecast::generator::gather(world);
    let took = start.elapsed();
    if took > std::time::Duration::from_millis(1) {
        let tick = world.resource::<crate::SimTick>().0;
        info!("tick {tick}: gathering chunk meshes took {:.1} ms", took.as_secs_f64() * 1000.0);
    }
}

/// First in every fixed step: every chunk mesh due by the tick about to
/// run lands, its building waited for if it is not done (how long is
/// logged), so the mesh at each tick is the same however fast the
/// background was (authority.md rule 5). One that failed lands as no mesh
/// for its chunk, logged.
fn land_meshes(world: &mut World) {
    let next = world.resource::<crate::SimTick>().0 + 1;
    let due: Vec<(ChunkKey, AssetId<Navmesh>)> = {
        let mut nav = world.resource_mut::<Nav>();
        let keys: Vec<ChunkKey> = nav.landing.iter().filter(|(_, (at, _))| *at <= next).map(|(k, _)| *k).collect();
        keys.into_iter().filter_map(|k| nav.landing.remove(&k).map(|(_, id)| (k, id))).collect()
    };
    for (key, id) in due {
        let start = std::time::Instant::now();
        bevy_rerecast::generator::land(world, id);
        let waited = start.elapsed();
        if waited > std::time::Duration::from_millis(1) {
            info!("tick {next}: chunk {key:?}'s mesh was late; the step waited {:.1} ms for it", waited.as_secs_f64() * 1000.0);
        }
    }
}

/// A building stood up: the chunks round it changed, written the moment it
/// is spawned.
fn chunks_changed_by_a_building(add: On<Add, crate::structure::Shelter>, shelters: Query<&crate::structure::Shelter>, nav: Option<ResMut<Nav>>) {
    let (Ok(shelter), Some(mut nav)) = (shelters.get(add.entity), nav) else {
        return;
    };
    for (lo, hi) in &shelter.0 {
        nav.changed.extend(chunks_round(*lo, *hi));
    }
}

/// A chunk's ground slab made or made again (the ground opened over a way
/// down): that chunk changed.
fn chunks_changed_by_the_ground(add: On<Add, crate::terrain::GroundBody>, slabs: Query<&crate::terrain::GroundBody>, nav: Option<ResMut<Nav>>) {
    if let (Ok(slab), Some(mut nav)) = (slabs.get(add.entity), nav) {
        nav.changed.insert(slab.0);
    }
}

/// The world rolled again (`terrain::roll`: the storm's next world, a
/// load's saved one): every island gone, nothing waiting or landing, and
/// nothing built for the world before ever lands; the mesh is built again
/// from nothing as the new world's chunks are made.
pub fn forget(world: &mut World) {
    let Some(mut nav) = world.get_resource_mut::<Nav>() else {
        return;
    };
    let islands: Vec<Entity> = std::mem::take(&mut nav.islands).into_values().map(|(_, island)| island).collect();
    nav.waiting.clear();
    nav.landing.clear();
    nav.changed.clear();
    for island in islands {
        if let Ok(island) = world.get_entity_mut(island) {
            island.despawn();
        }
    }
    bevy_rerecast::generator::cancel_all(world);
}

/// Chunk meshes still building when a game was saved: each chunk with the
/// tick it lands at. Saved, so after a load they land at those same ticks
/// and every other chunk's mesh is there at once, as in the saved game.
#[derive(Resource, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct MeshesLanding(pub BTreeMap<ChunkKey, u64>);

/// The chunk meshes still building now, for a save.
pub fn landing_now(world: &World) -> MeshesLanding {
    MeshesLanding(world.get_resource::<Nav>().map(|nav| nav.landing.iter().map(|(k, (at, _))| (*k, *at)).collect()).unwrap_or_default())
}

/// After a load, at its tick, the game paused: the chunk systems made the
/// chunks round the loaded people, and here every chunk's mesh is asked
/// and gathered the same way as in play, then landed now, waited for,
/// except those the saved game still had building (`MeshesLanding`, from
/// the save), which land at their own ticks.
pub fn mesh_the_loaded_world(world: &mut World) {
    let _ = world.run_system_cached(wait_for_chunks);
    let _ = world.run_system_cached(build_chunk_meshes);
    gather_meshes(world);
    let later = world.get_resource::<MeshesLanding>().map(|m| m.0.clone()).unwrap_or_default();
    let now: Vec<(ChunkKey, AssetId<Navmesh>)> = {
        let mut nav = world.resource_mut::<Nav>();
        let mut now = Vec::new();
        for (key, (at, id)) in nav.landing.iter_mut() {
            match later.get(key) {
                Some(saved) => *at = *saved,
                None => now.push((*key, *id)),
            }
        }
        for (key, _) in &now {
            nav.landing.remove(key);
        }
        now
    };
    let start = std::time::Instant::now();
    let count = now.len();
    for (_, id) in now {
        bevy_rerecast::generator::land(world, id);
    }
    info!("the loaded world's {count} chunk meshes landed in {:.0} ms", start.elapsed().as_secs_f64() * 1000.0);
}

/// A chunk mesh that failed to build: no mesh for that chunk, said with
/// which chunk.
fn mesh_failed(failed: On<bevy_rerecast::generator::NavmeshFailed>, nav: Res<Nav>) {
    let key = nav.islands.iter().find(|(_, (h, _))| h.id() == failed.0).map(|(k, _)| *k);
    warn!("chunk {key:?}'s navigation mesh failed to build, so that chunk has no mesh: {}", failed.1);
}

/// Every step, once the queues have drained: everyone sent somewhere this
/// step (`Action::Go`, the brain's, a command's, following's) is a landmass
/// agent heading there, and walks the way the mesh gives: its desired
/// velocity is their move, and they face along it; a closed door just ahead
/// gets their Use. Sent nowhere, their agent stops. The keys' own moves
/// never go through here.
#[allow(clippy::type_complexity)]
pub fn walk(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    nav: Res<Nav>,
    door_index: Res<crate::nav::DoorIndex>,
    doors: Query<(&crate::interact::Door, &Transform)>,
    mut people: Query<
        (Entity, &Transform, &mut crate::input::ThisTick, Option<&crate::actor::Personality>, Option<&Walker>, Has<crate::player::Player>),
        Without<crate::actor::Dead>,
    >,
    mut velocities: Query<&mut Velocity3d>,
    mut agents: Query<(&mut AgentTarget3d, Option<&AgentDesiredVelocity3d>, &AgentState)>,
    archipelagos: Query<&Archipelago3d>,
    actors: Query<&crate::actor::Actor>,
    // Each walker's feet last step, how many steps they have made no way,
    // and the spot they step aside to, until the tick they go on.
    mut stuck: Local<BTreeMap<Entity, (Vec3, u32)>>,
    mut aside: Local<BTreeMap<Entity, (Vec3, u64)>>,
) {
    use crate::input::Action;
    use modforge::actions::{ActionQueue, Stamped};
    let archipelago = archipelagos.get(nav.archipelago).ok();
    // Everyone walking now: who, their feet, and the way they head (their
    // velocity, set below from their way or their keys).
    let walking: Vec<(Entity, Vec3, Vec2)> = people
        .iter()
        .filter_map(|(entity, at, _, _, walker, _)| {
            let v = velocities.get(walker?.0).ok()?.velocity;
            let way = Vec2::new(v.x, v.z);
            (way.length() > 0.1).then(|| (entity, crate::interact::feet_of(at), way.normalize()))
        })
        .collect();
    // Everyone's feet, walking or standing; and the walkers stuck as of the
    // last step.
    let everyone: Vec<(Entity, Vec3)> = people.iter().map(|(entity, at, ..)| (entity, crate::interact::feet_of(at))).collect();
    let stuck_now: Vec<(Entity, Vec3)> = stuck.iter().filter(|(_, (_, steps))| *steps > STUCK_STEPS).map(|(e, (feet, _))| (*e, *feet)).collect();
    for (entity, at, mut this_tick, personality, walker, is_player) in &mut people {
        let speed = crate::player::WALK_SPEED * personality.map_or(1.0, |p| p.0.speed_mult());
        // Everyone living has a landmass stand-in at their feet: a character
        // while they stand (walkers steer round them), an agent while they
        // walk (the way found and steered). Only walkers are agents: two
        // people standing on one spot (put there together) as agents gave
        // landmass's steering no way between them, and it failed.
        let Some(stand_in) = walker.map(|w| w.0) else {
            // A little off the feet, each person their own way (the golden
            // angle by their lasting id, the same after a load; the player
            // is zero): two people put on one spot (respawned at one home)
            // are never at one point to landmass, whose steering then had no
            // way between them and stopped the game.
            let id = actors.get(entity).map_or(0, |a| a.0.0);
            let off = Vec2::from_angle((id & 0xFFFF_FFFF) as f32 * 2.399_963) * 0.02;
            let stand_in = commands
                .spawn((
                    Transform::from_xyz(off.x, -crate::player::BODY_CENTRE, off.y),
                    character(nav.archipelago),
                    Velocity3d::default(),
                    ChildOf(entity),
                ))
                .id();
            commands.entity(entity).insert(Walker(stand_in));
            continue;
        };
        let go = this_tick.0.iter().rev().find_map(|s| match s.action {
            Action::Go { x, y, z } => Some(Vec3::new(x, y, z)),
            _ => None,
        });
        let walked = agents.get(stand_in).ok();
        let was_walking = walked.is_some();
        // A Go is done once landmass has brought them there: it is held no
        // longer (each step holds the last Go between decisions), and they
        // stand.
        let arrived = walked.is_some_and(|(target, _, state)| {
            matches!(state, AgentState::ReachedTarget) && go.is_some_and(|to| matches!(*target, AgentTarget3d::Point(p) if p == to))
        });
        if arrived {
            this_tick.0.retain(|s| !matches!(s.action, Action::Go { .. }));
        }
        let go = go.filter(|_| !arrived);
        // People standing make way (Bethesda's people step aside, RimWorld's
        // make way): someone not sent anywhere and not moving steps out of a
        // doorway they stand in the way of, to their own side, and out of
        // the way of anyone walking into them, sideways to their own side
        // of that walker's way. The player's keys are their own.
        let go = go.or_else(|| {
            let (x, y) = ActionQueue::movement(&this_tick.0);
            if is_player || x != 0.0 || y != 0.0 {
                return None;
            }
            let feet = crate::interact::feet_of(at);
            let out_of_a_doorway = || {
                let door = crate::nav::doorway_at(&door_index, &doors, feet, IN_THE_WAY_OF_A_DOORWAY)?;
                let (d, door_at) = doors.get(door).ok()?;
                let out = d.closed_center + d.across() * d.side_of(feet.ground()) * STEP_OUT_OF_A_DOORWAY;
                Some(crate::on_ground(out, door_at.translation.y))
            };
            let out_of_a_walkers_way = || {
                let (_, their_feet, way) = walking.iter().find(|(other, their_feet, way)| {
                    let to_me = feet.ground() - their_feet.ground();
                    *other != entity && (their_feet.y - feet.y).abs() < 1.5 && to_me.length() < IN_A_WALKERS_WAY && to_me.dot(*way) > 0.0
                })?;
                let side = if (feet.ground() - their_feet.ground()).dot(way.perp()) >= 0.0 { 1.0 } else { -1.0 };
                Some(vec![crate::on_ground(feet.ground() + way.perp() * side * STEP_ASIDE, feet.y)])
            };
            // A walker stuck right by them, whichever way: landmass gives
            // someone pressed against another no way to head, so their being
            // stuck is the sign; to either side of the line between them, or
            // straight away.
            let out_of_a_stuck_walkers_way = || {
                let (_, their_feet) = stuck_now
                    .iter()
                    .find(|(other, their_feet)| *other != entity && (their_feet.y - feet.y).abs() < 1.5 && their_feet.ground().distance(feet.ground()) < IN_A_WALKERS_WAY)?;
                let away = (feet.ground() - their_feet.ground()).normalize_or_zero();
                Some([away.perp(), -away.perp(), away].into_iter().map(|d| crate::on_ground(feet.ground() + d * STEP_ASIDE, feet.y)).collect())
            };
            let candidates: Vec<Vec3> = out_of_a_doorway().map(|s| vec![s]).or_else(out_of_a_walkers_way).or_else(out_of_a_stuck_walkers_way)?;
            // A spot on the mesh where it is meant to be, not somewhere near.
            candidates.into_iter().find_map(|c| archipelago.and_then(|a| on_mesh(a, c)).filter(|p| p.ground().distance(c.ground()) < 0.3))
        });
        let Some(to) = go else {
            // Not sent anywhere: walking by their own move (the keys) or
            // standing, which the others steer by. Sent somewhere until now:
            // they stop (or the Move this walker pushed is held on, each step
            // keeping the last move, input::drain_actions, and they walk on
            // into a wall), and stand as a character. Someone not walking is
            // never stuck.
            stuck.remove(&entity);
            aside.remove(&entity);
            if was_walking {
                this_tick.0.retain(|s| !matches!(s.action, Action::Move { .. }));
                this_tick.0.push(Stamped { tick: tick.0, action: Action::Move { x: 0.0, y: 0.0 } });
                commands
                    .entity(stand_in)
                    .remove::<(bevy_landmass::Agent3d, AgentSettings, AgentTarget3d, AgentState, AgentDesiredVelocity3d)>()
                    .insert(character(nav.archipelago));
            }
            if let Ok(mut velocity) = velocities.get_mut(stand_in) {
                let (x, y) = ActionQueue::movement(&this_tick.0);
                let way = Vec2::new(x, y).normalize_or_zero() * speed;
                velocity.velocity = Vec3::new(way.x, 0.0, way.y);
            }
            continue;
        };
        // Sent somewhere: an agent, heading there (from the next step, the
        // first time).
        let Ok((mut target, desired, _)) = agents.get_mut(stand_in) else {
            commands.entity(stand_in).remove::<(bevy_landmass::Character<ThreeD>, CharacterSettings)>().insert((
                Agent3dBundle {
                    agent: Default::default(),
                    settings: AgentSettings { radius: AGENT_RADIUS, desired_speed: speed, max_speed: speed * 1.2 },
                    archipelago_ref: ArchipelagoRef3d::new(nav.archipelago),
                },
                AgentTarget3d::Point(to),
            ));
            continue;
        };
        let feet = crate::interact::feet_of(at);
        // Stepping aside for someone in a narrow way: to that spot for a
        // moment, then on.
        let to = match aside.get(&entity) {
            // (Nearer than the brain's reach: the spot is only a step off.)
            Some(&(spot, until)) if tick.0 < until && feet.distance(spot) > 0.2 => spot,
            _ => {
                aside.remove(&entity);
                to
            }
        };
        *target = AgentTarget3d::Point(to);
        let v = desired.map_or(Vec3::ZERO, |d| d.velocity());
        if let Ok(mut velocity) = velocities.get_mut(stand_in) {
            velocity.velocity = v;
        }
        // Stuck (RimWorld's pawns, Bethesda's people): wanting to walk and
        // making no way for half a second with someone right ahead (two
        // walkers in a doorway, each in the other's way; landmass's steering
        // does not part them), they step aside, away from that person, and
        // then go on.
        // Stuck is sent somewhere and not moving: landmass gives no velocity
        // at all to someone pressed against another.
        let (last, steps) = stuck.get(&entity).copied().unwrap_or((feet, 0));
        let steps = if feet.distance(last) < 0.005 { steps + 1 } else { 0 };
        stuck.insert(entity, (feet, steps));
        if steps > STUCK_STEPS && !aside.contains_key(&entity) {
            // Ahead is the way they head, or with none, the way to where they go.
            let dir = Vec2::new(v.x, v.z).try_normalize().unwrap_or_else(|| (to - feet).ground().normalize_or_zero());
            let ahead = everyone.iter().find(|(other, their_feet)| {
                let to_them = their_feet.ground() - feet.ground();
                *other != entity && (their_feet.y - feet.y).abs() < 1.5 && to_them.length() < IN_A_WALKERS_WAY && to_them.dot(dir) > 0.0
            });
            if let Some((_, their_feet)) = ahead {
                let away = if (their_feet.ground() - feet.ground()).dot(dir.perp()) >= 0.0 { -1.0 } else { 1.0 };
                let spot = crate::on_ground(feet.ground() + dir.perp() * away * STEP_ASIDE, feet.y);
                if let Some(spot) = archipelago.and_then(|a| on_mesh(a, spot)) {
                    aside.insert(entity, (spot, tick.0 + STEP_ASIDE_FOR));
                }
            }
        }
        let way = Vec2::new(v.x, v.z);
        let dir = way.normalize_or_zero();
        this_tick.0.retain(|s| !matches!(s.action, Action::Move { .. } | Action::Aim { .. }));
        if dir != Vec2::ZERO {
            let ahead = feet.ground() + dir * 2.0;
            this_tick.0.push(Stamped { tick: tick.0, action: Action::Aim { x: ahead.x, y: ahead.y } });
        }
        this_tick.0.push(Stamped { tick: tick.0, action: Action::Move { x: dir.x, y: dir.y } });
        if dir != Vec2::ZERO && crate::nav::closed_door_ahead(&door_index, &doors, feet, dir, to) && !ActionQueue::has(&this_tick.0, Action::Use) {
            this_tick.0.push(Stamped { tick: tick.0, action: Action::Use });
        }
    }
}

/// A person standing, as landmass knows them: a character the walkers
/// steer round, as wide as a body.
fn character(archipelago: Entity) -> Character3dBundle {
    Character3dBundle {
        character: Default::default(),
        settings: CharacterSettings { radius: AGENT_RADIUS },
        archipelago_ref: ArchipelagoRef3d::new(archipelago),
    }
}

/// Where on the mesh someone with their feet at `feet` stands, if anywhere:
/// the one way a point is put on the mesh.
fn sample<'a>(archipelago: &'a Archipelago3d, feet: Vec3) -> Option<bevy_landmass::SampledPoint<'a, ThreeD>> {
    archipelago.sample_point(feet, &sample_distance()).ok()
}

/// Whether someone can stand with their feet at `at` (x, height, z): the
/// mesh is there.
pub fn standable(archipelago: &Archipelago3d, at: Vec3) -> bool {
    sample(archipelago, at).is_some()
}

/// The point on the mesh where someone with their feet at `at` would stand
/// (it can be up to `sample_distance` off `at`).
pub fn on_mesh(archipelago: &Archipelago3d, at: Vec3) -> Option<Vec3> {
    sample(archipelago, at).map(|s| s.point())
}

/// A spot `off` metres from `at` that someone can stand on, on the mesh at
/// its height: the first of eight ways round, starting from the angle
/// `from` on the ground.
pub fn stand_by(archipelago: &Archipelago3d, at: Vec3, off: f32, from: f32) -> Option<Vec3> {
    (0..8)
        .map(|k| {
            let d = off * Vec2::from_angle(from + k as f32 * std::f32::consts::FRAC_PI_4);
            at + Vec3::new(d.x, 0.0, d.y)
        })
        .find_map(|p| sample(archipelago, p).map(|s| s.point()))
}

/// The `nav_path` op's question (from, to, in the world: x, height, z), and
/// its last answer.
pub static PATH_ASK: Mutex<Option<(Vec3, Vec3)>> = Mutex::new(None);
pub static PATH_ANSWER: Mutex<Json> = Mutex::new(Json::Null);

/// The way the mesh gives between the asked points: each waypoint, or why
/// there is none; and how many chunk meshes there are and wait.
fn answer_path(nav: Res<Nav>, archipelagos: Query<&Archipelago3d>) {
    let Some((from, to)) = PATH_ASK.lock().ok().and_then(|mut ask| ask.take()) else {
        return;
    };
    let mut answer = match archipelagos.get(nav.archipelago) {
        Err(_) => json!({"error": "no archipelago"}),
        Ok(archipelago) => {
            match (sample(archipelago, from), sample(archipelago, to)) {
                (Some(start), Some(end)) => match archipelago.find_path(&start, &end, &HashMap::new(), bevy_landmass::PermittedAnimationLinks::All) {
                    Ok(steps) => {
                        let points: Vec<Json> = steps
                            .iter()
                            .filter_map(|s| match s {
                                bevy_landmass::PathStep::Waypoint(p) => Some(json!([p.x, p.y, p.z])),
                                _ => None,
                            })
                            .collect();
                        json!({"path": points})
                    }
                    Err(e) => json!({"error": format!("no way: {e:?}")}),
                },
                (start, end) => json!({"error": format!("not on the mesh: from {}, to {}", start.is_some(), end.is_some())}),
            }
        }
    };
    answer["from"] = json!([from.x, from.y, from.z]);
    answer["to"] = json!([to.x, to.y, to.z]);
    answer["chunk_meshes"] = json!(nav.islands.len());
    answer["waiting"] = json!(nav.waiting.len());
    if let Ok(mut slot) = PATH_ANSWER.lock() {
        *slot = answer;
    }
}
