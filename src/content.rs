//! The actual items, biomes, building types, and monument types of
//! topside. modforge owns the shapes; this file says what
//! concretely exists in this game.

use bevy::math::Vec3;
use bevy::prelude::Resource;
use modforge::biome::{BiomeDef, BiomeRegistry, ScatterSpec};
use modforge::item::{FoodStats, ItemDef, ItemKind, ItemRegistry};
use modforge::monument::{
    Arrangement, BuildingRegistry, BuildingSize, BuildingSlot, BuildingTypeDef,
    MonumentRegistry, MonumentTypeDef,
};
use modforge::structure::Rgb;

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

/// The building and monument registries as one app resource: every
/// monument in the world is rolled from these.
#[derive(Resource)]
pub struct MonumentRegistryRes {
    pub buildings: BuildingRegistry,
    pub monuments: MonumentRegistry,
}

impl Default for MonumentRegistryRes {
    fn default() -> Self {
        let mut buildings = BuildingRegistry::default();
        register_buildings(&mut buildings);
        let mut monuments = MonumentRegistry::default();
        register_monuments(&mut monuments, &buildings);
        Self {
            buildings,
            monuments,
        }
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
            food: Some(FoodStats {
                hunger: 50.0,
                thirst: 0.0,
            }),
            storage: None,
        },
        ItemDef {
            name: "water bottle".to_string(),
            kind: ItemKind::Food,
            max_stack: 5,
            quality_siblings: 1,
            combat: None,
            food: Some(FoodStats {
                hunger: 0.0,
                thirst: 50.0,
            }),
            storage: None,
        },
        ItemDef {
            name: "storage box".to_string(),
            kind: ItemKind::Deployable,
            max_stack: 1,
            quality_siblings: 1,
            combat: None,
            food: None,
            storage: Some(12),
        },
        ItemDef {
            name: "scrap".to_string(),
            kind: ItemKind::Material,
            max_stack: 20,
            quality_siblings: 1,
            combat: None,
            food: None,
            storage: None,
        },
        ItemDef {
            name: "cloth".to_string(),
            kind: ItemKind::Material,
            max_stack: 20,
            quality_siblings: 1,
            combat: None,
            food: None,
            storage: None,
        },
        ItemDef {
            name: "pipe".to_string(),
            kind: ItemKind::Weapon,
            max_stack: 1,
            quality_siblings: 3,
            combat: None,
            food: None,
            storage: None,
        },
        ItemDef {
            name: "hatchet".to_string(),
            kind: ItemKind::Tool,
            max_stack: 1,
            quality_siblings: 3,
            combat: None,
            food: None,
            storage: None,
        },
    ];
    for item in items {
        reg.register(item).expect("item defs must not collide");
    }
}

const CONCRETE: &[Rgb] = &[[0.45, 0.45, 0.47], [0.5, 0.48, 0.45], [0.38, 0.4, 0.42]];
const WOOD: &[Rgb] = &[[0.45, 0.32, 0.18], [0.55, 0.45, 0.2], [0.35, 0.25, 0.15]];
const STEEL: &[Rgb] = &[[0.3, 0.32, 0.35], [0.25, 0.3, 0.4], [0.5, 0.2, 0.15]];
const CLEAN: &[Rgb] = &[[0.8, 0.82, 0.85], [0.6, 0.7, 0.75], [0.3, 0.5, 0.6]];

