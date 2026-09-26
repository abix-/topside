//! The actual items, biomes, building types, and monument types of
//! topside. modforge owns the shapes; this file says what
//! concretely exists in this game.

use bevy::math::{Vec2, Vec3};
use bevy::prelude::Resource;
use modforge::worldgen::{BiomeRule, Generator, WorldDef};
use modforge::ground::{ClimateRule, Ground, GroundDef, GroundGen};
use modforge::places::{Places, SiteKind};
use modforge::world2d::{Clearing, World2d};
use modforge::actor::{ActorDef, ActorRegistry, Behaviour};
use modforge::biome::{BiomeDef, BiomeRegistry, ScatterDef};
use modforge::combat::{DamageDef, DamageRegistry, DamageType, Falloff};
use modforge::faction::{FactionDef, FactionKind, FactionRegistry};
use modforge::memory::GoodFor;
use modforge::survival::Need;
use modforge::item::{Armor, CombatStats, EquipSlot, FoodStats, ItemDef, ItemKind, ItemRegistry, Note};
use modforge::monument::Roll;
use modforge::monument::{
    Arrangement, BuildingRegistry, BuildingSize, BuildingSlot, BuildingTypeDef,
    MonumentRegistry, MonumentTypeDef, PropDef,
};
use modforge::crafting::{RecipeDef, RecipeRegistry, StationKind};
use modforge::loot::{LootEntry, LootTable};
use modforge::storm::StormDef;
use modforge::structure::{
    CONCRETE_FLOOR, CONCRETE_WALL, LightDef, Opening, Rgb, RoomDef, Side, SolidDef, StructureDef,
};

/// What lies on the starting bunker's floor (design.md: three days of
/// food; a can is half a day of hunger and a bottle half a day of
/// thirst, so six of each), plus the first weapons and a vest: item,
/// count, salt, where in the bunker's plan (x, z), colour.
pub const STARTING_ITEMS: [(&str, u32, u64, [f32; 2], Rgb); 9] = [
    // Enough cloth for the bench's first bandage.
    ("cloth", 2, 14, [-1.6, 2.0], [0.75, 0.7, 0.6]),
    ("canned food", 6, 1, [-1.0, 2.0], [0.2, 0.6, 0.2]),
    ("water bottle", 5, 3, [-1.0, 2.6], [0.3, 0.5, 0.8]),
    ("water bottle", 1, 4, [-1.6, 2.6], [0.3, 0.5, 0.8]),
    ("scrap", 3, 2, [1.0, 2.0], [0.5, 0.5, 0.5]),
    ("pipe shotgun", 1, 9, [1.6, 2.0], [0.3, 0.3, 0.3]),
    ("shotgun shell", 8, 10, [1.6, 2.6], [0.8, 0.3, 0.2]),
    ("pipe", 1, 11, [1.0, 2.6], [0.4, 0.4, 0.45]),
    ("road sign vest", 1, 12, [2.2, 2.0], [0.2, 0.5, 0.2]),
];

/// Where the crafting bench is used in the starting bunker's plan
/// (x, z): its right end, clear of the note on it.
pub const BENCH_AT: [f32; 2] = [2.8, 3.5];

/// Where the note lies in the starting bunker's plan (x, z): on the
/// bench, the first question (design.md "The first hour").
pub const STARTING_NOTE_AT: [f32; 2] = [2.2, 3.5];

/// Every recipe, as data (modforge crafting).
#[derive(Resource)]
pub struct RecipeRegistryRes(pub RecipeRegistry);

impl Default for RecipeRegistryRes {
    fn default() -> Self {
        let mut reg = RecipeRegistry::default();
        // The bench's first recipes (todo 10): cloth into a bandage,
        // scrap into a pipe. Tuning.
        let bench = |name: &str, inputs: &[(&str, u32)], output: (&str, u32)| RecipeDef {
            name: name.to_string(),
            inputs: inputs.iter().map(|(i, n)| (i.to_string(), *n)).collect(),
            output: (output.0.to_string(), output.1),
            station: StationKind::CraftingBench,
            craft_time: 0.0,
        };
        for def in [
            bench("bandage", &[("cloth", 2)], ("bandage", 1)),
            bench("pipe", &[("scrap", 3)], ("pipe", 1)),
        ] {
            reg.register(def).expect("recipe registered once");
        }
        Self(reg)
    }
}

/// What a monument's box can hold (design.md "Loot tables scale with
/// danger"): common junk anywhere, weapons and the vest only where it
/// is dangerous. Tuning.
pub fn loot_table() -> LootTable {
    let e = |item: &str, count: (u32, u32), weight, min_danger| LootEntry {
        item: item.to_string(),
        count,
        weight,
        min_danger,
    };
    LootTable {
        entries: vec![
            e("scrap", (1, 4), 10, 0),
            e("cloth", (1, 3), 8, 0),
            e("canned food", (1, 2), 6, 0),
            e("water bottle", (1, 2), 6, 0),
            e("shotgun shell", (2, 5), 4, 1),
            e("bandage", (1, 1), 3, 1),
            e("pipe", (1, 1), 2, 1),
            e("hatchet", (1, 1), 2, 2),
            e("road sign vest", (1, 1), 1, 3),
            e("pipe shotgun", (1, 1), 1, 3),
        ],
        picks: (1, 2),
        picks_per_danger: 1,
    }
}

/// One game day in real seconds (the 3D game's Valheim-scale pacing;
/// tune later). A full stomach lasts one, a full canteen half of one,
/// rest one awake.
pub const GAME_DAY_SECS: f32 = 1200.0;

/// When storms come (design.md "The storm", MISERY's emission): one a
/// game day, a minute of warning, twenty seconds of storm. Tuning.
pub const STORM: StormDef = StormDef {
    every_secs: GAME_DAY_SECS,
    warning_secs: 60.0,
    storm_secs: 20.0,
};

