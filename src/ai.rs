//! The AI's hand on an NPC's action queue. Every fixed step, before
//! the queues drain, this gathers what each NPC can see from the
//! world (the nearest actor of another faction) and pushes the
//! actions modforge's `decide` returns, stamped for this tick. From
//! there an NPC is driven exactly like the player: the same
//! `drain_actions`, `look`, `movement`, `fire`.

use bevy::prelude::*;
use modforge::actor::{Sight, decide};

use crate::SimTick;
use crate::actor::{Facing, Npc};
use crate::content::ActorRegistryRes;
use crate::input::Actions;
use crate::player::Player;

/// The player's faction name, for NPCs deciding who is a target.
pub const PLAYER_FACTION: &str = "player";

pub fn think(
    tick: Res<SimTick>,
    registry: Res<ActorRegistryRes>,
    mut npcs: Query<(Entity, &Npc, &Transform, &Facing, &mut Actions)>,
    others: Query<(Entity, &Transform, Option<&Npc>), Or<(With<Npc>, With<Player>)>>,
) {
    // Positions first so the mutable pass below has no second borrow.
    let actors: Vec<(Entity, Vec3, String)> = others
        .iter()
        .map(|(e, t, npc)| {
            let faction = npc
                .map(|n| n.faction.clone())
                .unwrap_or_else(|| PLAYER_FACTION.to_string());
            (e, t.translation, faction)
        })
        .collect();
    for (entity, npc, transform, facing, mut actions) in &mut npcs {
        let Some(def) = registry.0.def(&npc.def) else {
            continue;
        };
        let here = transform.translation;
        let target = actors
            .iter()
            .filter(|(e, _, faction)| *e != entity && *faction != npc.faction)
            .map(|(_, at, _)| [at.x - here.x, at.z - here.z])
            .min_by(|a, b| {
                let la = a[0] * a[0] + a[1] * a[1];
                let lb = b[0] * b[0] + b[1] * b[1];
                la.total_cmp(&lb)
            });
        let sight = Sight {
            yaw: facing.yaw,
            target,
        };
        for action in decide(def, &sight) {
            actions.0.push(tick.0, action);
        }
    }
}
