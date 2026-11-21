//! Use-based skills on everyone (todo 31) on modforge's rules (modforge
//! practice.rs over modforge rpg): crafting, combat and research rise by
//! doing them, and a higher skill does measurably better: a crafter's
//! quality odds lean to better tiers, a fighter's hits land harder, a
//! researcher's topics take less. What is practised is queued by the
//! system that did it and given to the person here, once a step. Every
//! number is Claude's, for the operator to change.

use bevy::prelude::*;

use crate::actor::Persona;

/// A saved part from before the Persona, whose skills are now the
/// person's (actor::fold_old_parts).
#[derive(Component, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Skills(pub modforge::practice::Skills);

/// What was practised this step, by whom: given to them by `learn`.
#[derive(Resource, Default)]
pub struct Practised(pub Vec<(Entity, &'static str, u64)>);

pub const CRAFTING: &str = "crafting";
pub const COMBAT: &str = "combat";
pub const RESEARCH: &str = "research";

/// How far mastery leans the quality odds (modforge `lean_odds`).
pub const CRAFT_LEAN: f32 = 2.0;
/// What full mastery adds to a hit, and takes off a topic's cost.
pub const COMBAT_BONUS: f32 = 0.5;
pub const RESEARCH_SAVING: f32 = 0.5;
/// The experience one craft, one research and one point of damage give.
pub const CRAFT_XP: u64 = 10;
pub const RESEARCH_XP: u64 = 20;
pub const DAMAGE_XP: f32 = 1.0;

/// A person's mastery of a skill, 0 with none.
pub fn mastery(person: Option<&Persona>, skill: &str) -> f32 {
    person.map_or(0.0, |p| p.0.skills.mastery(skill))
}

/// What a hit lands at for a fighter of that skill.
pub fn hit_scale(person: Option<&Persona>) -> f32 {
    1.0 + mastery(person, COMBAT) * COMBAT_BONUS
}

/// A research cost lessened by the researcher's skill, never below one.
pub fn research_cost(cost: Vec<(String, u32)>, person: Option<&Persona>) -> Vec<(String, u32)> {
    let keep = 1.0 - mastery(person, RESEARCH) * RESEARCH_SAVING;
    cost.into_iter().map(|(item, n)| (item, ((n as f32 * keep).ceil() as u32).max(1))).collect()
}

/// Every step: each person given what they practised, in their Persona.
pub fn learn(mut practised: ResMut<Practised>, mut people: Query<&mut Persona>) {
    for (entity, skill, xp) in practised.0.drain(..) {
        if let Ok(mut person) = people.get_mut(entity) {
            person.0.skills.practise(skill, xp);
        }
    }
}
