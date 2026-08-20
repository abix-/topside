//! The actual items and biomes of topside. modforge owns the
//! shapes; this file says what concretely exists in this game.

use bevy::math::Vec3;
use bevy::prelude::Resource;
use modforge::biome::{BiomeDef, BiomeRegistry, ScatterSpec};
use modforge::item::{ItemDef, ItemKind, ItemRegistry};

/// The item registry as an app resource. Every production stack is
/// created from a def looked up here.
#[derive(Resource)]
pub struct ItemRegistryRes(pub ItemRegistry);

impl Default for ItemRegistryRes {
    fn default() -> Self {
        let mut reg = ItemRegistry::default();
        register_items(&mut reg);
        Self(reg)
    }
}

pub fn register_items(reg: &mut ItemRegistry) {
    let items = [
        ItemDef {
            name: "canned food".to_string(),
            kind: ItemKind::Food,
            max_stack: 10,
            quality_siblings: 1,
            combat: None,
        },
        ItemDef {
            name: "water bottle".to_string(),
            kind: ItemKind::Food,
            max_stack: 5,
            quality_siblings: 1,
            combat: None,
        },
        ItemDef {
            name: "scrap".to_string(),
            kind: ItemKind::Material,
            max_stack: 20,
            quality_siblings: 1,
            combat: None,
        },
        ItemDef {
            name: "cloth".to_string(),
            kind: ItemKind::Material,
            max_stack: 20,
            quality_siblings: 1,
            combat: None,
        },
        ItemDef {
            name: "pipe".to_string(),
            kind: ItemKind::Weapon,
            max_stack: 1,
            quality_siblings: 3,
            combat: None,
        },
        ItemDef {
            name: "hatchet".to_string(),
            kind: ItemKind::Tool,
            max_stack: 1,
            quality_siblings: 3,
            combat: None,
        },
    ];
    for item in items {
        reg.register(item).expect("item defs must not collide");
    }
}

pub fn register_biomes(reg: &mut BiomeRegistry) {
    let biomes = [
        BiomeDef {
            name: "forest".to_string(),
            ground: [0.22, 0.38, 0.18],
            scatter: vec![
                ScatterSpec {
                    size: Vec3::new(0.6, 5.0, 0.6),
                    color: [0.3, 0.22, 0.12],
                    density: 0.04,
                },
                ScatterSpec {
                    size: Vec3::new(0.8, 0.5, 0.8),
                    color: [0.4, 0.4, 0.35],
                    density: 0.01,
                },
            ],
            weather: vec!["rain".to_string(), "fog".to_string()],
            monuments: vec!["ranger station".to_string(), "campsite".to_string()],
            npcs: vec!["scavenger".to_string(), "hunter".to_string()],
            wildlife: vec!["deer".to_string(), "wolf".to_string()],
            harvest: vec!["scrap".to_string(), "cloth".to_string()],
        },
        BiomeDef {
            name: "wasteland".to_string(),
            ground: [0.55, 0.48, 0.35],
            scatter: vec![
                ScatterSpec {
                    size: Vec3::new(1.2, 0.6, 0.8),
                    color: [0.5, 0.42, 0.32],
                    density: 0.005,
                },
            ],
            weather: vec!["dust storm".to_string(), "clear".to_string()],
            monuments: vec![
                "gas station".to_string(),
                "wrecked convoy".to_string(),
            ],
            npcs: vec!["raider".to_string(), "wanderer".to_string()],
            wildlife: vec!["rat".to_string()],
            harvest: vec!["scrap".to_string()],
        },
        BiomeDef {
            name: "ruins".to_string(),
            ground: [0.4, 0.4, 0.42],
            scatter: vec![
                ScatterSpec {
                    size: Vec3::new(2.0, 1.5, 1.5),
                    color: [0.45, 0.43, 0.4],
                    density: 0.008,
                },
            ],
            weather: vec!["rain".to_string(), "fog".to_string(), "clear".to_string()],
            monuments: vec![
                "warehouse".to_string(),
                "apartment block".to_string(),
                "hospital".to_string(),
            ],
            npcs: vec![
                "raider".to_string(),
                "scavenger".to_string(),
                "militia".to_string(),
            ],
            wildlife: vec!["rat".to_string(), "crow".to_string()],
            harvest: vec!["scrap".to_string(), "pipe".to_string(), "cloth".to_string()],
        },
    ];
    for biome in biomes {
        reg.register(biome).expect("biome defs must not collide");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_items_register() {
        let mut reg = ItemRegistry::default();
        register_items(&mut reg);
        assert_eq!(reg.len(), 6);
        assert!(reg.def("canned food").is_some());
        assert!(reg.def("hatchet").is_some());
    }

    #[test]
    fn all_biomes_register() {
        let mut reg = BiomeRegistry::default();
        register_biomes(&mut reg);
        assert_eq!(reg.len(), 3);
        assert!(reg.def("forest").is_some());
        assert!(reg.def("ruins").is_some());
    }
}