/// Who waits outside the starting bunker's door and where in its plan
/// (x, z): a scavenger 6 m past the door (the 3D game's first fight).
pub const DOOR_ENEMY: (&str, [f32; 2]) = ("scavenger", [0.0, -10.0]);

/// Which actor def stands at a monument's NPC spot of this danger
/// (todo 11q: modforge should decide this; until then it is content).
pub fn def_for_danger(danger: u32) -> &'static str {
    if danger < 3 { "scavenger" } else { "raider" }
}

/// The starting bunker (design.md "The bunker"): one room with exactly
/// a bed, a crafting bench, and the food crate, door north toward
/// topside.
pub fn starting_bunker() -> StructureDef {
    StructureDef {
        name: "the starting bunker".to_string(),
        wall_color: CONCRETE_WALL,
        floor_color: CONCRETE_FLOOR,
        rooms: vec![RoomDef {
            origin: Vec3::ZERO,
            interior: Vec3::new(6.0, 3.0, 8.0),
            wall_thickness: 0.2,
            openings: vec![Opening {
                side: Side::North,
                offset: 0.0,
                width: 1.2,
                sill: 0.0,
                door: true,
            }],
            floor: true,
            ceiling: true,
        }],
        stairs: vec![],
        furniture: vec![
            // The bed.
            SolidDef {
                center: Vec3::new(-2.4, 0.25, 3.0),
                size: Vec3::new(1.0, 0.5, 2.0),
                color: [0.30, 0.35, 0.55],
            },
            // The crafting bench.
            SolidDef {
                center: Vec3::new(2.2, 0.45, 3.5),
                size: Vec3::new(1.6, 0.9, 0.7),
                color: [0.45, 0.32, 0.18],
            },
            // The food crate.
            SolidDef {
                center: Vec3::new(2.5, 0.35, -3.3),
                size: Vec3::new(0.7, 0.7, 0.7),
                color: [0.55, 0.45, 0.20],
            },
        ],
        lights: vec![LightDef {
            position: Vec3::new(0.0, 2.7, 0.0),
            color: [1.0, 0.9, 0.7],
            intensity: 400_000.0,
        }],
        parts: vec![],
    }
}

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

/// The biome registry as an app resource: worldgen paints cells from
/// these and places monuments where a biome allows them.
#[derive(Resource)]
pub struct BiomeRegistryRes(pub BiomeRegistry);

impl Default for BiomeRegistryRes {
    fn default() -> Self {
        let mut reg = BiomeRegistry::default();
        register_biomes(&mut reg);
        Self(reg)
    }
}

/// The realities that can be rolled, by name (design.md "The
/// generators we want"; operator 2026-08-21: "call the first one
/// topside and this one labyrinth"). `--world <name>` picks one
/// until the storm rolls it.
pub const WORLDS: [&str; 2] = ["topside", "labyrinth"];

/// The world to roll for a reality name, or the names it knows.
pub fn world_def(name: &str) -> Result<WorldDef, String> {
    match name {
        "topside" => Ok(topside_def()),
        "labyrinth" => Ok(labyrinth_def()),
        other => Err(format!("unknown world '{other}'; one of {WORLDS:?}")),
    }
}

/// The Labyrinth (design.md): the same size as topside, flat, a maze
/// of 10 m corridors (operator, 2026-08-21: "i want the corridor to
/// be 10m") with walls as high as they are wide, the bunker and its
/// house in the clearing the maze grows out of, and only the minor
/// sites, which fit in a corridor.
pub fn labyrinth_def() -> WorldDef {
    let mut def = topside_def();
    def.generator = Generator::Labyrinth {
        corridor: 10.0,
        wall_thickness: 1.0,
    };
    def.water_level = 0.0;
    def.monuments = vec![
        ("wreck".to_string(), 150),
        ("campsite".to_string(), 90),
        ("supply drop".to_string(), 45),
        ("body".to_string(), 90),
    ];
    // Nothing in the maze stands over its walls yet (the gate tower
    // is row 11v), so no landmark can be in sight from the door.
    def.landmarks = 0;
    // The maze grows out of one clearing; a second clearing would be a
    // maze of its own, cut off from the first. No other bunkers here yet.
    def.bunkers = Vec::new();
    def
}

/// The world to roll (design.md "The world"): two kilometres
/// square, four metre cells, forty metres of relief, the bunker at
/// the origin on flat ground with the starting house beside it.
/// Wasteland in the dry lowlands, forest in the wet ones, ruins on
/// the high ground.
pub fn topside_def() -> WorldDef {
    WorldDef {
        // Rust's default map (operator, 2026-08-21: "i want 3500 m
        // like rust").
        size: 3500.0,
        cell: 4.0,
        // Rust-scale relief: sixty metres of base terrain flattened
        // into plains and valleys, ridged ranges up to ninety more,
        // plateaus in eight metre steps.
        // Operator (2026-08-21): "mountain high up to 400". A hundred
        // metres of base terrain pressed into plains and valleys,
        // ridges up to four hundred more, plateaus in ten metre steps.
        height_scale: 100.0,
        feature_size: 500.0,
        generator: Generator::Relief {
            flatness: 2.2,
            mountain_height: 400.0,
            plateau_step: 10.0,
        },
        water_level: 0.15,
        biome_rules: vec![
            BiomeRule {
                biome: "ruins".to_string(),
                height: (0.6, 1.0),
                moisture: (0.0, 1.0),
            },
            BiomeRule {
                biome: "forest".to_string(),
                height: (0.0, 1.0),
                moisture: (0.5, 1.0),
            },
            BiomeRule {
                biome: "wasteland".to_string(),
                height: (0.0, 1.0),
                moisture: (0.0, 1.0),
            },
        ],
        // Counts scale with the area (three times the 2 km world).
        monuments: vec![
            ("roadside stop".to_string(), 18),
            ("airfield".to_string(), 2),
            ("train yard".to_string(), 2),
            ("launch site".to_string(), 1),
            ("research station".to_string(), 3),
            ("military outpost".to_string(), 5),
            ("power plant".to_string(), 2),
            ("sewer".to_string(), 2),
            ("harbor".to_string(), 2),
            ("ruined city".to_string(), 2),
            // Minor sites: one within 150 m of almost anywhere.
            ("wreck".to_string(), 150),
            ("campsite".to_string(), 90),
            ("supply drop".to_string(), 45),
            ("body".to_string(), 90),
        ],
        // The door opens on one big triangle: the launch site's
        // gantry in sight from the bunker (design.md "The first
        // hour").
        landmarks: 1,
        bunker: Vec2::ZERO,
        bunker_clear: 50.0,
        bunkers: OTHER_BUNKERS.to_vec(),
    }
}

