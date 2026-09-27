//! The modforge control plane embedded in the 2D game: the one runtime
//! inspection surface (the runtime-control-http doctrine).
//!
//! POST http://127.0.0.1:15703/topside with {"op": "...", "args": {}}.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use modforge::envelope::OpResponse;
use modforge::ops::{OP_REGISTRY, OpDef};
use serde_json::{Value as Json, json};

use crate::Ground;
use crate::actor::{Actor, ActorMapRes, Npc, Personality, Survival};
use crate::combat::Health;
use crate::input::ThisTick;
use crate::player::Player;

pub const PORT: u16 = 15703;

// The server runs on its own thread and cannot borrow the ECS world,
// so the simulation publishes a snapshot here each tick.
static TICK: AtomicU64 = AtomicU64::new(0);
static PEOPLE: AtomicUsize = AtomicUsize::new(0);
static HEALTH_BITS: AtomicU32 = AtomicU32::new(0);
static HUNGER_BITS: AtomicU32 = AtomicU32::new(0);
static THIRST_BITS: AtomicU32 = AtomicU32::new(0);
static X_BITS: AtomicU32 = AtomicU32::new(0);
static Y_BITS: AtomicU32 = AtomicU32::new(0);
/// How high the player's feet stand above the ground (a floor upstairs).
static FEET_BITS: AtomicU32 = AtomicU32::new(0);
static FPS_BITS: AtomicU32 = AtomicU32::new(0);
/// How many times real time the game actually ran over the last second.
static RUNS_AT_BITS: AtomicU32 = AtomicU32::new(0);
static DIAGNOSTICS: Mutex<Json> = Mutex::new(Json::Null);
static PEOPLE_LIST: Mutex<Json> = Mutex::new(Json::Null);
static PERF: Mutex<Json> = Mutex::new(Json::Null);
static WORLD: Mutex<Json> = Mutex::new(Json::Null);
static PLAYER: Mutex<Json> = Mutex::new(Json::Null);
static WAYS: Mutex<Json> = Mutex::new(Json::Null);
/// The `kill` op's request: the ActorId to kill on the game's next step.
static KILL_ASK: Mutex<Option<u64>> = Mutex::new(None);
/// The `use` op's request: the ActorId, and the point of the thing to use.
static USE_ASK: Mutex<Option<(u64, Vec2)>> = Mutex::new(None);
/// The `talk` op's request: who speaks, to whom, saying what.
static TALK_ASK: Mutex<Option<(u64, u64, modforge::actions::Said)>> = Mutex::new(None);
/// The `npc` op's question (an ActorId) and the game's answer.
static NPC_ASK: Mutex<Option<u64>> = Mutex::new(None);
static NPC_ANSWER: Mutex<Option<(u64, Json)>> = Mutex::new(None);

