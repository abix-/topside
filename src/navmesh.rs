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

use std::collections::{HashMap, HashSet};
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
/// Ticks after a chunk is made (or changes) before its mesh is built: its
/// colliders are in the physics world by then.
const SETTLE: u64 = 5;
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
/// The most chunk meshes started in one frame, so making many chunks at
/// once does not stall a frame (the building itself runs in the
/// background).
const PER_FRAME: usize = 8;

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
        app.add_systems(Update, (wait_for_chunks, build_chunk_meshes, answer_path).chain());
    }
}

/// The archipelago everyone walks in, and every chunk's mesh: its island,
/// and when it waits to be built (again).
#[derive(Resource)]
pub struct Nav {
    pub archipelago: Entity,
    islands: HashMap<ChunkKey, (Handle<Navmesh>, Entity)>,
    waiting: HashMap<ChunkKey, u64>,
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
            // of the stairs; 0.05 s is 0.75 m.
            obstacle_avoidance_time_horizon: 0.05,
            ..ArchipelagoOptions::from_agent_radius(AGENT_RADIUS)
        }))
        .id();
    commands.insert_resource(Nav { archipelago, islands: HashMap::new(), waiting: HashMap::new() });
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
    buildings: Query<&crate::structure::Shelter, Added<crate::structure::StructureRoot>>,
    opened: Query<&crate::terrain::GroundBody, Added<crate::terrain::GroundBody>>,
) {
    let Some(chunks) = chunks else {
        return;
    };
    let ready = tick.0 + SETTLE;
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
    let mut again: HashSet<ChunkKey> = HashSet::new();
    for shelter in &buildings {
        for (lo, hi) in &shelter.0 {
            again.extend(chunks_round(*lo, *hi));
        }
    }
    for slab in &opened {
        again.insert(slab.0);
    }
    let again: Vec<ChunkKey> = again.into_iter().filter(|k| nav.islands.contains_key(k)).collect();
    for key in again {
        nav.waiting.insert(key, ready);
    }
}

/// The chunk meshes due, a few a frame: each built over its chunk's column
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
    let due: Vec<ChunkKey> = nav.waiting.iter().filter(|(_, t)| **t <= tick.0).map(|(k, _)| *k).take(PER_FRAME).collect();
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
        generator.regenerate(&handle, settings);
    }
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
    mut agents: Query<(&mut AgentTarget3d, Option<&AgentDesiredVelocity3d>, &mut Velocity3d)>,
    archipelagos: Query<&Archipelago3d>,
) {
    use crate::input::Action;
    use modforge::actions::{ActionQueue, Stamped};
    let archipelago = archipelagos.get(nav.archipelago).ok();
    // Everyone walking now: who, their feet, and the way they head (their
    // agent's velocity, set below from their way or their keys).
    let walking: Vec<(Entity, Vec3, Vec2)> = people
        .iter()
        .filter_map(|(entity, at, _, _, walker, _)| {
            let v = agents.get(walker?.0).ok()?.2.velocity;
            let way = Vec2::new(v.x, v.z);
            (way.length() > 0.1).then(|| (entity, crate::interact::feet_of(at), way.normalize()))
        })
        .collect();
    for (entity, at, mut this_tick, personality, walker, is_player) in &mut people {
        let speed = crate::player::WALK_SPEED * personality.map_or(1.0, |p| p.0.speed_mult());
        let go = this_tick.0.iter().rev().find_map(|s| match s.action {
            Action::Go { x, y, z } => Some(Vec3::new(x, y, z)),
            _ => None,
        });
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
                Some(crate::on_ground(feet.ground() + way.perp() * side * STEP_ASIDE, feet.y))
            };
            let aside = out_of_a_doorway().or_else(out_of_a_walkers_way)?;
            archipelago.and_then(|a| on_mesh(a, aside))
        });
        // Everyone living is a landmass agent, sent somewhere or not: the
        // walkers step around each other and around whoever stands (landmass
        // steers every agent round the agents near it). The agent stands at
        // their feet.
        let Some((mut target, desired, mut velocity)) = walker.and_then(|w| agents.get_mut(w.0).ok()) else {
            if walker.is_none() {
                let agent = commands
                    .spawn((
                        Transform::from_xyz(0.0, -crate::player::BODY_CENTRE, 0.0),
                        Agent3dBundle {
                            agent: Default::default(),
                            settings: AgentSettings { radius: AGENT_RADIUS, desired_speed: speed, max_speed: speed * 1.2 },
                            archipelago_ref: ArchipelagoRef3d::new(nav.archipelago),
                        },
                        AgentTarget3d::None,
                        Velocity3d::default(),
                        ChildOf(entity),
                    ))
                    .id();
                commands.entity(entity).insert(Walker(agent));
            }
            continue;
        };
        let Some(to) = go else {
            // Not sent anywhere: walking by their own move (the keys) or
            // standing, which the others steer by. Sent somewhere until now:
            // they stop, or the Move this walker pushed is held on (each step
            // keeps the last move: input::drain_actions) and they walk on
            // into a wall.
            if matches!(*target, AgentTarget3d::Point(_)) {
                this_tick.0.retain(|s| !matches!(s.action, Action::Move { .. }));
                this_tick.0.push(Stamped { tick: tick.0, action: Action::Move { x: 0.0, y: 0.0 } });
            }
            *target = AgentTarget3d::None;
            let (x, y) = ActionQueue::movement(&this_tick.0);
            let way = Vec2::new(x, y).normalize_or_zero() * speed;
            velocity.velocity = Vec3::new(way.x, 0.0, way.y);
            continue;
        };
        *target = AgentTarget3d::Point(to);
        let feet = crate::interact::feet_of(at);
        let v = desired.map_or(Vec3::ZERO, |d| d.velocity());
        velocity.velocity = v;
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