/// The flat ground of the 2D world (design.md "2D world generation"):
/// lakes from a hidden elevation, cliff lines with gaps along its
/// contours, beaches next to water, and three biomes by climate: forest
/// where it is wet, ruins where it is dry and warm, wasteland elsewhere.
/// Tuning.
pub fn ground_def() -> GroundDef {
    GroundDef {
        feature_size: 500.0,
        water_level: -0.45,
        cliff_every: 0.3,
        cliff_gaps: 0.35,
        climate_size: 900.0,
        climate: vec![
            ClimateRule {
                biome: "forest".to_string(),
                moisture: (0.55, 1.0),
                temperature: (0.0, 1.0),
                land: Ground::ForestFloor,
                patches: Ground::Grass,
            },
            ClimateRule {
                biome: "ruins".to_string(),
                moisture: (0.0, 0.55),
                temperature: (0.6, 1.0),
                land: Ground::Rubble,
                patches: Ground::Dirt,
            },
            ClimateRule {
                biome: "wasteland".to_string(),
                moisture: (0.0, 1.0),
                temperature: (0.0, 1.0),
                land: Ground::Dirt,
                patches: Ground::Grass,
            },
        ],
        beach: 0.04,
        patch_size: 80.0,
        chunks_across: None,
    }
}

/// Which 2D world to make: the small world to confirm everything works
/// on (design.md "The small world first"), or the big one.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldKind {
    Small,
    Big,
}

impl WorldKind {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "small" => Some(Self::Small),
            "big" => Some(Self::Big),
            _ => None,
        }
    }

    /// Where the other bunkers stand.
    pub fn other_bunkers(self) -> Vec<Vec2> {
        match self {
            Self::Small => SMALL_OTHER_BUNKERS.to_vec(),
            Self::Big => OTHER_BUNKERS.to_vec(),
        }
    }
}

/// The small world (design.md "The small world first"): 32 by 32 chunks,
/// flat grass, no water, no cliffs, nothing scattered.
pub fn small_ground_def() -> GroundDef {
    GroundDef {
        feature_size: 500.0,
        water_level: -2.0,
        cliff_every: 0.0,
        cliff_gaps: 0.0,
        climate_size: 900.0,
        climate: vec![ClimateRule {
            biome: "grassland".to_string(),
            moisture: (0.0, 1.0),
            temperature: (0.0, 1.0),
            land: Ground::Grass,
            patches: Ground::Grass,
        }],
        beach: 0.0,
        patch_size: 80.0,
        chunks_across: Some(32),
    }
}

/// The small world's other bunkers (operator, 2026-09-25: "lets do 5
/// bunkers. each with 5 people"), round the player's, clear of the sites.
pub const SMALL_OTHER_BUNKERS: [Vec2; 5] = [
    Vec2::new(300.0, 0.0),
    Vec2::new(-300.0, 0.0),
    Vec2::new(0.0, 320.0),
    Vec2::new(0.0, -300.0),
    Vec2::new(300.0, -300.0),
];

/// The small world's topside people, placed by world generation on open
/// land (operator, 2026-09-25: "another 25 nuetral topside creatures. and
/// 25 hostile topside creatures"): each actor def and how many.
pub const SMALL_TOPSIDE_PEOPLE: [(&str, u32); 3] = [("wanderer", 25), ("scavenger", 13), ("raider", 12)];

/// The small world's sites: one of each kind, each at a fixed place round
/// the bunker, clear of both bunkers' ground.
pub const SMALL_SITES: [(&str, [f32; 2]); 5] = [
    ("roadside stop", [0.0, 180.0]),
    ("wreck", [-150.0, 0.0]),
    ("campsite", [0.0, -150.0]),
    ("supply drop", [-120.0, 120.0]),
    ("body", [120.0, -120.0]),
];

/// How each ground kind is drawn. Tuning.
pub fn ground_color(ground: Ground) -> Rgb {
    match ground {
        Ground::Grass => [0.30, 0.42, 0.22],
        Ground::Dirt => [0.50, 0.42, 0.30],
        Ground::Sand => [0.72, 0.66, 0.48],
        Ground::ForestFloor => [0.20, 0.32, 0.16],
        Ground::Rubble => [0.42, 0.41, 0.40],
        Ground::Road => [0.22, 0.21, 0.20],
        Ground::Water => [0.20, 0.30, 0.45],
        Ground::Cliff => [0.28, 0.26, 0.24],
        Ground::OutOfMap => [0.0, 0.0, 0.0],
    }
}

/// How far round a bunker the ground is cleared (design.md "A valid
/// world": every bunker stands on land). Tuning.
pub const BUNKER_CLEAR: f32 = 50.0;

