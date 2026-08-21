//! The actual items, biomes, building types, and monument types of
//! topside. modforge owns the shapes; this file says what
//! concretely exists in this game.

use bevy::math::Vec3;
use bevy::prelude::Resource;
use modforge::biome::{BiomeDef, BiomeRegistry, ScatterSpec};
use modforge::combat::{DamageDef, DamageRegistry, DamageType, Falloff};
use modforge::item::{CombatStats, FoodStats, ItemDef, ItemKind, ItemRegistry};
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

/// The damage registry as an app resource: every hit names a def
/// from here (design.md "How a hit works").
#[derive(Resource)]
pub struct DamageRegistryRes(pub DamageRegistry);

impl Default for DamageRegistryRes {
    fn default() -> Self {
        let mut reg = DamageRegistry::default();
        register_damage(&mut reg);
        Self(reg)
    }
}

/// What each weapon's hit does. Melee has no falloff.
pub fn register_damage(reg: &mut DamageRegistry) {
    let melee = |name: &str, amount, kind| DamageDef {
        name: name.to_string(),
        amount,
        kind,
        knockback: 1.0,
        ignores_armor: false,
        self_scale: 0.5,
        falloff: Falloff::NONE,
    };
    for def in [
        melee("pipe swing", 20.0, DamageType::Blunt),
        melee("hatchet chop", 30.0, DamageType::Slash),
    ] {
        reg.register(def).expect("damage defs must not collide");
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
            combat: Some(CombatStats {
                damage: "pipe swing".to_string(),
                delay: 0.6,
                reach: 2.0,
                pellets: 1,
                spread_degrees: 0.0,
                ammo: None,
            }),
            food: None,
            storage: None,
        },
        ItemDef {
            name: "hatchet".to_string(),
            kind: ItemKind::Tool,
            max_stack: 1,
            quality_siblings: 3,
            combat: Some(CombatStats {
                damage: "hatchet chop".to_string(),
                delay: 0.8,
                reach: 1.8,
                pellets: 1,
                spread_degrees: 0.0,
                ammo: None,
            }),
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
    struct Spec {
        name: &'static str,
        size: BuildingSize,
        columns: (u32, u32),
        rows: (u32, u32),
        floors: (u32, u32),
        basements: (u32, u32),
        width: (f32, f32),
        length: (f32, f32),
        height: (f32, f32),
        windows: u64,
        clutter: u64,
        lights: u64,
        damage: u64,
        carve: u64,
        palette: &'static [Rgb],
    }
    // Columns: name, size, columns, rows, floors up, floors down,
    // room width, room length, floor height, window / clutter /
    // light / damage / carve chances per mille, palette.
    let buildings = [
        // Small: one or two rooms, one floor.
        Spec { name: "gas station", size: Small, columns: (1, 2), rows: (1, 1), floors: (1, 1), basements: (0, 0), width: (5.0, 8.0), length: (5.0, 7.0), height: (2.8, 3.4), windows: 600, clutter: 500, lights: 800, damage: 150, carve: 0, palette: STEEL },
        Spec { name: "convenience store", size: Small, columns: (2, 3), rows: (1, 1), floors: (1, 1), basements: (0, 1), width: (5.0, 7.0), length: (6.0, 9.0), height: (2.8, 3.4), windows: 500, clutter: 700, lights: 800, damage: 150, carve: 0, palette: CONCRETE },
        Spec { name: "water well", size: Small, columns: (1, 1), rows: (1, 1), floors: (1, 1), basements: (0, 0), width: (3.5, 4.5), length: (3.5, 4.5), height: (2.6, 3.2), windows: 0, clutter: 300, lights: 0, damage: 300, carve: 0, palette: CONCRETE },
        Spec { name: "guard tower", size: Small, columns: (1, 1), rows: (1, 1), floors: (3, 5), basements: (0, 0), width: (3.5, 4.0), length: (5.0, 6.0), height: (2.8, 3.2), windows: 800, clutter: 200, lights: 600, damage: 100, carve: 0, palette: WOOD },
        Spec { name: "shack", size: Small, columns: (1, 1), rows: (1, 1), floors: (1, 1), basements: (0, 0), width: (4.0, 6.0), length: (4.0, 6.0), height: (2.5, 3.0), windows: 400, clutter: 500, lights: 800, damage: 350, carve: 0, palette: WOOD },
        Spec { name: "lighthouse", size: Small, columns: (1, 1), rows: (1, 1), floors: (5, 8), basements: (0, 0), width: (4.0, 5.0), length: (5.0, 6.0), height: (2.8, 3.2), windows: 500, clutter: 200, lights: 500, damage: 100, carve: 0, palette: CLEAN },
        // Medium: two to six rooms, one to three floors.
        Spec { name: "warehouse", size: Medium, columns: (2, 3), rows: (1, 2), floors: (1, 1), basements: (0, 0), width: (8.0, 12.0), length: (10.0, 16.0), height: (4.5, 6.0), windows: 300, clutter: 800, lights: 600, damage: 200, carve: 200, palette: STEEL },
        Spec { name: "office building", size: Medium, columns: (2, 3), rows: (1, 2), floors: (2, 4), basements: (0, 1), width: (5.0, 6.0), length: (6.0, 8.0), height: (2.8, 3.4), windows: 800, clutter: 600, lights: 400, damage: 150, carve: 200, palette: CONCRETE },
        Spec { name: "workshop", size: Medium, columns: (2, 3), rows: (1, 1), floors: (1, 1), basements: (0, 0), width: (6.0, 8.0), length: (7.0, 10.0), height: (3.5, 4.5), windows: 400, clutter: 900, lights: 600, damage: 200, carve: 0, palette: STEEL },
        Spec { name: "barracks", size: Medium, columns: (2, 3), rows: (1, 2), floors: (1, 2), basements: (0, 0), width: (5.0, 6.0), length: (7.0, 9.0), height: (2.8, 3.2), windows: 500, clutter: 700, lights: 500, damage: 150, carve: 200, palette: WOOD },
        Spec { name: "lab", size: Medium, columns: (2, 2), rows: (1, 2), floors: (1, 2), basements: (1, 3), width: (5.0, 7.0), length: (6.0, 8.0), height: (2.8, 3.2), windows: 300, clutter: 800, lights: 600, damage: 100, carve: 100, palette: CLEAN },
        Spec { name: "supermarket", size: Medium, columns: (2, 3), rows: (1, 2), floors: (1, 1), basements: (0, 0), width: (10.0, 14.0), length: (10.0, 14.0), height: (3.5, 4.5), windows: 200, clutter: 900, lights: 500, damage: 200, carve: 200, palette: CONCRETE },
        // Large: twenty or more rooms (operator, 2026-08-20: "each
        // with 20+ rooms"), up to ten floors up or down or both
        // (operator: "up to 10 floors up OR down like a bunker",
        // "BOTH up and down"). Minimum grids keep the room count
        // past twenty even after carving.
        Spec { name: "hangar", size: Large, columns: (3, 4), rows: (4, 4), floors: (2, 3), basements: (0, 0), width: (8.0, 12.0), length: (8.0, 12.0), height: (5.0, 7.0), windows: 100, clutter: 600, lights: 200, damage: 150, carve: 100, palette: STEEL },
        Spec { name: "industrial plant", size: Large, columns: (4, 5), rows: (3, 4), floors: (2, 4), basements: (0, 2), width: (6.0, 9.0), length: (6.0, 9.0), height: (4.0, 5.5), windows: 300, clutter: 900, lights: 250, damage: 250, carve: 150, palette: STEEL },
        Spec { name: "silo", size: Large, columns: (3, 4), rows: (4, 4), floors: (1, 2), basements: (4, 10), width: (4.0, 6.0), length: (4.0, 6.0), height: (2.8, 3.2), windows: 100, clutter: 500, lights: 300, damage: 100, carve: 100, palette: CONCRETE },
        Spec { name: "dock building", size: Large, columns: (4, 5), rows: (3, 3), floors: (2, 2), basements: (0, 0), width: (5.0, 7.0), length: (6.0, 9.0), height: (3.5, 4.5), windows: 400, clutter: 700, lights: 200, damage: 300, carve: 150, palette: WOOD },
        Spec { name: "apartment block", size: Large, columns: (4, 5), rows: (3, 4), floors: (3, 10), basements: (0, 1), width: (4.5, 6.0), length: (5.0, 7.0), height: (2.8, 3.2), windows: 700, clutter: 700, lights: 200, damage: 200, carve: 150, palette: CONCRETE },
        Spec { name: "hospital", size: Large, columns: (4, 6), rows: (3, 4), floors: (3, 6), basements: (1, 2), width: (5.0, 7.0), length: (5.0, 7.0), height: (3.0, 3.4), windows: 500, clutter: 800, lights: 250, damage: 150, carve: 100, palette: CLEAN },
        Spec { name: "bunker complex", size: Large, columns: (3, 5), rows: (3, 4), floors: (1, 1), basements: (5, 10), width: (4.5, 6.5), length: (5.0, 7.0), height: (2.6, 3.0), windows: 0, clutter: 700, lights: 400, damage: 50, carve: 100, palette: CONCRETE },
    ]
    .map(|s| BuildingTypeDef {
        name: s.name.to_string(),
        size: s.size,
        columns: s.columns,
        rows: s.rows,
        floors: s.floors,
        basements: s.basements,
        width: s.width,
        length: s.length,
        height: s.height,
        windows: s.windows,
        clutter: s.clutter,
        lights: s.lights,
        damage: s.damage,
        carve: s.carve,
        palette: s.palette.to_vec(),
    });
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
                &[
                    "apartment block",
                    "hospital",
                    "industrial plant",
                    "silo",
                    "dock building",
                    "bunker complex",
                    "hangar",
                ],
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
        assert_eq!(regs.buildings.len(), 19);
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