/// The building types (design.md "Building types"): small, medium,
/// large, each with the parameter ranges that change on every roll:
/// columns and rows of rooms per floor, floors, room footprint,
/// height, window, clutter, and light chances, palette. Floors top
/// out at two until multi-flight stairwells land.
pub fn register_buildings(reg: &mut BuildingRegistry) {
    use BuildingSize::*;
    #[allow(clippy::too_many_arguments)]
    fn def(
        name: &str,
        size: BuildingSize,
        columns: (u32, u32),
        rows: (u32, u32),
        floors: (u32, u32),
        width: (f32, f32),
        length: (f32, f32),
        height: f32,
        windows: u64,
        clutter: u64,
        lights: u64,
        palette: &[Rgb],
    ) -> BuildingTypeDef {
        BuildingTypeDef {
            name: name.to_string(),
            size,
            columns,
            rows,
            floors,
            width,
            length,
            height,
            windows,
            clutter,
            lights,
            palette: palette.to_vec(),
        }
    }
    let buildings = [
        // Small: one room, one floor.
        def("gas station", Small, (1, 2), (1, 1), (1, 1), (5.0, 8.0), (5.0, 7.0), 3.0, 600, 500, 800, STEEL),
        def("convenience store", Small, (2, 3), (1, 1), (1, 1), (5.0, 7.0), (6.0, 9.0), 3.0, 500, 700, 800, CONCRETE),
        def("water well", Small, (1, 1), (1, 1), (1, 1), (3.5, 4.5), (3.5, 4.5), 3.0, 0, 300, 0, CONCRETE),
        def("guard tower", Small, (1, 1), (1, 1), (2, 2), (3.5, 4.0), (5.0, 6.0), 3.0, 800, 200, 800, WOOD),
        def("shack", Small, (1, 1), (1, 1), (1, 1), (4.0, 6.0), (4.0, 6.0), 2.8, 400, 500, 800, WOOD),
        def("lighthouse", Small, (1, 1), (1, 1), (2, 2), (4.0, 5.0), (5.0, 6.0), 3.0, 500, 200, 800, CLEAN),
        // Medium: two to four rooms, one or two floors.
        def("warehouse", Medium, (2, 3), (1, 1), (1, 1), (8.0, 12.0), (10.0, 16.0), 5.0, 300, 800, 600, STEEL),
        def("office building", Medium, (2, 2), (1, 2), (2, 2), (5.0, 6.0), (6.0, 8.0), 3.0, 800, 600, 400, CONCRETE),
        def("workshop", Medium, (2, 3), (1, 1), (1, 1), (6.0, 8.0), (7.0, 10.0), 4.0, 400, 900, 600, STEEL),
        def("barracks", Medium, (2, 2), (1, 2), (1, 1), (5.0, 6.0), (7.0, 9.0), 3.0, 500, 700, 500, WOOD),
        def("lab", Medium, (2, 2), (1, 2), (1, 2), (5.0, 7.0), (6.0, 8.0), 3.0, 300, 800, 600, CLEAN),
        def("supermarket", Medium, (2, 3), (1, 1), (1, 1), (10.0, 14.0), (10.0, 14.0), 4.0, 200, 900, 500, CONCRETE),
        // Large: twenty or more rooms on two floors (operator,
        // 2026-08-20: "each with 20+ rooms").
        def("hangar", Large, (3, 4), (4, 4), (2, 2), (8.0, 12.0), (8.0, 12.0), 6.0, 100, 600, 200, STEEL),
        def("industrial plant", Large, (4, 5), (3, 4), (2, 2), (6.0, 9.0), (6.0, 9.0), 5.0, 300, 900, 250, STEEL),
        def("silo", Large, (3, 4), (4, 4), (2, 2), (4.0, 6.0), (4.0, 6.0), 3.0, 100, 500, 200, CONCRETE),
        def("dock building", Large, (4, 5), (3, 3), (2, 2), (5.0, 7.0), (6.0, 9.0), 4.0, 400, 700, 200, WOOD),
        def("apartment block", Large, (4, 5), (3, 4), (2, 2), (4.5, 6.0), (5.0, 7.0), 3.0, 700, 700, 200, CONCRETE),
        def("hospital", Large, (4, 6), (3, 4), (2, 2), (5.0, 7.0), (5.0, 7.0), 3.2, 500, 800, 250, CLEAN),
    ];
    for building in buildings {
        reg.register(building).expect("building type defs must not collide");
    }
}