/// Everything about one person, for the `npc` op.
#[derive(bevy::ecs::system::SystemParam)]
pub struct PersonView<'w, 's> {
    people: Query<
        'w,
        's,
        (
            (&'static Actor, &'static Npc, &'static Transform, &'static Health, &'static Survival),
            (
                &'static crate::actor::ActorRecord,
                &'static crate::actor::Memory,
                &'static crate::actor::Inventory,
                &'static crate::actor::Hotbar,
            ),
            (
                &'static crate::nav::NavPath,
                Option<&'static crate::actor::Home>,
                Option<&'static crate::actor::Store>,
                Has<crate::actor::Dead>,
                Option<&'static crate::actor::SpawnedAt>,
            ),
        ),
    >,
    /// What a solid thing is, to name what blocks a tile.
    things: Query<
        'w,
        's,
        (
            Option<&'static crate::structure::StructureRoot>,
            Option<&'static crate::interact::WorldItem>,
            Has<crate::terrain::WorldPiece>,
        ),
    >,
    /// What a person could use, to name what is in their reach.
    usable: Query<
        'w,
        's,
        (
            &'static crate::interact::Interactable,
            Option<&'static crate::interact::Door>,
            Option<&'static crate::interact::WorldItem>,
            &'static Transform,
        ),
    >,
    /// What they do this tick.
    this_tick: Query<'w, 's, (&'static Actor, &'static crate::input::ThisTick)>,
    chunks: Option<Res<'w, crate::terrain::Chunks>>,
}

/// The `kill` op's request, if one is waiting: that person takes the
/// `kill` hit through the one hit path, and `die` does the rest.
pub fn answer_kill(
    tick: Res<crate::SimTick>,
    items: Res<crate::content::ItemRegistryRes>,
    layers: Res<crate::content::LayerRegistryRes>,
    damage: Res<crate::content::DamageRegistryRes>,
    mut people: Query<
        (&Actor, &crate::combat::Protection, &crate::combat::Equipment, &mut Health, Option<&mut crate::actor::Memory>),
        Without<crate::actor::Dead>,
    >,
) {
    let Some(id) = KILL_ASK.lock().ok().and_then(|mut ask| ask.take()) else {
        return;
    };
    let Some(def) = damage.0.def("kill") else {
        return;
    };
    for (actor, own, worn, mut health, mut memory) in &mut people {
        if actor.0.0 != id {
            continue;
        }
        let hit = modforge::combat::Hit {
            def,
            self_inflicted: false,
            distance: 0.0,
            location_scale: 1.0,
        };
        crate::combat::hit_actor(&hit, own, worn, &items.0, &layers.0, &mut health, memory.as_deref_mut(), None, tick.0);
    }
}

/// The `use` op's request, if one is waiting: that person stands a metre
/// from the point and pushes Use aimed at it, the same Use the keys and the
/// brain push, so the one interact path does the rest.
pub fn answer_use(
    tick: Res<crate::SimTick>,
    mut people: Query<(&Actor, &mut Transform, &mut crate::input::Actions), Without<crate::actor::Dead>>,
) {
    let Some((id, at)) = USE_ASK.lock().ok().and_then(|mut ask| ask.take()) else {
        return;
    };
    for (actor, mut transform, mut actions) in &mut people {
        if actor.0.0 != id {
            continue;
        }
        transform.translation = crate::on_ground(at + Vec2::X, crate::player::BODY_CENTRE);
        actions.0.push(tick.0, crate::input::Action::Aim { x: at.x, y: at.y });
        actions.0.push(tick.0, crate::input::Action::Use);
    }
}

/// The `talk` op's request, if one is waiting: the speaker pushes the
/// `Talk`, the same the keys and the brain push; `talk::talk` does the rest.
pub fn answer_talk(tick: Res<crate::SimTick>, mut people: Query<(&Actor, &mut crate::input::Actions), Without<crate::actor::Dead>>) {
    let Some((id, to, said)) = TALK_ASK.lock().ok().and_then(|mut ask| ask.take()) else {
        return;
    };
    for (actor, mut actions) in &mut people {
        if actor.0.0 == id {
            actions.0.push(tick.0, crate::input::Action::Talk { to, said });
        }
    }
}

/// Answer the `npc` op's question, if one is waiting: everything the game
/// knows about that person, and their chunk as the walk grid sees it.
pub fn answer_npc(
    view: PersonView,
    mut walk: ResMut<crate::nav::Walk>,
    tick: Res<crate::SimTick>,
    tree: Res<crate::content::brain::BrainTreeRes>,
) {
    let Some(id) = NPC_ASK.lock().ok().and_then(|mut ask| ask.take()) else {
        return;
    };
    let found = view.people.iter().find(|((actor, ..), _, _)| actor.0.0 == id);
    let answer = match found {
        None => json!({"id": id, "error": "no NPC with that id"}),
        Some(((_, npc, at, health, needs), (record, memory, inventory, hotbar), (way, home, store, dead, spawned))) => {
            let here = at.translation.ground();
            // What blocks the tile they stand on, if anything.
            let standing_on: Vec<Json> = walk
                .0
                .solids_on(here)
                .into_iter()
                .map(|(key, r)| {
                    let what = Entity::try_from_bits(key)
                        .and_then(|e| view.things.get(e).ok())
                        .map_or("gone".to_string(), |(building, item, piece)| match (building, item) {
                            (Some(_), _) => "a building (wall or furniture)".to_string(),
                            (_, Some(item)) => format!("an item: {}", item.stack.item),
                            _ if piece => "the ground (water, trees, rocks) or a monument's props".to_string(),
                            _ => "something else".to_string(),
                        });
                    json!({"what": what, "min": [r.min.x, r.min.y], "max": [r.max.x, r.max.y]})
                })
                .collect();
            let stacks = |held: &modforge::item::Inventory| -> Vec<Json> {
                held.slots.iter().flatten().map(|s| json!({"item": s.item, "count": s.count})).collect()
            };
            let chunk = |p: modforge::walk::ChunkPicture| {
                json!({
                    "chunk": [p.chunk.0, p.chunk.1],
                    "tile": [p.tile.0, p.tile.1],
                    "doorways": p.doorways,
                    "doorways_reachable": p.reached,
                    "tiles": p.rows,
                })
            };
            let mut values: Vec<_> = memory.0.learned.values.iter().collect();
            values.sort_by(|a, b| b.1.total_cmp(a.1));
            let learned: Vec<Json> = values
                .into_iter()
                .map(|((s, c), v)| json!({"choice": format!("{c:?}"), "value": v, "situation": format!("{s:?}")}))
                .collect();
            // What Use would reach where they stand (interact_on_use): the
            // things within reach, and a river's water.
            let in_reach: Vec<String> = view
                .usable
                .iter()
                .filter(|(_, door, _, t)| crate::interact::use_point(*door, t).distance(here) <= crate::interact::REACH)
                .map(|(thing, _, item, _)| format!("{:?} {}", thing.kind, item.map_or("", |i| i.stack.item.as_str())))
                .collect();
            let water_in_reach = view.chunks.as_ref().and_then(|c| crate::interact::nearest_water(c, here, here));
            let acting: Vec<String> = view
                .this_tick
                .iter()
                .find(|(a, _)| a.0.0 == id)
                .map(|(_, t)| t.0.iter().map(|s| format!("{:?}", s.action)).collect())
                .unwrap_or_default();
            let picture = chunk(walk.0.picture(here, '@'));
            let goal_picture = way.planned_at.map(|_| chunk(walk.0.picture(way.goal, 'G')));
            json!({
                "id": id,
                "tick": tick.0,
                "def": npc.def,
                "faction": npc.faction,
                "dead": dead,
                "at": [here.x, here.y],
                "spawned_at": spawned.map(|s| [s.0.x, s.0.y]),
                "standing_on": standing_on,
                "in_reach": in_reach,
                "water_in_reach": water_in_reach.map(|w| [w.x, w.y]),
                "this_tick": acting,
                "health": health.0.current,
                "needs": {"hunger": needs.0.hunger, "thirst": needs.0.thirst, "rest": needs.0.rest, "safety": needs.0.safety},
                "doing": doing_name(&tree, &record.0),
                "about": format!("{:?}", record.0.target),
                "home": home.map(|h| [h.0.x, h.0.y]),
                "store": store.map(|s| s.0.to_bits()),
                "carrying": stacks(&inventory.0),
                "hotbar": stacks(&hotbar.0.slots),
                "knows": memory.0.known.iter().map(|k| json!({
                    "key": k.key,
                    "kind": k.kind,
                    "at": [k.position.x, k.position.z],
                    "seen_at": k.seen_at,
                    "checked_at": k.checked_at,
                    "held": k.held,
                })).collect::<Vec<_>>(),
                "way": {
                    "goal": [way.goal.x, way.goal.y],
                    "waypoints": way.waypoints.iter().map(|p| [p.x, p.y]).collect::<Vec<_>>(),
                    "planned_at": way.planned_at,
                    "queued": way.queued,
                    "retry_at": way.retry_at,
                },
                "chunk": picture,
                "goal_chunk": goal_picture,
                "did": memory.0.done.iter().map(|(t, d)| json!({"tick": t, "did": d.words()})).collect::<Vec<_>>(),
                // What they learned (life.md "Learning to stay alive"): how
                // long they could stay alive at their last think, how long
                // each life lasted, and each choice's value in each
                // situation, best first.
                "alive_for": memory.0.learned.last.map(|(_, secs)| secs),
                "lives": lives(&memory.0),
                "learned": learned,
            })
        }
    };
    if let Ok(mut slot) = NPC_ANSWER.lock() {
        *slot = Some((id, answer));
    }
}

/// How long each of a person's lives lasted, in seconds, from their
/// memory: from the start of the game or their last death to each death.
/// The life they are living is not in it.
pub fn lives(memory: &modforge::memory::Memory) -> Vec<f32> {
    let mut from = 0;
    let mut out = Vec::new();
    for (tick, did) in &memory.done {
        if matches!(did, modforge::memory::Did::Died(..)) {
            out.push(tick.saturating_sub(from) as f32 / 60.0);
            from = *tick;
        }
    }
    out
}

/// Once a second: the last searches that found no way, for the `ways`
/// op.
pub fn publish_ways(tick: Res<crate::SimTick>, failed: Res<crate::nav::FailedWays>) {
    if tick.0 % 60 != 0 {
        return;
    }
    let list: Vec<Json> = failed
        .0
        .iter()
        .map(|f| {
            json!({
                "tick": f.tick,
                "id": f.id,
                "who": f.who,
                "doing": f.doing,
                "from": [f.from.x, f.from.y],
                "to": [f.to.x, f.to.y],
                "distance": f.from.distance(f.to),
                "from_blocked": f.from_blocked,
                "to_blocked": f.to_blocked,
                "tiles_searched": f.searched,
            })
        })
        .collect();
    if let Ok(mut slot) = WAYS.lock() {
        *slot = Json::Array(list);
    }
}
/// How many times real time the game runs, set by the `speed` op
/// (Factorio's game speed); 1 is normal.
/// The game starts at 4 times real time (operator, 2026-09-26: a test
/// waits at most 3 minutes; the first night, where the thirst deaths
/// were, is past by tick 42,000, 2 minutes 55 seconds at 4x).
static SPEED_BITS: AtomicU32 = AtomicU32::new(0x4080_0000);

/// The `screenshot` op asked for one, taken the next frame.
static SCREENSHOT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Where the screenshot is saved.
const SCREENSHOT_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/logs/screenshot.png");

/// What the camera sees, once a second, for the `view` op.
static VIEW: Mutex<Json> = Mutex::new(Json::Null);

/// Once a second: where the camera is and looks, and how many meshes
/// there are, by material, and how many the camera sees.
pub fn publish_view(
    tick: Res<crate::SimTick>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    meshes: Query<(
        Has<MeshMaterial3d<StandardMaterial>>,
        Has<MeshMaterial3d<crate::model::Cel>>,
        &ViewVisibility,
    ), With<Mesh3d>>,
    models: Query<(&crate::model::Model, &Transform)>,
    map_view: Option<Res<crate::minimap::MapView>>,
    players: Query<(&Transform, &crate::actor::Map), With<Player>>,
) {
    if tick.0 % 60 != 0 {
        return;
    }
    // What the map last drew, against where the player stands and what
    // their map holds.
    let map = map_view.map(|v| {
        let player = players.single().ok().map(|(t, m)| {
            let tile = modforge::walk::cell_of(t.translation.ground());
            json!({"tile": [tile.0, tile.1], "tiles_seen": m.map.tiles_seen()})
        });
        json!({
            "full": v.full,
            "centre": [v.drawn.centre.0, v.drawn.centre.1],
            "span": v.drawn.span,
            "shown": v.drawn.shown,
            "player": player,
        })
    });
    // Floor tiles standing above the ground (a building's upper
    // storeys), and blocks standing above it (steps and landings).
    let above = |name: &str| models.iter().filter(|(m, t)| m.0 == name && t.translation.y > 0.5).count();
    let (floors_above, blocks_above) = (above("floor"), above("furniture"));
    let camera = cameras.iter().next().map(|c| {
        let (at, forward) = (c.translation(), c.forward());
        json!({"at": [at.x, at.y, at.z], "forward": [forward.x, forward.y, forward.z]})
    });
    let (mut standard, mut cel, mut seen, mut all) = (0, 0, 0, 0);
    for (is_standard, is_cel, visible) in &meshes {
        all += 1;
        standard += is_standard as u32;
        cel += is_cel as u32;
        seen += visible.get() as u32;
    }
    if let Ok(mut slot) = VIEW.lock() {
        *slot = json!({
            "camera": camera,
            "meshes": all,
            "standard": standard,
            "cel": cel,
            "seen": seen,
            "models": models.iter().count(),
            "floors_above": floors_above,
            "blocks_above": blocks_above,
            "map": map,
        });
    }
}

/// The layered items lying in the world, once a second, for the `items`
/// op.
static ITEMS: Mutex<Json> = Mutex::new(Json::Null);

/// Once a second: every item lying in the world with layers, its layers,
/// the numbers `numbers_of` works out from them, and the models it is
/// drawn with (its own and each child's).
pub fn publish_items(
    tick: Res<crate::SimTick>,
    registry: Res<crate::content::ItemRegistryRes>,
    layers: Res<crate::content::LayerRegistryRes>,
    items: Query<(&crate::interact::WorldItem, &Transform, Option<&crate::model::Model>, Option<&Children>)>,
    models: Query<&crate::model::Model>,
) {
    if tick.0 % 60 != 0 {
        return;
    }
    let list: Vec<Json> = items
        .iter()
        .filter(|(item, ..)| !item.stack.layers.is_empty())
        .map(|(item, at, own, children)| {
            let combat = registry
                .0
                .def(&item.stack.item)
                .and_then(|def| modforge::item::numbers_of(def, &item.stack, &layers.0).combat);
            let drawn: Vec<&str> = own
                .into_iter()
                .chain(children.into_iter().flat_map(|c| c.iter()).filter_map(|c| models.get(c).ok()))
                .map(|m| m.0.as_str())
                .collect();
            json!({
                "item": item.stack.item,
                "layers": item.stack.layers,
                "at": [at.translation.x, at.translation.z],
                "reach": combat.as_ref().map(|c| c.reach),
                "spread_degrees": combat.as_ref().map(|c| c.spread_degrees),
                "delay": combat.as_ref().map(|c| c.delay),
                "models": drawn,
            })
        })
        .collect();
    if let Ok(mut slot) = ITEMS.lock() {
        *slot = Json::Array(list);
    }
}

/// The inventory screen, once a second, for the `hud` op.
static HUD: Mutex<Json> = Mutex::new(Json::Null);

/// Once a second: whether the inventory screen is open, and each panel of
/// slots (the equipment row, the inventory, the belt, the open box): its
/// holder, how many slots it shows, and where its middle stands on the
/// window, 0 the left or top edge and 1 the right or bottom.
pub fn publish_hud(
    tick: Res<crate::SimTick>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    screen: Query<&Visibility, With<crate::hud::InventoryScreen>>,
    panels: Query<(&crate::hud::Panel, &UiGlobalTransform)>,
    slots: Query<(&crate::hud::SlotUi, &crate::hud::SlotIcons)>,
    item_panel: Query<&crate::hud::ItemPanel>,
) {
    if tick.0 % 60 != 0 {
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let (w, h) = (window.physical_width() as f32, window.physical_height() as f32);
    let list: Vec<Json> = panels
        .iter()
        .map(|(panel, at)| {
            json!({
                "holder": format!("{:?}", panel.0),
                "slots": slots.iter().filter(|(s, _)| s.0.holder == panel.0).count(),
                "x": at.translation.x / w,
                "y": at.translation.y / h,
            })
        })
        .collect();
    // Every slot with something in it, and the icons it shows.
    let filled: Vec<Json> = slots
        .iter()
        .filter_map(|(slot, icons)| {
            let item = icons.item.as_ref()?;
            Some(json!({
                "holder": format!("{:?}", slot.0.holder),
                "index": slot.0.index,
                "item": item,
                "icons": icons.shown,
            }))
        })
        .collect();
    // The item information panel: the selected slot and what it shows.
    let selected = item_panel.single().ok().and_then(|p| p.shown.as_ref()).map(|s| {
        json!({
            "holder": format!("{:?}", s.slot.holder),
            "index": s.slot.index,
            "name": s.name,
            "description": s.description,
            "information": s.information,
            "actions": s.actions.iter().map(|a| a.label()).collect::<Vec<_>>(),
            "count": s.count,
            "split": s.split,
        })
    });
    if let Ok(mut slot) = HUD.lock() {
        *slot = json!({
            "open": screen.single().is_ok_and(|v| *v != Visibility::Hidden),
            "panels": list,
            "filled": filled,
            "selected": selected,
        });
    }
}

/// What the window shows, saved to disk when the `screenshot` op asked.
pub fn take_screenshot(mut commands: Commands) {
    if SCREENSHOT.swap(false, Ordering::Relaxed) {
        use bevy::render::view::screenshot::{Screenshot, save_to_disk};
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(SCREENSHOT_PATH));
    }
}

/// The game speed the `speed` op asked for, applied to the clock the
/// fixed step runs on: the same game, more steps each frame. A frame may
/// catch up at most a second of game time, so a slow machine runs as fast
/// as it can, never skips steps.
pub fn apply_speed(mut time: ResMut<Time<Virtual>>) {
    let speed = f32::from_bits(SPEED_BITS.load(Ordering::Relaxed));
    if time.relative_speed() != speed {
        time.set_relative_speed(speed);
        time.set_max_delta(std::time::Duration::from_secs(1));
    }
}

/// Once a second of game time: the day, the storms, what the people have
/// done, and every bunker's store and people, for the `world` op.
pub fn publish_world(
    tick: Res<crate::SimTick>,
    day: Res<crate::survival::Day>,
    storms: Res<crate::storm::StormRes>,
    items: Res<crate::content::ItemRegistryRes>,
    memories: Query<&crate::actor::Memory>,
    stores: Query<&crate::interact::Container>,
    people: Query<
        (
            (&crate::actor::Store, &Npc, Has<crate::actor::Dead>, &Survival),
            (&crate::actor::Home, &Transform, &crate::actor::ActorCondition, &crate::actor::Memory),
        ),
    >,
    chunks: Option<Res<crate::terrain::Chunks>>,
    rolled: Option<Res<crate::terrain::WorldRes>>,
    standing: Option<Res<crate::structure::MonumentsRes>>,
    reality: Res<crate::Reality>,
    episode: Res<crate::Episode>,
) {
    if tick.0 % 60 != 0 {
        return;
    }
    let night = crate::survival::night(&tick, *day, &reality);
    // The reality the world is in now, and its rules (realities.md).
    let reality = json!({
        "name": reality.0.name,
        "danger": reality.0.danger.name(),
        "night": reality.0.night,
        "death_sticks": reality.0.death_sticks,
    });
    // The chunks made, and for a world with an edge, how many of the
    // chunks it is made whole with (the world and the ring past its
    // edge, terrain::make_chunks) are still not made.
    let made = chunks.as_ref().map_or(0, |c| c.made.len());
    let missing = rolled.as_ref().and_then(|w| w.world.ground.chunk_range()).map(|(lo, hi)| {
        (lo - 1..=hi)
            .flat_map(|y| (lo - 1..=hi).map(move |x| (x, y)))
            .filter(|key| !chunks.as_ref().is_some_and(|c| c.made.contains(key)))
            .count()
    });
    let mut bunkers: std::collections::BTreeMap<String, Json> = Default::default();
    for ((store, npc, dead, needs), (home, at, condition, memory)) in &people {
        let name = npc.faction.clone().unwrap_or_default();
        let entry = bunkers.entry(name).or_insert_with(|| {
            let (mut food, mut water) = (0, 0);
            if let Ok(held) = stores.get(store.0) {
                for stack in held.0.slots.iter().flatten() {
                    if let Some(f) = items.0.def(&stack.item).and_then(|d| d.food) {
                        if f.hunger > 0.0 {
                            food += stack.count;
                        }
                        if f.thirst > 0.0 {
                            water += stack.count;
                        }
                    }
                }
            }
            json!({"food": food, "water": water, "alive": 0, "starving": 0, "at_home": 0, "asleep": 0, "stocked": 0})
        });
        let add = |entry: &mut Json, field: &str, n: u64| {
            entry[field] = json!(entry[field].as_u64().unwrap_or(0) + n);
        };
        if !dead {
            add(entry, "alive", 1);
            // Home, as the brain counts it.
            if at.translation.ground().distance(home.0) <= crate::ai::HOME_REACH {
                add(entry, "at_home", 1);
            }
            if condition.0.asleep {
                add(entry, "asleep", 1);
            }
        }
        if needs.0.starving() || needs.0.dehydrated() {
            add(entry, "starving", 1);
        }
        // Times they put something into the store, from their memory.
        let stocked = memory.0.done.iter().filter(|(_, d)| matches!(d, modforge::memory::Did::Stocked(..))).count();
        add(entry, "stocked", stocked as u64);
    }
    // What everyone did, alive and dead, each kind counted, read from
    // their memories (life.md: memory is the one record).
    let mut did = serde_json::Map::new();
    for memory in &memories {
        for (_, d) in &memory.0.done {
            let n = did.get(d.kind()).and_then(Json::as_u64).unwrap_or(0);
            did.insert(d.kind().to_string(), json!(n + 1));
        }
    }
    let world = json!({
        "tick": tick.0,
        "day": crate::survival::Day::secs(&tick) / day.0,
        "night": night,
        // Whether the day clock is in its night hours, night or not in
        // this reality.
        "night_hours": crate::survival::night_hours(&tick, *day),
        "storms": storms.shifts,
        "storm": format!("{:?}", crate::storm::phase(&tick, *day, &storms)).to_lowercase(),
        "reality": reality,
        "episode": episode.name,
        // Each part and the ActorId cast for it.
        "cast": episode.cast.iter().map(|(part, id)| json!({"part": part, "id": id.0})).collect::<Vec<_>>(),
        // What the episode brought into the world, and where.
        "things": episode.things.iter().map(|(name, at, key)| json!({"name": name, "at": [at.x, at.y], "key": key})).collect::<Vec<_>>(),
        // Each pivot point reached: who and when.
        "reached": episode.reached.iter().map(|(pivot, who, at)| json!({"pivot": pivot, "by": who.0, "tick": at})).collect::<Vec<_>>(),
        "did": did,
        "bunkers": bunkers,
        "chunks": {"made": made, "missing": missing},
        // The seed this world rolled from, and the road tiles in its chunks.
        "seed": rolled.as_ref().map(|w| w.seed),
        "roads": chunks.as_ref().map_or(0, |c| c.roads),
        // Every monument standing: its kind, where, and how many buildings.
        "monuments": standing.as_ref().map_or(Vec::new(), |m| m.0.iter().map(|(at, def)| json!({
            "kind": def.kind,
            "at": [at.x, at.y],
            "buildings": def.members.len(),
        })).collect::<Vec<_>>()),
    });
    if let Ok(mut slot) = WORLD.lock() {
        *slot = world;
    }
}

/// What the brain's tree has a person doing: the active state's name, or
/// "nothing" between one thing and the next.
fn doing_name(tree: &crate::content::brain::BrainTreeRes, record: &modforge::statetree::Record) -> &'static str {
    record.state.map_or("nothing", |s| tree.0.name(s))
}