/// The kinds of site in the 2D world, biggest first (an earlier kind keeps
/// a later one away), each as common as the old world's count for its
/// 3500 m square: the chance a cell of its spacing holds one.
pub fn site_kinds(monuments: &MonumentRegistry) -> Vec<SiteKind> {
    let area = 3500.0f32 * 3500.0;
    let mut kinds: Vec<(f32, SiteKind)> = topside_def()
        .monuments
        .into_iter()
        .filter_map(|(name, count)| {
            let spacing = monuments.def(&name)?.spacing;
            let chance = (count as f32 * spacing * spacing / area).min(1.0);
            Some((spacing, SiteKind { monument: name, chance }))
        })
        .collect();
    kinds.sort_by(|a, b| b.0.total_cmp(&a.0));
    kinds.into_iter().map(|(_, k)| k).collect()
}

/// Where the player's bunker stands.
pub const BUNKER_AT: Vec2 = Vec2::ZERO;

/// The world a seed gives (design.md "2D world generation"): the ground,
/// the sites (rolled in the big world, placed by hand in the small one),
/// and every bunker on its cleared ground. The one way a world is made,
/// for the game and for the tests.
pub fn roll_world2d(
    kind: WorldKind,
    seed: u64,
    biomes: &BiomeRegistry,
    monuments: &MonumentRegistry,
) -> Result<World2d, String> {
    let def = match kind {
        WorldKind::Small => small_ground_def(),
        WorldKind::Big => ground_def(),
    };
    let ground = GroundGen::new(def, seed, biomes)?;
    let clearings: Vec<Clearing> = std::iter::once(BUNKER_AT)
        .chain(kind.other_bunkers())
        .map(|at| Clearing { at, radius: BUNKER_CLEAR })
        .collect();
    let keep_clear = clearings.iter().map(|c| (c.at, c.radius)).collect();
    let places = match kind {
        WorldKind::Small => Places::new(Vec::new(), keep_clear)
            .with_fixed(SMALL_SITES.iter().map(|(m, [x, y])| (m.to_string(), Vec2::new(*x, *y))).collect()),
        WorldKind::Big => Places::new(site_kinds(monuments), keep_clear),
    };
    let people = match kind {
        WorldKind::Small => SMALL_TOPSIDE_PEOPLE.iter().map(|(d, n)| (d.to_string(), *n)).collect(),
        WorldKind::Big => Vec::new(),
    };
    Ok(World2d::new(ground, places, clearings).with_people(people))
}

/// The actor registry as an app resource: every NPC spawned through
/// `spawn_actor` is one of these defs.
#[derive(Resource)]
pub struct ActorRegistryRes(pub ActorRegistry);

impl Default for ActorRegistryRes {
    fn default() -> Self {
        let mut reg = ActorRegistry::default();
        register_actors(&mut reg);
        Self(reg)
    }
}

/// The faction registry as an app resource (design.md "Factions and
/// relationships"): the player's, the hostile ones, the builders.
#[derive(Resource)]
pub struct FactionRegistryRes(pub FactionRegistry);

impl Default for FactionRegistryRes {
    fn default() -> Self {
        let mut reg = FactionRegistry::default();
        register_factions(&mut reg);
        Self(reg)
    }
}

/// The player's faction name; the player entity carries no `Npc`, so
/// the AI names it by this.
pub const PLAYER_FACTION: &str = "player";

/// The topside hostiles (design.md "Bunker people and topside people"):
/// wild animals and raiders, part of the topside world, hostile to
/// everyone. Not a bunker; the faction table is how hostility is read.
pub const TOPSIDE_HOSTILES: &str = "topside hostiles";

/// Where the other bunkers stand (design.md: every bunker is its own
/// faction), the same in every world the storm rolls. Tuning.
pub const OTHER_BUNKERS: [Vec2; 4] = [
    Vec2::new(300.0, 300.0),
    Vec2::new(-300.0, 300.0),
    Vec2::new(-300.0, -300.0),
    Vec2::new(300.0, -300.0),
];

/// How many bunker people live in each other bunker. Tuning.
pub const BUNKER_PEOPLE: usize = 5;

/// Where the bunker people stand in the bunker's plan (x, z), one each.
pub const BUNKER_PEOPLE_AT: [[f32; 2]; BUNKER_PEOPLE] =
    [[-1.5, 0.0], [0.0, 0.0], [1.5, 0.0], [-1.0, 1.5], [1.0, 1.5]];

/// A bunker's store (design.md: factions have real stores): one box in
/// the bunker's plan (x, z) and what is in it. Tuning.
pub const BUNKER_STORE_AT: [f32; 2] = [-2.0, -3.0];
pub const BUNKER_STORE: [(&str, u32); 2] = [("canned food", 10), ("water bottle", 10)];

/// A bunker is short when its store holds less than this many days of
/// food or water for everyone living there: the starting bunker's three
/// days (design.md "The bunker"). Tuning.
pub const BUNKER_SHORT_DAYS: f32 = 3.0;

/// The faction of the other bunker at `OTHER_BUNKERS[i]`.
pub fn bunker_faction(i: usize) -> String {
    format!("bunker {}", i + 1)
}

/// Every bunker is its own faction: the player's, and one per other
/// bunker, neutral to each other at the start (what happens between
/// them changes that, todo 11t). The topside hostiles are hostile to
/// everyone.
pub fn register_factions(reg: &mut FactionRegistry) {
    // Enough for the other bunkers of either world.
    let most = OTHER_BUNKERS.len().max(SMALL_OTHER_BUNKERS.len());
    let bunkers = (0..most).map(|i| (bunker_faction(i), FactionKind::Builder));
    for (name, kind) in [
        (PLAYER_FACTION.to_string(), FactionKind::Player),
        (TOPSIDE_HOSTILES.to_string(), FactionKind::Hostile),
    ]
    .into_iter()
    .chain(bunkers)
    {
        reg.register(FactionDef { name, kind }).expect("faction registered once");
    }
}

