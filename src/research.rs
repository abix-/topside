//! Research (todo 28) on modforge's rules (modforge tech.rs): the topics
//! are content (`tech_tree`); each group researches its own. At a
//! research station the station panel lists the topics open to the
//! user's group, each with what its next level takes; picking one (the
//! same Craft action as a recipe) pays it from the inventory and
//! researches it. At any other station a recipe a topic unlocks is not
//! listed until its topic is done. Everyone is equal: any person
//! researches the same way.

use std::collections::BTreeMap;

use bevy::prelude::*;
use modforge::crafting::{RecipeDef, RecipeRegistry, StationKind};
use modforge::tech::{Known, TechTree, TopicDef};

use crate::actor::Npc;
use crate::content::PLAYER_FACTION;

/// Every topic (content).
#[derive(Resource)]
pub struct TechTreeRes(pub TechTree);

impl Default for TechTreeRes {
    fn default() -> Self {
        Self(crate::content::tech_tree())
    }
}

/// What each group has researched (saved).
#[derive(Resource, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct Researched(pub BTreeMap<String, Known>);

/// One row of a station's panel.
pub enum Row<'a> {
    Recipe(&'a RecipeDef),
    Topic(&'a TopicDef),
}

/// A person's group: the player's bunker for the player.
pub fn group_of(is_player: bool, npc: Option<&Npc>) -> String {
    if is_player { PLAYER_FACTION.to_string() } else { npc.and_then(|n| n.faction.clone()).unwrap_or_default() }
}

/// The rows a station shows a group, top to bottom: the open topics at a
/// research station, else the recipes not locked and within the group's
/// bench tier (modforge `TechTree::tier`).
pub fn rows<'a>(kind: StationKind, recipes: &'a RecipeRegistry, tree: &'a TechTree, known: &Known) -> Vec<Row<'a>> {
    if kind == StationKind::ResearchStation {
        return tree.open(known).into_iter().map(Row::Topic).collect();
    }
    let tier = tree.tier(known);
    recipes
        .for_station(kind)
        .into_iter()
        .filter(|r| r.tier <= tier && !tree.locked(&r.name, known))
        .map(Row::Recipe)
        .collect()
}

/// Whether the inventory holds a cost.
pub fn affords(inventory: &modforge::item::Inventory, cost: &[(impl AsRef<str>, u32)]) -> bool {
    cost.iter().all(|(item, n)| inventory.count_of(item.as_ref()) >= *n)
}

/// Pay a cost from the inventory, only if it is all there.
pub fn pay(inventory: &mut modforge::item::Inventory, cost: &[(impl AsRef<str>, u32)]) -> bool {
    if !affords(inventory, cost) {
        return false;
    }
    for (item, n) in cost {
        for _ in 0..*n {
            inventory.take_one(item.as_ref());
        }
    }
    true
}
