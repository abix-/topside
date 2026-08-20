//! The modforge control plane embedded in topside (authority row
//! 13): the one runtime inspection and op surface. Wired at startup
//! per the runtime-control-http doctrine.
//!
//! POST http://127.0.0.1:15703/topside with {"op": "...", "args": {}}.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

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
static FOOD_DAYS_BITS: AtomicU32 = AtomicU32::new(0);

pub fn publish_system(
    tick: Res<crate::SimTick>,
    map: Res<crate::entity_map::EntityMap>,
    food: Res<crate::bunker::FoodStore>,
) {
    TICK.store(tick.0, Ordering::Relaxed);
    PEOPLE.store(map.npc_count(), Ordering::Relaxed);
    FOOD_DAYS_BITS.store(food.days.to_bits(), Ordering::Relaxed);
}

fn snapshot() -> Json {
    json!({
        "tick": TICK.load(Ordering::Relaxed),
        "people": PEOPLE.load(Ordering::Relaxed),
        "food_days": f32::from_bits(FOOD_DAYS_BITS.load(Ordering::Relaxed)),
    })
}

pub fn start() {
    OP_REGISTRY.register(OpDef::new(
        "status",
        "current simulation snapshot",
        "",
        |_args| Ok(snapshot()),
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