/// The people: the bunker people (their bunker's faction is given at
/// spawn), the topside hostiles found at monuments, and the wanderer,
/// a topside neutral who belongs to no side.
pub fn register_actors(reg: &mut ActorRegistry) {
    let person = |name: &str, faction: Option<&str>, behaviour, max_health, weapon: &str| {
        ActorDef {
            name: name.to_string(),
            faction: faction.map(String::from),
            behaviour,
            max_health,
            protection: Default::default(),
            weapon: Some(weapon.to_string()),
            sight: 25.0,
            reach: 1.8,
        }
    };
    // The player: a bunker person of the player's bunker, under human
    // control, starting with nothing in hand.
    reg.register(ActorDef {
        weapon: None,
        ..person("player", Some(PLAYER_FACTION), Behaviour::Guard, 100.0, "")
    })
    .expect("actor registered once");
    for def in [
        person("bunker person", None, Behaviour::Guard, 100.0, "pipe"),
        person("scavenger", Some(TOPSIDE_HOSTILES), Behaviour::Hunter, 60.0, "pipe"),
        person("raider", Some(TOPSIDE_HOSTILES), Behaviour::Hunter, 100.0, "hatchet"),
        person("lookout", Some(TOPSIDE_HOSTILES), Behaviour::Guard, 80.0, "pipe"),
        person("wanderer", None, Behaviour::Guard, 70.0, "pipe"),
    ] {
        reg.register(def).expect("actor registered once");
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
    // Rust's pipe shotgun: a handful of pellets, full damage only up
    // close (Facepunch devblog 123: shotguns fall off hardest).
    let pellet = DamageDef {
        name: "shotgun pellet".to_string(),
        amount: 12.0,
        kind: DamageType::Bullet,
        knockback: 1.0,
        ignores_armor: false,
        self_scale: 0.5,
        falloff: Falloff {
            full_range: 8.0,
            far_range: 30.0,
            far_fraction: 0.2,
        },
    };
    // Landing hard (Quake 3's PM_CrashLand, Rust's fall damage):
    // `amount` per metre per second over the safe landing speed, no
    // armor helps, through the one damage function like every hit.
    let fall = DamageDef {
        name: "fall".to_string(),
        amount: 10.0,
        kind: DamageType::Blunt,
        knockback: 0.0,
        ignores_armor: true,
        self_scale: 1.0,
        falloff: Falloff::NONE,
    };
    // An empty stomach or canteen (todo 10c): `amount` a second, no
    // armor helps, through the one damage function. Tuning.
    let wasting = |name: &str| DamageDef {
        name: name.to_string(),
        amount: 2.0,
        kind: DamageType::Blunt,
        knockback: 0.0,
        ignores_armor: true,
        self_scale: 1.0,
        falloff: Falloff::NONE,
    };
    for def in [
        melee("pipe swing", 20.0, DamageType::Blunt),
        melee("hatchet chop", 30.0, DamageType::Slash),
        pellet,
        fall,
        wasting("starvation"),
        wasting("dehydration"),
        // Caught in the open when the storm hits: `amount` a second, no
        // armor helps. Tuning.
        DamageDef {
            name: "storm".to_string(),
            amount: 25.0,
            kind: DamageType::Blunt,
            knockback: 0.0,
            ignores_armor: true,
            self_scale: 1.0,
            falloff: Falloff::NONE,
        },
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

/// The first hour's first question (design.md "The first hour"): the
/// words on the note in the starting room, one of these, and the
/// name that signs it. Operator, 2026-08-21: "the note tells you not
/// to go outside."
pub const STARTING_NOTES: [&str; 3] = [
    "do not go outside",
    "DO NOT GO OUTSIDE. not yet. wait for me",
    "whatever you hear, do not open the door",
];

/// Names the world signs things with.
pub const NAMES: [&str; 12] = [
    "M.", "Tomas", "Ida", "Reyes", "Cal", "the doctor", "Anja", "Booker", "Sol", "Wren",
    "your brother", "H.",
];

/// The note the starting room holds for this world: same seed, same
/// note.
pub fn starting_note(seed: u64) -> Note {
    let mut roll = Roll::new(seed ^ 0x4e4f_5445);
    Note {
        text: (*roll.pick(&STARTING_NOTES)).to_string(),
        signed: (*roll.pick(&NAMES)).to_string(),
    }
}

pub fn register_items(reg: &mut ItemRegistry) {
    let items = [
        ItemDef {
            name: "note".to_string(),
            unique: false,
            kind: ItemKind::Note,
            max_stack: 1,
            quality_siblings: 1,
            combat: None,
            food: None,
            storage: None,
            armor: None,
            good_for: Default::default(),
            model: None,
        },
        ItemDef {
            name: "canned food".to_string(),
            unique: false,
            kind: ItemKind::Food,
            max_stack: 10,
            quality_siblings: 1,
            combat: None,
            // Food heals a little too (todo 10ca, Valheim). Tuning.
            food: Some(FoodStats {
                hunger: 50.0,
                thirst: 0.0,
                health: 5.0,
            }),
            storage: None,
            armor: None,
            good_for: Default::default(),
            model: None,
        },
        ItemDef {
            name: "water bottle".to_string(),
            unique: false,
            kind: ItemKind::Food,
            max_stack: 5,
            quality_siblings: 1,
            combat: None,
            food: Some(FoodStats {
                hunger: 0.0,
                thirst: 50.0,
                health: 0.0,
            }),
            storage: None,
            armor: None,
            good_for: Default::default(),
            model: None,
        },
        // Made at the bench from cloth; heals far faster than food, a
        // reason to craft before going out (todo 10ca). Tuning.
        ItemDef {
            name: "bandage".to_string(),
            unique: false,
            kind: ItemKind::Medicine,
            max_stack: 5,
            quality_siblings: 1,
            combat: None,
            food: Some(FoodStats {
                hunger: 0.0,
                thirst: 0.0,
                health: 30.0,
            }),
            storage: None,
            armor: None,
            good_for: Default::default(),
            model: None,
        },
        ItemDef {
            name: "storage box".to_string(),
            unique: false,
            kind: ItemKind::Deployable,
            max_stack: 1,
            quality_siblings: 1,
            combat: None,
            food: None,
            storage: Some(12),
            armor: None,
            // A box feeds nobody; the food seen inside it does
            // (modforge `food_worth`, life.md "What things are good for").
            good_for: Default::default(),
            model: None,
        },
        ItemDef {
            name: "scrap".to_string(),
            unique: false,
            kind: ItemKind::Material,
            max_stack: 20,
            quality_siblings: 1,
            combat: None,
            food: None,
            storage: None,
            armor: None,
            good_for: Default::default(),
            model: None,
        },
        ItemDef {
            name: "cloth".to_string(),
            unique: false,
            kind: ItemKind::Material,
            max_stack: 20,
            quality_siblings: 1,
            combat: None,
            food: None,
            storage: None,
            armor: None,
            good_for: Default::default(),
            model: None,
        },
        ItemDef {
            name: "pipe".to_string(),
            unique: false,
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
            armor: None,
            good_for: Default::default(),
            // scripts/models/pipe.py builds it.
            model: Some("pipe.glb".to_string()),
        },
        ItemDef {
            name: "hatchet".to_string(),
            unique: false,
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
            armor: None,
            good_for: Default::default(),
            model: None,
        },
        // Rust's pipe shotgun: one shell, six pellets, wide spread,
        // instant out to 40 m (the hybrid's instant part).
        ItemDef {
            name: "pipe shotgun".to_string(),
            unique: false,
            kind: ItemKind::Weapon,
            max_stack: 1,
            quality_siblings: 3,
            combat: Some(CombatStats {
                damage: "shotgun pellet".to_string(),
                delay: 1.2,
                reach: 40.0,
                pellets: 6,
                spread_degrees: 6.0,
                ammo: Some("shotgun shell".to_string()),
            }),
            food: None,
            storage: None,
            armor: None,
            good_for: Default::default(),
            model: None,
        },
        ItemDef {
            name: "shotgun shell".to_string(),
            unique: false,
            kind: ItemKind::Material,
            max_stack: 20,
            quality_siblings: 1,
            combat: None,
            food: None,
            storage: None,
            armor: None,
            good_for: Default::default(),
            model: None,
        },
        // Worn in the chest slot; guards the chest only.
        ItemDef {
            name: "road sign vest".to_string(),
            unique: false,
            kind: ItemKind::Tool,
            max_stack: 1,
            quality_siblings: 3,
            combat: None,
            food: None,
            storage: None,
            armor: Some(Armor {
                slot: EquipSlot::Chest,
                amount: 40.0,
            }),
            good_for: Default::default(),
            model: None,
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
            // Big monuments keep 180 m from each other.
            spacing: 180.0,
            props: vec![],
            // Walls and a roof: the people who live there rest and
            // feel safe there.
            good_for: GoodFor::new(&[(Need::Rest, 100.0), (Need::Safety, 60.0)]),
            // The tallest thing there (design.md "The triangle
            // rule"): the launch site's gantry is the landmark seen
            // from the bunker door; the power plant's stacks and the
            // city's towers are next.
            height: match name {
                "launch site" => 60.0,
                "power plant" => 35.0,
                "ruined city" => 30.0,
                "research station" | "military outpost" => 12.0,
                _ => 8.0,
            },
        }
    };
    // Minor sites (design.md: "something worth stopping for every 30
    // seconds"): props around a loot box, no buildings, close spacing.
    let prop = |size: [f32; 3], color: Rgb, count: (u32, u32), radius: f32| PropDef {
        size: Vec3::from(size),
        color,
        count,
        radius,
    };
    let minor = |name: &str, danger, suffix: &str, props: Vec<PropDef>| MonumentTypeDef {
        name: name.to_string(),
        slots: vec![],
        arrangement: Clustered,
        danger,
        gated: false,
        suffix: suffix.to_string(),
        spacing: 60.0,
        props,
        // A camp is a place to sleep; a wreck or a grave is not.
        good_for: if name == "campsite" {
            GoodFor::new(&[(Need::Rest, 100.0), (Need::Safety, 30.0)])
        } else {
            GoodFor::default()
        },
        // Low things, hidden behind any rise.
        height: 2.0,
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
        // The minor sites.
        minor(
            "wreck",
            0,
            "Wreck",
            vec![
                prop([4.2, 1.4, 1.9], [0.45, 0.2, 0.15], (1, 2), 4.0),
                prop([0.6, 0.9, 0.6], [0.3, 0.3, 0.32], (0, 3), 5.0),
            ],
        ),
        minor(
            "campsite",
            1,
            "Camp",
            vec![
                prop([2.4, 1.6, 2.4], [0.35, 0.3, 0.2], (1, 2), 4.0),
                prop([1.0, 0.4, 1.0], [0.2, 0.2, 0.2], (1, 1), 2.5),
                prop([0.5, 0.5, 0.5], [0.4, 0.32, 0.2], (1, 3), 3.5),
            ],
        ),
        minor(
            "supply drop",
            1,
            "Drop",
            vec![
                prop([1.6, 1.2, 1.6], [0.3, 0.4, 0.3], (1, 1), 1.5),
                prop([3.0, 0.1, 3.0], [0.6, 0.6, 0.62], (1, 1), 4.0),
            ],
        ),
        minor(
            "body",
            0,
            "Grave",
            vec![
                prop([1.8, 0.3, 0.6], [0.3, 0.28, 0.25], (1, 1), 1.5),
                prop([0.5, 0.5, 0.5], [0.4, 0.4, 0.38], (0, 2), 3.0),
            ],
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
                ScatterDef {
                    size: Vec3::new(0.6, 5.0, 0.6),
                    color: [0.3, 0.22, 0.12],
                    density: 0.04,
                },
                ScatterDef {
                    size: Vec3::new(0.8, 0.5, 0.8),
                    color: [0.4, 0.4, 0.35],
                    density: 0.01,
                },
            ],
            weather: vec!["rain".to_string(), "fog".to_string()],
            monuments: vec![
                "roadside stop".to_string(),
                "research station".to_string(),
                "military outpost".to_string(),
                "campsite".to_string(),
                "body".to_string(),
                "supply drop".to_string(),
            ],
            npcs: vec!["scavenger".to_string(), "hunter".to_string()],
            wildlife: vec!["deer".to_string(), "wolf".to_string()],
            harvest: vec!["scrap".to_string(), "cloth".to_string()],
        },
        BiomeDef {
            name: "wasteland".to_string(),
            ground: [0.55, 0.48, 0.35],
            scatter: vec![
                ScatterDef {
                    size: Vec3::new(1.2, 0.6, 0.8),
                    color: [0.5, 0.42, 0.32],
                    density: 0.005,
                },
            ],
            weather: vec!["dust storm".to_string(), "clear".to_string()],
            monuments: vec![
                "roadside stop".to_string(),
                "airfield".to_string(),
                "train yard".to_string(),
                "launch site".to_string(),
                "wreck".to_string(),
                "body".to_string(),
                "supply drop".to_string(),
            ],
            npcs: vec!["raider".to_string(), "wanderer".to_string()],
            wildlife: vec!["rat".to_string()],
            harvest: vec!["scrap".to_string()],
        },
        BiomeDef {
            name: "ruins".to_string(),
            ground: [0.4, 0.4, 0.42],
            scatter: vec![
                ScatterDef {
                    size: Vec3::new(2.0, 1.5, 1.5),
                    color: [0.45, 0.43, 0.4],
                    density: 0.008,
                },
            ],
            weather: vec!["rain".to_string(), "fog".to_string(), "clear".to_string()],
            monuments: vec![
                "ruined city".to_string(),
                "power plant".to_string(),
                "sewer".to_string(),
                "harbor".to_string(),
                "wreck".to_string(),
                "campsite".to_string(),
                "body".to_string(),
            ],
            npcs: vec![
                "raider".to_string(),
                "scavenger".to_string(),
                "militia".to_string(),
            ],
            wildlife: vec!["rat".to_string(), "crow".to_string()],
            harvest: vec!["scrap".to_string(), "pipe".to_string(), "cloth".to_string()],
        },
        // The small world's ground (design.md "The small world first"):
        // grass with nothing on it. Its sites are placed by hand.
        BiomeDef {
            name: "grassland".to_string(),
            ground: [0.3, 0.45, 0.22],
            scatter: vec![],
            weather: vec!["clear".to_string()],
            monuments: vec![],
            npcs: vec![],
            wildlife: vec![],
            harvest: vec![],
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
        assert_eq!(reg.len(), 12);
        assert!(reg.def("bandage").is_some());
        assert!(reg.def("canned food").is_some());
        assert!(reg.def("hatchet").is_some());
        assert_eq!(reg.def("note").map(|d| d.kind), Some(ItemKind::Note));
        // Every gun's ammo is a registered item.
        for name in ["pipe", "hatchet", "pipe shotgun"] {
            if let Some(ammo) = reg.def(name).unwrap().combat.as_ref().unwrap().ammo.as_deref() {
                assert!(reg.def(ammo).is_some(), "{name} fires unregistered {ammo}");
            }
        }
    }

    #[test]
    fn all_actors_register_and_hold_registered_weapons() {
        let mut items = ItemRegistry::default();
        register_items(&mut items);
        let mut reg = ActorRegistry::default();
        register_actors(&mut reg);
        let names: Vec<&str> = reg.names().collect();
        assert_eq!(names, ["player", "bunker person", "scavenger", "raider", "lookout", "wanderer"]);
        let factions = FactionRegistryRes::default();
        for name in names {
            let def = reg.def(name).unwrap();
            if let Some(weapon) = def.weapon.as_deref() {
                assert!(items.def(weapon).is_some(), "{name} holds unregistered {weapon}");
            } else {
                assert_eq!(name, "player", "everyone but the player starts armed");
            }
            if let Some(faction) = def.faction.as_deref() {
                assert!(factions.0.def(faction).is_some(), "{name} is in unregistered {faction}");
            }
        }
        assert!(reg.def("wanderer").unwrap().faction.is_none(), "the wanderer is neutral");
    }

    #[test]
    fn every_monument_type_rolls_legal_monuments() {
        let regs = MonumentRegistryRes::default();
        assert_eq!(regs.buildings.len(), 19);
        assert_eq!(regs.monuments.len(), 15);
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
        // Every monument a biome allows is a registered type.
        let monuments = MonumentRegistryRes::default();
        for name in ["forest", "wasteland", "ruins"] {
            for allowed in &reg.def(name).unwrap().monuments {
                assert!(monuments.monuments.def(allowed).is_some(), "{name} allows unregistered {allowed}");
            }
        }
    }

    #[test]
    fn the_world_def_rolls_with_the_real_registries() {
        let biomes = BiomeRegistryRes::default();
        let monuments = MonumentRegistryRes::default();
        for seed in [1u64, 2, 3] {
            let world = modforge::worldgen::roll_world(&topside_def(), seed, &biomes.0, &monuments.monuments, &monuments.buildings)
                .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
            assert_eq!(world.sites.len(), 414);
            assert_eq!(world.roads.len(), 38, "roads join the 39 built monuments, none to the bunker");
            // The door opens on the launch site's gantry (design.md
            // "The first hour").
            let door = Vec3::new(0.0, 1.7, 0.0);
            let gantry = world.sites.iter().find(|s| s.monument == "launch site").unwrap();
            let top = world.height_at(gantry.position) + 60.0;
            assert!(
                world.sees(door, Vec3::new(gantry.position.x, top, gantry.position.y)),
                "seed {seed}: the launch site is hidden from the door"
            );
            // Bethesda's rule over the real world: from 200 random
            // land points, a site within 150 m of at least 90 percent.
            let mut roll = modforge::monument::Roll::new(seed ^ 77);
            let (mut land, mut covered) = (0, 0);
            for _ in 0..200 {
                let p = Vec2::new(roll.measure(-1700.0, 1700.0), roll.measure(-1700.0, 1700.0));
                let (c, r) = world.cell_of(p);
                if world.is_water(c, r) {
                    continue;
                }
                land += 1;
                if world.sites.iter().any(|s| s.position.distance(p) <= 150.0) {
                    covered += 1;
                }
            }
            assert!(covered * 10 >= land * 9, "seed {seed}: {covered} of {land} within 150 m");
        }
    }

    /// The game rolls a new seed every launch: any seed must give a
    /// world (a site cut off by water once crashed a launch).
    #[test]
    fn forty_seeds_all_roll() {
        let biomes = BiomeRegistryRes::default();
        let monuments = MonumentRegistryRes::default();
        for seed in 100..140u64 {
            modforge::worldgen::roll_world(&topside_def(), seed, &biomes.0, &monuments.monuments, &monuments.buildings)
                .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        }
    }

    /// Every rolled world is valid (design.md "A valid world"): modforge's
    /// check floods the walk grid from the player's bunker door, and every
    /// building door and room, every spot for a person or a box, and every
    /// other bunker's door must be standable and reached. Each seed's
    /// problems, by name, go to logs/valid.txt.
    #[test]
    fn the_rolled_worlds_are_valid() {
        // Every bunker and the ground round them.
        let far = OTHER_BUNKERS.iter().fold(0.0f32, |m, b| m.max(b.x.abs()).max(b.y.abs())) + 2.0 * BUNKER_CLEAR;
        let (total, report) = check(WorldKind::Big, 100..140, far, "valid.txt");
        assert_eq!(total, 0, "{report}");
    }

    /// The small world (design.md "The small world first") is valid over
    /// all of it, and every one of its sites stands.
    #[test]
    fn the_small_world_is_valid_with_every_site() {
        let (total, report) = check(WorldKind::Small, 1..4, 512.0, "valid_small.txt");
        assert_eq!(total, 0, "{report}");
        let biomes = BiomeRegistryRes::default();
        let monuments = MonumentRegistryRes::default();
        let r = modforge::world2d::Registries {
            biomes: &biomes.0,
            monuments: &monuments.monuments,
            buildings: &monuments.buildings,
        };
        let mut world = roll_world2d(WorldKind::Small, 1, &biomes.0, &monuments.monuments).unwrap();
        let mut sites: Vec<(String, Vec2)> = world
            .sites_in(r, Vec2::splat(-512.0), Vec2::splat(512.0))
            .into_iter()
            .map(|p| (p.monument, p.position))
            .collect();
        sites.sort_by(|a, b| a.0.cmp(&b.0));
        let mut want: Vec<(String, Vec2)> = SMALL_SITES.iter().map(|(m, [x, y])| (m.to_string(), Vec2::new(*x, *y))).collect();
        want.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(sites, want);
        // Every topside person asked for has a spot.
        let people = world.topside_people(r);
        for (def, n) in SMALL_TOPSIDE_PEOPLE {
            assert_eq!(people.iter().filter(|(d, _)| d == def).count(), n as usize, "{def}");
        }
    }

    /// modforge's valid-world check on `kind` for each seed, over the
    /// square `far` metres each way from the bunker: the problems found,
    /// and each seed's problems by name, also written to logs/`file`.
    fn check(kind: WorldKind, seeds: std::ops::Range<u64>, far: f32, file: &str) -> (usize, String) {
        let biomes = BiomeRegistryRes::default();
        let monuments = MonumentRegistryRes::default();
        let bunker = starting_bunker();
        let mut report = Vec::new();
        let mut total = 0;
        let r = modforge::world2d::Registries {
            biomes: &biomes.0,
            monuments: &monuments.monuments,
            buildings: &monuments.buildings,
        };
        let mut bunkers = vec![modforge::valid::Bunker {
            name: "the player's bunker".to_string(),
            at: BUNKER_AT,
            def: &bunker,
        }];
        for (i, at) in kind.other_bunkers().into_iter().enumerate() {
            bunkers.push(modforge::valid::Bunker { name: bunker_faction(i), at, def: &bunker });
        }
        for seed in seeds {
            let mut world = roll_world2d(kind, seed, &biomes.0, &monuments.monuments).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
            let problems = modforge::valid::problems(&mut world, r, Vec2::splat(-far), Vec2::splat(far), &bunkers);
            total += problems.len();
            report.push(format!("seed {seed}: {} problems", problems.len()));
            report.extend(problems.iter().take(40).map(|p| format!("  {p}")));
        }
        let report = report.join("\n");
        let _ = std::fs::create_dir_all("../logs");
        let _ = std::fs::write(format!("../logs/{file}"), &report);
        (total, report)
    }

    #[test]
    fn every_named_world_rolls_and_an_unknown_name_is_refused() {
        let biomes = BiomeRegistryRes::default();
        let monuments = MonumentRegistryRes::default();
        for name in WORLDS {
            let def = world_def(name).unwrap();
            modforge::worldgen::roll_world(&def, 2, &biomes.0, &monuments.monuments, &monuments.buildings)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
        }
        let labyrinth = modforge::worldgen::roll_world(&labyrinth_def(), 2, &biomes.0, &monuments.monuments, &monuments.buildings).unwrap();
        assert!(labyrinth.walls.len() > 1000, "{} wall runs", labyrinth.walls.len());
        assert_eq!(labyrinth.sites.len(), 375);
        assert!(world_def("backrooms").is_err());
    }
}
