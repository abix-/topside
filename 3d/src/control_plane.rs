//! The modforge control plane embedded in topside (authority row
//! 13): the one runtime inspection and op surface. Wired at startup
//! per the runtime-control-http doctrine.
//!
//! POST http://127.0.0.1:15703/topside with {"op": "...", "args": {}}.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use modforge::envelope::OpResponse;
use modforge::ops::{OP_REGISTRY, OpDef};
use serde_json::{Value as Json, json};

pub const PORT: u16 = 15703;

// The server handler runs on its own thread and cannot borrow the
// ECS world, so the simulation publishes a snapshot into atomics
// each tick and the handler reads only these.
static TICK: AtomicU64 = AtomicU64::new(0);
static PEOPLE: AtomicUsize = AtomicUsize::new(0);
static HUNGER_BITS: AtomicU32 = AtomicU32::new(0);
static THIRST_BITS: AtomicU32 = AtomicU32::new(0);
static FPS_BITS: AtomicU32 = AtomicU32::new(0);

pub fn publish_system(
    tick: Res<crate::SimTick>,
    actors: Res<crate::actor_map::ActorMapRes>,
    diagnostics: Option<Res<DiagnosticsStore>>,
    players: Query<&crate::survival::Survival, With<crate::player::Player>>,
    npcs: Query<(
        &crate::actor_map::Actor,
        &crate::actor::Npc,
        &Transform,
        &crate::combat::Health,
        &crate::survival::Survival,
        &crate::actor::Personality,
        &crate::input::ThisTick,
    )>,
) {
    TICK.store(tick.0, Ordering::Relaxed);
    PEOPLE.store(actors.0.live_count(), Ordering::Relaxed);
    if let Ok(survival) = players.single() {
        HUNGER_BITS.store(survival.0.hunger.to_bits(), Ordering::Relaxed);
        THIRST_BITS.store(survival.0.thirst.to_bits(), Ordering::Relaxed);
    }
    // Every person, once a second, for the `people` op: who they are,
    // where, how they are, and whether they did anything this tick.
    if tick.0 % 60 == 0 {
        let list: Vec<Json> = npcs
            .iter()
            .map(|(actor, npc, transform, health, needs, personality, this_tick)| {
                let at = transform.translation;
                json!({
                    "id": actor.0.0,
                    "def": npc.def,
                    "faction": npc.faction,
                    "at": [at.x, at.y, at.z],
                    "health": health.0.current,
                    "max_health": health.0.max,
                    "hunger": needs.0.hunger,
                    "thirst": needs.0.thirst,
                    "rest": needs.0.rest,
                    "safety": needs.0.safety,
                    "axes": personality.0.axes,
                    "acting": !this_tick.0.is_empty(),
                    "actions": this_tick.0.iter().map(|s| format!("{:?}", s.action)).collect::<Vec<_>>(),
                })
            })
            .collect();
        if let Ok(mut slot) = PEOPLE_LIST.lock() {
            *slot = Json::Array(list);
        }
    }
    // Frame rate exists only with a window; headless publishes 0.
    let Some(diagnostics) = diagnostics else {
        return;
    };
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|f| f.smoothed())
        .unwrap_or(0.0) as f32;
    FPS_BITS.store(fps.to_bits(), Ordering::Relaxed);
    // Every diagnostic Bevy records (frame time, entity count,
    // render pass CPU and GPU times), published once a second for
    // the `diagnostics` op.
    if tick.0 % 60 == 0 {
        let mut all: Vec<(String, f64)> = diagnostics
            .iter()
            .filter_map(|d| d.smoothed().map(|v| (d.path().as_str().to_string(), v)))
            .collect();
        all.sort_by(|a, b| a.0.cmp(&b.0));
        let map: serde_json::Map<String, Json> =
            all.into_iter().map(|(k, v)| (k, json!(v))).collect();
        if let Ok(mut slot) = DIAGNOSTICS.lock() {
            *slot = Json::Object(map);
        }
    }
}

static DIAGNOSTICS: Mutex<Json> = Mutex::new(Json::Null);
static PEOPLE_LIST: Mutex<Json> = Mutex::new(Json::Null);

fn snapshot() -> Json {
    json!({
        "tick": TICK.load(Ordering::Relaxed),
        "people": PEOPLE.load(Ordering::Relaxed),
        "hunger": f32::from_bits(HUNGER_BITS.load(Ordering::Relaxed)),
        "thirst": f32::from_bits(THIRST_BITS.load(Ordering::Relaxed)),
        "fps": f32::from_bits(FPS_BITS.load(Ordering::Relaxed)),
    })
}

pub fn start() {
    OP_REGISTRY.register(OpDef::new(
        "status",
        "current simulation snapshot",
        "",
        |_args| Ok(snapshot()),
    ));
    OP_REGISTRY.register(OpDef::new(
        "diagnostics",
        "every engine diagnostic: frame time, entity count, render pass times",
        "",
        |_args| Ok(DIAGNOSTICS.lock().map(|d| d.clone()).unwrap_or(Json::Null)),
    ));
    OP_REGISTRY.register(OpDef::new(
        "people",
        "every NPC: id, def, faction, position, health, needs, traits, what it did this tick",
        "",
        |_args| Ok(PEOPLE_LIST.lock().map(|p| p.clone()).unwrap_or(Json::Null)),
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
            let op = req
                .get("op")
                .and_then(Json::as_str)
                .unwrap_or_default()
                .to_string();
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
