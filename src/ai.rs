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
use crate::content::{ActorRegistryRes, FactionRegistryRes, PLAYER_FACTION};
use crate::input::Actions;
use crate::player::Player;

/// Every fixed step: each NPC's nearest hostile (by the faction
/// table) becomes its target; a neutral sees no one and is seen by
/// no one.
pub fn think(
    tick: Res<SimTick>,
    registry: Res<ActorRegistryRes>,
    factions: Res<FactionRegistryRes>,
    mut npcs: Query<(Entity, &Npc, &Transform, &Facing, &mut Actions)>,
    others: Query<(Entity, &Transform, Option<&Npc>), Or<(With<Npc>, With<Player>)>>,
) {
    // Positions first so the mutable pass below has no second borrow.
    let actors: Vec<(Entity, Vec3, Option<String>)> = others
        .iter()
        .map(|(e, t, npc)| {
            let faction = match npc {
                Some(n) => n.faction.clone(),
                None => Some(PLAYER_FACTION.to_string()),
            };
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
            .filter(|(e, _, faction)| {
                *e != entity && factions.0.hostile(npc.faction.as_deref(), faction.as_deref())
            })
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