pub fn publish(
    tick: Res<crate::SimTick>,
    real: Res<Time<Real>>,
    mut last_second: Local<Option<(u64, f64)>>,
    laps: Res<crate::profile::Laps>,
    actors: Res<ActorMapRes>,
    diagnostics: Option<Res<DiagnosticsStore>>,
    items: Res<crate::content::ItemRegistryRes>,
    layers: Res<crate::content::LayerRegistryRes>,
    tree: Res<crate::content::brain::BrainTreeRes>,
    players: Query<(&Transform, &Health, &Survival, &Actor, &crate::actor::Memory), With<Player>>,
    npcs: Query<(
        (&Actor, &Npc, &Transform, &Health, &Survival, &Personality, &ThisTick, Option<&Name>),
        (&crate::actor::Memory, &crate::actor::ActorRecord, Has<crate::actor::Dead>, &crate::actor::Hotbar, &crate::actor::Facing, Option<&crate::people::Drawn>, &crate::actor::Map, &crate::combat::Equipment),
    )>,
) {
    TICK.store(tick.0, Ordering::Relaxed);
    PEOPLE.store(actors.0.live_count(), Ordering::Relaxed);
    // How fast the game runs against real time, measured once a second:
    // the game seconds its ticks covered over the real seconds that
    // passed (1 is keeping up).
    if tick.0 % 60 == 0 {
        let now = real.elapsed_secs_f64();
        if let Some((then_tick, then)) = *last_second
            && now > then
        {
            let speed = (tick.0 - then_tick) as f64 / 60.0 / (now - then);
            RUNS_AT_BITS.store((speed as f32).to_bits(), Ordering::Relaxed);
        }
        *last_second = Some((tick.0, now));
    }
    if let Ok((at, health, needs, actor, memory)) = players.single() {
        // Who the player is and what they know, once a second, for the
        // `player` op: every thing known (and who told them, if told), and
        // everything they heard said.
        if tick.0 % 60 == 0
            && let Ok(mut slot) = PLAYER.lock()
        {
            *slot = json!({
                "id": actor.0.0,
                "knows": memory.0.known.iter().map(|k| json!({"key": k.key, "kind": k.kind, "told_by": k.told_by.map(|a| a.0)})).collect::<Vec<_>>(),
                "heard": memory.0.done.iter().filter_map(|(t, d)| match d {
                    modforge::memory::Did::Heard(by, said) => Some(json!({"tick": t, "by": by.0, "said": said})),
                    _ => None,
                }).collect::<Vec<_>>(),
            });
        }
        HEALTH_BITS.store(health.0.current.to_bits(), Ordering::Relaxed);
        HUNGER_BITS.store(needs.0.hunger.to_bits(), Ordering::Relaxed);
        THIRST_BITS.store(needs.0.thirst.to_bits(), Ordering::Relaxed);
        // The ground point, and how high the player's feet are.
        X_BITS.store(at.translation.x.to_bits(), Ordering::Relaxed);
        Y_BITS.store(at.translation.z.to_bits(), Ordering::Relaxed);
        FEET_BITS.store((at.translation.y - crate::player::BODY_CENTRE).to_bits(), Ordering::Relaxed);
    }
    // Where the step's time goes, once a second: the whole step, every
    // part costliest first (a part named "steer: ..." is a piece inside
    // steer), and what was counted.
    if tick.0 % 60 == 0 {
        let parts: Vec<Json> = laps.sorted().into_iter().map(|(n, ms)| json!({"part": n, "ms": ms})).collect();
        let counts: Vec<Json> = laps.counts.iter().map(|(n, v)| json!({"what": n, "per_step": v})).collect();
        if let Ok(mut slot) = PERF.lock() {
            *slot = json!({
                "step_ms": laps.step_ms,
                "budget_ms": 1000.0 / 60.0,
                "people": actors.0.live_count(),
                "parts": parts,
                "counts": counts,
            });
        }
    }
    // Every person, once a second.
    if tick.0 % 60 == 0 {
        let list: Vec<Json> = npcs
            .iter()
            .map(|((actor, npc, at, health, needs, personality, this_tick, name), (memory, record, dead, hotbar, facing, drawn, map, worn))| {
                // What is in their hands, its layers, and the damage its
                // hits land, worked out by the one place a hit reads it.
                let weapon = hotbar.0.held_stack().map(|stack| {
                    let damage = items
                        .0
                        .def(&stack.item)
                        .and_then(|def| modforge::item::numbers_of(def, stack, &layers.0).combat)
                        .map(|c| c.damage);
                    json!({"item": stack.item, "layers": stack.layers, "damage": damage})
                });
                // How many of each kind of thing they did (life.md "What a
                // person did"), and the last one.
                let mut did = serde_json::Map::new();
                // What they ate and drank, by what: a can, a well, water.
                let mut ate_what = serde_json::Map::new();
                for (_, d) in &memory.0.done {
                    let n = did.get(d.kind()).and_then(Json::as_u64).unwrap_or(0);
                    did.insert(d.kind().to_string(), json!(n + 1));
                    if let modforge::memory::Did::Ate(what) = d {
                        let n = ate_what.get(what).and_then(Json::as_u64).unwrap_or(0);
                        ate_what.insert(what.clone(), json!(n + 1));
                    }
                }
                json!({
                    "id": actor.0.0,
                    "name": name.map(|n| n.as_str()),
                    "def": npc.def,
                    "faction": npc.faction,
                    "dead": dead,
                    "at": [at.translation.x, at.translation.y],
                    "health": health.0.current,
                    "hunger": needs.0.hunger,
                    "thirst": needs.0.thirst,
                    "rest": needs.0.rest,
                    "doing": doing_name(&tree, &record.0),
                    "weapon": weapon,
                    // Where they face on the ground, in radians (0 along +x).
                    "facing": facing.0,
                    // What they wear, each in its equipment slot (one row of
                    // plain slots).
                    "worn": (0..modforge::item::EQUIPMENT_SLOTS)
                        .filter_map(|i| worn.0.get(i).map(|s| json!({"slot": i, "item": s.item})))
                        .collect::<Vec<_>>(),
                    // How many tiles are on the map they carry.
                    "map_tiles": map.map.tiles_seen(),
                    "map_tiles_told": map.told,
                    // What their model shows (only with a window).
                    "drawn": drawn.map(|d| json!({"layers": d.layers, "held": d.held, "walking": d.walking})),
                    "did": did,
                    "ate_what": ate_what,
                    // Every death in their memory: when, of what, by whom.
                    "deaths": memory.0.done.iter().filter_map(|(t, d)| match d {
                        modforge::memory::Did::Died(by, of) => Some(json!({"tick": t, "of": of, "by": by.map(|a| a.0)})),
                        _ => None,
                    }).collect::<Vec<_>>(),
                    "last_did": memory.0.done.last().map(|(t, d)| json!({"tick": t, "did": d.words()})),
                    "killed_by": memory.0.killed_by().map(|(by, of)| json!({"by": by.map(|a| a.0), "of": of})),
                    "alive_for": memory.0.learned.last.map(|(_, secs)| secs),
                    "lives": lives(&memory.0),
                    "learned": memory.0.learned.values.len(),
                    "axes": personality.0.axes,
                    "actions": this_tick.0.iter().map(|s| format!("{:?}", s.action)).collect::<Vec<_>>(),
                })
            })
            .collect();
        if let Ok(mut slot) = PEOPLE_LIST.lock() {
            *slot = Json::Array(list);
        }
    }
    let Some(diagnostics) = diagnostics else {
        return;
    };
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|f| f.smoothed())
        .unwrap_or(0.0) as f32;
    FPS_BITS.store(fps.to_bits(), Ordering::Relaxed);
    if tick.0 % 60 == 0 {
        let map: serde_json::Map<String, Json> = diagnostics
            .iter()
            .filter_map(|d| d.smoothed().map(|v| (d.path().as_str().to_string(), json!(v))))
            .collect();
        if let Ok(mut slot) = DIAGNOSTICS.lock() {
            *slot = Json::Object(map);
        }
    }
}

