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

/// How far from the mesh a point may be and still be on it.
fn sample_distance() -> PointSampleDistance3d {
    PointSampleDistance3d {
        horizontal_distance: 0.5,
        distance_above: 1.0,
        distance_below: 1.0,
        vertical_preference_ratio: 1.0,
        animation_link_max_vertical_distance: 1.0,
    }
}

fn setup(mut commands: Commands) {
    let archipelago = commands
        .spawn(Archipelago3d::new(ArchipelagoOptions {
            point_sample_distance: sample_distance(),
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
        (
            (Entity, &Transform, &mut crate::input::ThisTick, Option<&crate::actor::Personality>),
            (Option<&mut AgentTarget3d>, Option<&AgentDesiredVelocity3d>, Option<&mut Velocity3d>),
        ),
        Without<crate::actor::Dead>,
    >,
) {
    use crate::input::Action;
    use modforge::actions::{ActionQueue, Stamped};
    for ((entity, at, mut this_tick, personality), (target, desired, velocity)) in &mut people {
        let go = this_tick.0.iter().rev().find_map(|s| match s.action {
            Action::Go { x, y, z } => Some(Vec3::new(x, y, z)),
            _ => None,
        });
        match (go, target) {
            (Some(to), None) => {
                let speed = crate::player::WALK_SPEED * personality.map_or(1.0, |p| p.0.speed_mult());
                commands.entity(entity).insert((
                    Agent3dBundle {
                        agent: Default::default(),
                        settings: AgentSettings { radius: AGENT_RADIUS, desired_speed: speed, max_speed: speed * 1.2 },
                        archipelago_ref: ArchipelagoRef3d::new(nav.archipelago),
                    },
                    AgentTarget3d::Point(to),
                    Velocity3d::default(),
                ));
            }
            (Some(to), Some(mut target)) => *target = AgentTarget3d::Point(to),
            (None, Some(mut target)) => {
                *target = AgentTarget3d::None;
                continue;
            }
            (None, None) => continue,
        }
        let feet = crate::interact::feet_of(at);
        let v = desired.map_or(Vec3::ZERO, |d| d.velocity());
        if let Some(mut velocity) = velocity {
            velocity.velocity = v;
        }
        let way = Vec2::new(v.x, v.z);
        let dir = way.normalize_or_zero();
        this_tick.0.retain(|s| !matches!(s.action, Action::Move { .. } | Action::Aim { .. }));
        if dir != Vec2::ZERO {
            let ahead = feet.ground() + dir * 2.0;
            this_tick.0.push(Stamped { tick: tick.0, action: Action::Aim { x: ahead.x, y: ahead.y } });
        }
        this_tick.0.push(Stamped { tick: tick.0, action: Action::Move { x: dir.x, y: dir.y } });
        if dir != Vec2::ZERO && crate::nav::closed_door_ahead(&door_index, &doors, feet, dir) && !ActionQueue::has(&this_tick.0, Action::Use) {
            this_tick.0.push(Stamped { tick: tick.0, action: Action::Use });
        }
    }
}

/// Whether someone can stand at `at` (x, height, z): the mesh is there.
pub fn standable(archipelago: &Archipelago3d, at: Vec3) -> bool {
    archipelago.sample_point(at, &sample_distance()).is_ok()
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
        .find_map(|p| archipelago.sample_point(p, &sample_distance()).ok().map(|s| s.point()))
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
            let distance = sample_distance();
            match (archipelago.sample_point(from, &distance), archipelago.sample_point(to, &distance)) {
                (Ok(start), Ok(end)) => match archipelago.find_path(&start, &end, &HashMap::new(), bevy_landmass::PermittedAnimationLinks::All) {
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
                (start, end) => json!({"error": format!("not on the mesh: from {:?}, to {:?}", start.err(), end.err())}),
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
