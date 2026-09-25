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
static FPS_BITS: AtomicU32 = AtomicU32::new(0);
static DIAGNOSTICS: Mutex<Json> = Mutex::new(Json::Null);
static PEOPLE_LIST: Mutex<Json> = Mutex::new(Json::Null);
static PERF: Mutex<Json> = Mutex::new(Json::Null);
static WORLD: Mutex<Json> = Mutex::new(Json::Null);
static WAYS: Mutex<Json> = Mutex::new(Json::Null);
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
                &'static crate::actor::ActorActivity,
                &'static crate::actor::ActorCombat,
                &'static crate::actor::Memory,
                &'static crate::actor::Inventory,
                &'static crate::actor::Hotbar,
            ),
            (
                &'static crate::nav::NavPath,
                Option<&'static crate::actor::Home>,
                Option<&'static crate::actor::Store>,
                Has<crate::actor::Dead>,
            ),
        ),
    >,
}

/// Answer the `npc` op's question, if one is waiting: everything the game
/// knows about that person, and their chunk as the walk grid sees it.
pub fn answer_npc(view: PersonView, mut walk: ResMut<crate::nav::Walk>, tick: Res<crate::SimTick>) {
    let Some(id) = NPC_ASK.lock().ok().and_then(|mut ask| ask.take()) else {
        return;
    };
    let found = view.people.iter().find(|((actor, ..), _, _)| actor.0.0 == id);
    let answer = match found {
        None => json!({"id": id, "error": "no NPC with that id"}),
        Some(((_, npc, at, health, needs), (doing, fight, memory, inventory, hotbar), (way, home, store, dead))) => {
            let here = at.translation.truncate();
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
            let picture = chunk(walk.0.picture(here, '@'));
            let goal_picture = way.planned_at.map(|_| chunk(walk.0.picture(way.goal, 'G')));
            json!({
                "id": id,
                "tick": tick.0,
                "def": npc.def,
                "faction": npc.faction,
                "dead": dead,
                "at": [here.x, here.y],
                "health": health.0.current,
                "needs": {"hunger": needs.0.hunger, "thirst": needs.0.thirst, "rest": needs.0.rest, "safety": needs.0.safety},
                "doing": format!("{:?}", doing.0),
                "fight": format!("{:?}", fight.0),
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
            })
        }
    };
    if let Ok(mut slot) = NPC_ANSWER.lock() {
        *slot = Some((id, answer));
    }
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
static SPEED_BITS: AtomicU32 = AtomicU32::new(0x3f80_0000);

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
    life: Res<crate::ai::LifeStats>,
    items: Res<crate::content::ItemRegistryRes>,
    stores: Query<&crate::interact::Container>,
    people: Query<(&crate::actor::Store, &Npc, Has<crate::actor::Dead>, &Survival), With<crate::actor::Home>>,
) {
    if tick.0 % 60 != 0 {
        return;
    }
    let mut bunkers: std::collections::BTreeMap<String, Json> = Default::default();
    for (store, npc, dead, needs) in &people {
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
            json!({"food": food, "water": water, "alive": 0, "starving": 0})
        });
        if !dead {
            entry["alive"] = json!(entry["alive"].as_u64().unwrap_or(0) + 1);
        }
        if needs.0.starving() || needs.0.dehydrated() {
            entry["starving"] = json!(entry["starving"].as_u64().unwrap_or(0) + 1);
        }
    }
    let world = json!({
        "tick": tick.0,
        "day": crate::survival::Day::secs(&tick) / day.0,
        "storms": storms.shifts,
        "ate": life.ate,
        "looked": life.looked,
        "slept": life.slept,
        "fights": life.fights,
        "died": life.died,
        "taken": life.taken,
        "stocked": life.stocked,
        "bunkers": bunkers,
    });
    if let Ok(mut slot) = WORLD.lock() {
        *slot = world;
    }
}

pub fn publish(
    tick: Res<crate::SimTick>,
    laps: Res<crate::profile::Laps>,
    actors: Res<ActorMapRes>,
    diagnostics: Option<Res<DiagnosticsStore>>,
    players: Query<(&Transform, &Health, &Survival), With<Player>>,
    npcs: Query<(&Actor, &Npc, &Transform, &Health, &Survival, &Personality, &ThisTick)>,
) {
    TICK.store(tick.0, Ordering::Relaxed);
    PEOPLE.store(actors.0.live_count(), Ordering::Relaxed);
    if let Ok((at, health, needs)) = players.single() {
        HEALTH_BITS.store(health.0.current.to_bits(), Ordering::Relaxed);
        HUNGER_BITS.store(needs.0.hunger.to_bits(), Ordering::Relaxed);
        THIRST_BITS.store(needs.0.thirst.to_bits(), Ordering::Relaxed);
        X_BITS.store(at.translation.x.to_bits(), Ordering::Relaxed);
        Y_BITS.store(at.translation.y.to_bits(), Ordering::Relaxed);
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
            .map(|(actor, npc, at, health, needs, personality, this_tick)| {
                json!({
                    "id": actor.0.0,
                    "def": npc.def,
                    "faction": npc.faction,
                    "at": [at.translation.x, at.translation.y],
                    "health": health.0.current,
                    "hunger": needs.0.hunger,
                    "thirst": needs.0.thirst,
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
        "health": f(&HEALTH_BITS),
        "hunger": f(&HUNGER_BITS),
        "thirst": f(&THIRST_BITS),
        "fps": f(&FPS_BITS),
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