/// The monument types (design.md "Monument types", Rust's adapted):
/// which building types each picks from, how many, how they sit,
/// how dangerous, whether a gated room waits at the back, and the
/// word that ends its rolled name.
pub fn register_monuments(reg: &mut MonumentRegistry, buildings: &BuildingRegistry) {
    use Arrangement::*;
    reg.set_name_words(
        [
            "Ash", "Rust", "Hollow", "Grey", "Salt", "Cinder", "Dust", "Black", "Cold", "Broken",
            "Iron", "Sour", "Dead", "Low", "Far",
        ]
        .map(String::from)
        .to_vec(),
        [
            "fall", "ridge", "water", "field", "mark", "reach", "pass", "well", "brook", "gate",
            "fork", "hill", "marsh", "bend", "row",
        ]
        .map(String::from)
        .to_vec(),
    );
    let small = ["gas station", "convenience store", "water well", "guard tower", "shack", "lighthouse"];
    let medium = ["warehouse", "office building", "workshop", "barracks", "lab", "supermarket"];
    let small_or_medium = ["gas station", "shack", "guard tower", "warehouse", "workshop", "office building"];
    let slot = |choices: &[&str], min, max| BuildingSlot {
        choices: choices.iter().map(|c| c.to_string()).collect(),
        min,
        max,
    };
    let def = |name: &str, slots: Vec<BuildingSlot>, arrangement, danger, gated, suffix: &str| {
        MonumentTypeDef {
            name: name.to_string(),
            slots,
            arrangement,
            danger,
            gated,
            suffix: suffix.to_string(),
        }
    };
    let monuments = [
        def("roadside stop", vec![slot(&small, 1, 2)], AlongRoad, 1, false, "Stop"),
        def(
            "airfield",
            vec![slot(&["hangar"], 1, 1), slot(&small_or_medium, 1, 2)],
            AlongRoad,
            3,
            true,
            "Airfield",
        ),
        def(
            "power plant",
            vec![slot(&["industrial plant"], 1, 1), slot(&medium, 1, 3)],
            Clustered,
            4,
            true,
            "Plant",
        ),
        def(
            "train yard",
            vec![slot(&["warehouse"], 1, 2), slot(&small, 1, 2)],
            AlongRoad,
            2,
            false,
            "Yard",
        ),
        def(
            "military outpost",
            vec![slot(&["barracks"], 1, 1), slot(&["guard tower"], 1, 2)],
            AroundYard,
            3,
            true,
            "Outpost",
        ),
        def(
            "research station",
            vec![slot(&["lab"], 1, 1), slot(&medium, 1, 2)],
            AroundYard,
            3,
            true,
            "Station",
        ),
        def(
            "harbor",
            vec![slot(&["dock building"], 1, 1), slot(&["warehouse"], 1, 2)],
            AlongRoad,
            2,
            false,
            "Harbor",
        ),
        def("junkyard", vec![slot(&["shack", "workshop"], 2, 3)], Clustered, 1, false, "Junkyard"),
        def(
            "sewer",
            vec![slot(&["silo"], 1, 1), slot(&["shack"], 1, 1)],
            Clustered,
            3,
            true,
            "Sewer",
        ),
        def(
            "launch site",
            vec![
                slot(&["hangar"], 1, 1),
                slot(&["industrial plant"], 1, 1),
                slot(&medium, 1, 2),
            ],
            AroundYard,
            5,
            true,
            "Site",
        ),
        // The big one (operator, 2026-08-20: "a monument with 50
        // buildings, each with 20+ rooms"): fifty large buildings
        // on city blocks.
        def(
            "ruined city",
            vec![slot(
                &["apartment block", "hospital", "industrial plant", "silo", "dock building"],
                50,
                50,
            )],
            Clustered,
            4,
            true,
            "City",
        ),
    ];
    for monument in monuments {
        reg.register(monument, buildings)
            .expect("monument type defs must not collide or name unknown buildings");
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
        assert_eq!(reg.len(), 7);
        assert!(reg.def("canned food").is_some());
        assert!(reg.def("hatchet").is_some());
    }

    #[test]
    fn every_monument_type_rolls_legal_monuments() {
        let regs = MonumentRegistryRes::default();
        assert_eq!(regs.buildings.len(), 18);
        assert_eq!(regs.monuments.len(), 11);
        let names: Vec<String> = regs.monuments.names().map(String::from).collect();
        for name in names {
            for seed in 0..5u64 {
                let rolled = regs.monuments.roll(&name, &regs.buildings, seed).unwrap();
                modforge::monument::validate_monument(&rolled).unwrap();
            }
        }
        // The city: fifty buildings, every one twenty rooms or more.
        let city = regs.monuments.roll("ruined city", &regs.buildings, 1).unwrap();
        assert_eq!(city.members.len(), 50);
        let smallest = city.members.iter().map(|m| m.structure.rooms.len()).min().unwrap();
        assert!(smallest >= 20, "smallest city building has {smallest} rooms");
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
