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

pub fn publish(
    tick: Res<crate::SimTick>,
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
        "people",
        "every NPC: id, def, faction, position, health, needs, traits, actions this tick",
        "",
        |_| Ok(PEOPLE_LIST.lock().map(|p| p.clone()).unwrap_or(Json::Null)),
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