fn f(bits: &AtomicU32) -> f32 {
    f32::from_bits(bits.load(Ordering::Relaxed))
}

fn snapshot() -> Json {
    json!({
        "tick": TICK.load(Ordering::Relaxed),
        "people": PEOPLE.load(Ordering::Relaxed),
        "player": [f(&X_BITS), f(&Y_BITS)],
        "player_feet": f(&FEET_BITS),
        "health": f(&HEALTH_BITS),
        "hunger": f(&HUNGER_BITS),
        "thirst": f(&THIRST_BITS),
        "fps": f(&FPS_BITS),
        "speed": f(&RUNS_AT_BITS),
    })
}

pub fn start() {
    OP_REGISTRY.register(OpDef::new("status", "current simulation snapshot", "", |_| Ok(snapshot())));
    OP_REGISTRY.register(OpDef::new(
        "diagnostics",
        "every engine diagnostic: frame time, entity count",
        "",
        |_| Ok(DIAGNOSTICS.lock().map(|d| d.clone()).unwrap_or(Json::Null)),
    ));
    OP_REGISTRY.register(OpDef::new(
        "perf",
        "the whole fixed step's ms against its budget, every part's ms costliest first (pieces inside a system named 'system: piece'), and counts per step",
        "",
        |_| Ok(PERF.lock().map(|p| p.clone()).unwrap_or(Json::Null)),
    ));
    OP_REGISTRY.register(OpDef::new(
        "people",
        "every NPC: id, def, faction, position, health, needs, traits, actions this tick",
        "",
        |_| Ok(PEOPLE_LIST.lock().map(|p| p.clone()).unwrap_or(Json::Null)),
    ));
    OP_REGISTRY.register(OpDef::new(
        "speed",
        "run the game this many times real time (1 is normal); as fast as the machine allows",
        "{\"times\": 20}",
        |args| {
            let times = args.get("times").and_then(Json::as_f64).ok_or_else(|| "times: a number".to_string())? as f32;
            if !(times > 0.0) {
                return Err("times must be above zero".to_string());
            }
            SPEED_BITS.store(times.to_bits(), Ordering::Relaxed);
            Ok(json!({"times": times}))
        },
    ));
    OP_REGISTRY.register(OpDef::new(
        "npc",
        "everything about one NPC by id (see `people`): who, where, health, needs, what the brain has them doing, what they know and carry, their way, and their chunk drawn from the walk grid with how many doorways they can reach",
        "{\"id\": 42}",
        |args| {
            let id = args.get("id").and_then(Json::as_u64).ok_or_else(|| "id: a number".to_string())?;
            if let Ok(mut ask) = NPC_ASK.lock() {
                *ask = Some(id);
            }
            // The game answers on its next step.
            for _ in 0..100 {
                std::thread::sleep(std::time::Duration::from_millis(10));
                if let Ok(mut slot) = NPC_ANSWER.lock()
                    && slot.as_ref().is_some_and(|(asked, _)| *asked == id)
                {
                    return Ok(slot.take().map(|(_, a)| a).unwrap_or(Json::Null));
                }
            }
            Err("the game did not answer within a second".to_string())
        },
    ));
    OP_REGISTRY.register(OpDef::new(
        "ways",
        "the last searches that found no way: who, what they were doing, from, to, whether the tiles at each end are blocked, tiles searched",
        "",
        |_| Ok(WAYS.lock().map(|w| w.clone()).unwrap_or(Json::Null)),
    ));
    OP_REGISTRY.register(OpDef::new(
        "world",
        "the day, the storms, what the people have done, every bunker's store and people",
        "",
        |_| Ok(WORLD.lock().map(|w| w.clone()).unwrap_or(Json::Null)),
    ));
    OP_REGISTRY.register(OpDef::new(
        "view",
        "where the camera is and looks, how many meshes there are by material, and how many the camera sees",
        "",
        |_| Ok(VIEW.lock().map(|v| v.clone()).unwrap_or(Json::Null)),
    ));
    OP_REGISTRY.register(OpDef::new(
        "items",
        "every item lying in the world with layers: its layers, its reach, spread and delay from them, and the models it is drawn with",
        "",
        |_| Ok(ITEMS.lock().map(|v| v.clone()).unwrap_or(Json::Null)),
    ));
    OP_REGISTRY.register(OpDef::new(
        "hud",
        "the inventory screen: whether it is open, each panel of slots with its holder, slot count, and where it stands on the window, and every filled slot with the icons it shows",
        "",
        |_| Ok(HUD.lock().map(|v| v.clone()).unwrap_or(Json::Null)),
    ));
    OP_REGISTRY.register(OpDef::new(
        "storm",
        "bring the next storm now: the sky warns on the game's next step, then the storm hits and the world shifts to the next reality, as any storm does; `reality` names the one it shifts to",
        "{\"reality\": \"Loop\"}",
        |args| {
            let reality = args.get("reality").and_then(Json::as_str).map(str::to_string);
            if let Some(name) = &reality
                && crate::content::RealityRegistryRes::default().0.def(name).is_none()
            {
                return Err(format!("no reality named '{name}'"));
            }
            if let Ok(mut next) = crate::storm::NEXT_REALITY.lock() {
                *next = reality.clone();
            }
            crate::storm::STORM_NOW.store(true, Ordering::Relaxed);
            Ok(json!({"storm": "coming", "reality": reality}))
        },
    ));
    OP_REGISTRY.register(OpDef::new(
        "kill",
        "kill one living person by id (see `people`) on the game's next step, through the one hit path, so a test can bring a death now",
        "{\"id\": 42}",
        |args| {
            let id = args.get("id").and_then(Json::as_u64).ok_or_else(|| "id: a number".to_string())?;
            if let Ok(mut ask) = KILL_ASK.lock() {
                *ask = Some(id);
            }
            Ok(json!({"kill": id}))
        },
    ));
    OP_REGISTRY.register(OpDef::new(
        "use",
        "a living person by id (see `people`) stands a metre from the point and uses the thing there on the game's next step, through the one interact path, so a test brings a use now",
        "{\"id\": 42, \"at\": [10.0, 20.0]}",
        |args| {
            let id = args.get("id").and_then(Json::as_u64).ok_or_else(|| "id: a number".to_string())?;
            let at = args
                .get("at")
                .and_then(Json::as_array)
                .and_then(|a| Some(Vec2::new(a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32)))
                .ok_or_else(|| "at: [x, y]".to_string())?;
            if let Ok(mut ask) = USE_ASK.lock() {
                *ask = Some((id, at));
            }
            Ok(json!({"use": id, "at": [at.x, at.y]}))
        },
    ));
    OP_REGISTRY.register(OpDef::new(
        "player",
        "the player: their id, every thing they know (key, kind, and who told them, if told), and everything they heard said",
        "",
        |_| Ok(PLAYER.lock().map(|p| p.clone()).unwrap_or(Json::Null)),
    ));
    OP_REGISTRY.register(OpDef::new(
        "talk",
        "one living person by id says something to another (`to`) on the game's next step, through the one talk path: said is {\"Tell\": {\"key\": k}}, {\"Ask\": {\"key\": k}}, {\"Offer\": {\"slot\": n}}, or \"Threaten\"; a thing's key is the one in the `npc` op's knowledge",
        "{\"id\": 3, \"to\": 1, \"said\": {\"Tell\": {\"key\": 42}}}",
        |args| {
            let id = args.get("id").and_then(Json::as_u64).ok_or_else(|| "id: a number".to_string())?;
            let to = args.get("to").and_then(Json::as_u64).ok_or_else(|| "to: a number".to_string())?;
            let said: modforge::actions::Said =
                serde_json::from_value(args.get("said").cloned().unwrap_or(Json::Null)).map_err(|e| format!("said: {e}"))?;
            if let Ok(mut ask) = TALK_ASK.lock() {
                *ask = Some((id, to, said));
            }
            Ok(json!({"talk": id, "to": to, "said": format!("{said:?}")}))
        },
    ));
    OP_REGISTRY.register(OpDef::new(
        "screenshot",
        "save what the game window shows next frame to logs/screenshot.png",
        "",
        |_| {
            SCREENSHOT.store(true, Ordering::Relaxed);
            Ok(json!({"path": SCREENSHOT_PATH}))
        },
    ));
    modforge::server::spawn(
        modforge::server::Config {
            port: PORT,
            endpoint: "/topside",
            thread_name: "topside-control-plane",
            auth_token: None,
        },
        |body| {
            let req: Json = serde_json::from_str(body).unwrap_or(Json::Null);
            let op = req.get("op").and_then(Json::as_str).unwrap_or_default().to_string();
            let args = req.get("args").cloned().unwrap_or(Json::Null);
            let resp = match OP_REGISTRY.dispatch(&op, &args) {
                Some(r) => OpResponse::from_result(&op, r, snapshot()),
                None => OpResponse::err(&op, format!("unknown op '{op}'"), snapshot()),
            };
            serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec())
        },
        |line| eprintln!("[topside control plane] {line}"),
    );
}
