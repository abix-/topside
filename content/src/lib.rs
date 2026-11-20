//! The actual items, biomes, building types, and monument types of
//! topside. modforge owns the shapes; this file says what
//! concretely exists in this game.

pub mod brain;

use bevy::math::{Vec2, Vec3};
use bevy::prelude::Resource;
use modforge::worldgen::{BiomeRule, Generator, WorldDef};
use modforge::ground::{ClimateRule, Ground, GroundDef, GroundGen};
use modforge::places::{Places, SiteKind};
use modforge::world2d::{Clearing, World2d};
use modforge::actor::{ActorDef, ActorRegistry, Behaviour, BodyLook, LookDef};
use modforge::biome::{BiomeDef, BiomeRegistry, ScatterDef};
use modforge::combat::{DamageDef, DamageRegistry, DamageType, Falloff};
use modforge::faction::{FactionDef, FactionKind, FactionRegistry};
use modforge::memory::GoodFor;
use modforge::survival::Need;
use modforge::item::{Armor, CombatStats, FoodStats, ItemDef, ItemKind, ItemRegistry, LayerDef, LayerRegistry, Note};
use modforge::monument::Roll;
use modforge::reality::{Danger, RealityDef, RealityRegistry};
use modforge::storyteller::{EpisodeDef, EpisodeRegistry, LineDef, When};
use modforge::monument::{
    Arrangement, BuildingRegistry, BuildingSize, BuildingSlot, BuildingTypeDef,
    MonumentRegistry, MonumentTypeDef, PropDef,
};
use modforge::crafting::{RecipeDef, RecipeRegistry, StationKind};
use modforge::loot::{LootEntry, LootTable};
use modforge::storm::StormDef;
use modforge::structure::{
    CONCRETE_FLOOR, CONCRETE_WALL, Hatch, LightDef, Opening, Rgb, RoomDef, STEP_DEPTH, STEP_RISE_MAX, Side, SolidDef, StairDef,
    StructureDef,
};

/// What lies on the floor of the player's room (design.md: three days of
/// food; a can is half a day of hunger, so six), plus the first weapons
/// and a vest: item, count, salt, where in the bunker's plan (x, z),
/// colour. Water is the bunker's: its barrel and bottles (`BUNKER_STORE`).
pub const STARTING_ITEMS: [(&str, u32, u64, [f32; 2], Rgb); 8] = [
    // The bolt in your hand (design.md "The first hour" rule 5).
    ("bolt", 5, 17, [0.8, 2.6], [0.55, 0.56, 0.58]),
    // Enough cloth for the bench's first bandage.
    ("cloth", 2, 14, [-1.6, 2.0], [0.75, 0.7, 0.6]),
    ("canned food", 6, 1, [-1.0, 2.0], [0.2, 0.6, 0.2]),
    // West of the stairs (x 1.4 to 2.6).
    ("scrap", 3, 2, [-0.4, 2.0], [0.5, 0.5, 0.5]),
    ("pipe shotgun", 1, 9, [0.2, 2.0], [0.3, 0.3, 0.3]),
    ("shotgun shell", 8, 10, [0.2, 2.6], [0.8, 0.3, 0.2]),
    ("pipe", 1, 11, [-0.4, 2.6], [0.4, 0.4, 0.45]),
    ("road sign vest", 1, 12, [0.8, 2.0], [0.2, 0.5, 0.2]),
];

/// How tall each level of the bunker you start in is, and the levels of
/// its two rooms (design.md "The bunker you start in"): the surface room
/// on the ground, the player's room below it.
/// What every person wears from the start: a shirt, trousers and boots,
/// each a layer of their model (scripts/models.py `WORN`).
pub const WORN: [&str; 3] = ["shirt", "trousers", "boots"];

pub const BUNKER_LEVEL: f32 = 3.0;
pub const PLAYER_ROOM_FLOOR: f32 = -BUNKER_LEVEL;
pub const SURFACE_ROOM_FLOOR: f32 = 0.0;

/// Where the crafting bench is used in the starting bunker's plan
/// (x, z), in the surface room: its right end.
pub const BENCH_AT: [f32; 2] = [2.8, 3.5];

/// Where the research station is used, in the surface room: its left
/// end, across from the bench (todo 28; the place is Claude's).
pub const RESEARCH_AT: [f32; 2] = [-2.8, 3.5];

/// Where the note lies in the starting bunker's plan (x, z), in the
/// player's room beside the bed: the first question (design.md "The
/// first hour").
pub const STARTING_NOTE_AT: [f32; 2] = [-1.4, 3.6];

/// Where the room's question lies in the starting bunker's plan (x, z)
/// when it is not the note: by the note, west of the stairs.
pub const QUESTION_AT: [f32; 2] = [0.4, 3.6];

/// Every recipe, as data (modforge crafting).
#[derive(Resource)]
pub struct RecipeRegistryRes(pub RecipeRegistry);

impl Default for RecipeRegistryRes {
    fn default() -> Self {
        let mut reg = RecipeRegistry::default();
        // The bench's recipes (todo 10, Craft 1; design.md "Technology and
        // crafting"): cloth into a bandage, scrap into a pipe, and the
        // improvised weapons, a spiked bat (a handle with nails) and a
        // sharpened road sign on a pipe. What each takes is Claude's, for
        // the operator to change.
        let made = |name: &str, inputs: &[(&str, u32)], output: (&str, u32), layers: &[&str]| RecipeDef {
            name: name.to_string(),
            inputs: inputs.iter().map(|(i, n)| (i.to_string(), *n)).collect(),
            output: (output.0.to_string(), output.1),
            layers: layers.iter().map(|l| l.to_string()).collect(),
            station: StationKind::CraftingBench,
            craft_time: 0.0,
        };
        let bench = |name: &str, inputs: &[(&str, u32)], output: (&str, u32)| made(name, inputs, output, &[]);
        for def in [
            bench("bandage", &[("cloth", 2)], ("bandage", 1)),
            bench("pipe", &[("scrap", 3)], ("pipe", 1)),
            made("spiked bat", &[("wooden handle", 1), ("scrap", 2)], ("wooden handle", 1), &["nails"]),
            made("sharpened road sign", &[("pipe", 1), ("scrap", 3)], ("pipe", 1), &["sharpened road sign"]),
        ] {
            reg.register(def).expect("recipe registered once");
        }
        Self(reg)
    }
}

/// The research topics (todo 28, modforge tech): the improvised weapons
/// and field medicine unlock the bench's recipes; better tools branches
/// from the weapons and never caps. Topics, costs and branches are
/// Claude's, for the operator to change.
pub fn tech_tree() -> modforge::tech::TechTree {
    let topic = |name: &str, cost: &[(&str, u32)], needs: &[&str], unlocks: &[&str], endless| modforge::tech::TopicDef {
        name: name.to_string(),
        cost: cost.iter().map(|(i, n)| (i.to_string(), *n)).collect(),
        needs: needs.iter().map(|s| s.to_string()).collect(),
        unlocks: unlocks.iter().map(|s| s.to_string()).collect(),
        endless,
    };
    modforge::tech::TechTree {
        topics: vec![
            topic("field medicine", &[("cloth", 2)], &[], &["bandage"], false),
            topic("pipework", &[("scrap", 2)], &[], &["pipe"], false),
            topic("improvised weapons", &[("scrap", 4)], &["pipework"], &["spiked bat", "sharpened road sign"], false),
            topic("better tools", &[("scrap", 5)], &["improvised weapons"], &[], true),
        ],
    }
}

/// What a monument's box can hold (design.md "Loot tables scale with
/// danger"): common junk anywhere, weapons and the vest only where it
/// is dangerous. Tuning.
pub fn loot_table() -> LootTable {
    // A handle and its head (design.md "What 2D changes": any handle
    // goes with any head).
    let weapon = |item: &str, head: &[&str], weight, min_danger| LootEntry {
        item: item.to_string(),
        layers: head.iter().map(|h| h.to_string()).collect(),
        count: (1, 1),
        weight,
        min_danger,
    };
    let e = |item: &str, count: (u32, u32), weight, min_danger| LootEntry {
        count,
        ..weapon(item, &[], weight, min_danger)
    };
    LootTable {
        entries: vec![
            e("scrap", (1, 4), 10, 0),
            e("cloth", (1, 3), 8, 0),
            e("canned food", (1, 2), 6, 0),
            // One to a slot, found full (structure::spawn_monuments).
            e("water bottle", (1, 1), 6, 0),
            e("shotgun shell", (2, 5), 4, 1),
            e("bandage", (1, 1), 3, 1),
            weapon("pipe", &[], 2, 1),
            weapon("wooden handle", &["nails"], 2, 1),
            weapon("wooden handle", &["scrap blade"], 1, 1),
            weapon("wooden handle", &["axe head"], 2, 2),
            weapon("pipe", &["sharpened road sign"], 1, 2),
            weapon("long pole", &["spear point"], 1, 2),
            e("road sign vest", (1, 1), 1, 3),
            e("helmet", (1, 1), 1, 2),
            // Building (todo 9f; weights Claude's).
            e(WOOD, (5, 20), 8, 0),
            e(STONE, (5, 15), 5, 0),
            e(BUILDING_PLAN, (1, 1), 3, 0),
            e(HAMMER, (1, 1), 3, 0),
            e(TOOL_CUPBOARD, (1, 1), 1, 1),
            e(TURRET, (1, 1), 1, 3),
            e(FUEL, (1, 3), 4, 0),
            e(SHOVEL, (1, 1), 2, 0),
            e("crafting bench", (1, 1), 1, 1),
            e("research station", (1, 1), 1, 2),
            e(GENERATOR, (1, 1), 1, 2),
            weapon("pipe shotgun", &["pipe barrel", "wooden grip", "wooden stock"], 1, 3),
            weapon("pipe shotgun", &["short pipe barrel", "taped grip"], 1, 3),
            weapon("pipe shotgun", &["pipe barrel", "taped grip", "wooden stock", "nail sight"], 1, 4),
        ],
        picks: (1, 2),
        picks_per_danger: 1,
    }
}

/// One game day in real seconds (the 3D game's Valheim-scale pacing;
/// tune later). A full stomach lasts one, a full canteen half of one,
/// rest one awake.
pub const GAME_DAY_SECS: f32 = 1200.0;

/// How long the world runs before the player wakes, in game seconds
/// (design.md "History, not decoration": "before the player wakes, the
/// world is run: the people ... live for some days in the rolled world";
/// todo 4a, 4b). Two days is Claude's reading of "some", for the operator
/// to change: every second of it is loading time.
pub const HISTORY_SECS: f32 = 2.0 * GAME_DAY_SECS;

/// How a storm goes (design.md "The storm", MISERY's emission): a minute
/// of warning, twenty seconds of storm. When it comes is the episode's
/// (its `storm_in`). Tuning.
/// When the sky changes once in the first hour, in seconds after the
/// player wakes, and for how long (todo 10cb). Claude's numbers, for the
/// operator to change.
pub const SKY_CHANGE_AFTER_SECS: f32 = 1800.0;
pub const SKY_CHANGE_SECS: f32 = 30.0;

/// The first teeth (todo 10i1; design.md "Maze, mined"): a burner, the
/// damage it does to whoever walks in, the model of its tell, how far it
/// reaches from its middle, and how far outside the second site's ground
/// it lies, toward the bunker. Claude's numbers, for the operator to
/// change.
pub const ANOMALY: &str = "burner";
pub const ANOMALY_TELL: &str = "scorched ground";
pub const ANOMALY_RADIUS: f32 = 2.0;
pub const ANOMALY_FROM_SITE: f32 = 10.0;

/// The danger a secret site's loot is rolled at (todo 11x1): a secret is
/// worth finding. Claude's, for the operator to change.
pub const SECRET_DANGER: u32 = 3;

/// Building (todo 9f): the building plan, the hammer, the tool cupboard,
/// and the materials.
pub const BUILDING_PLAN: &str = "building plan";
pub const HAMMER: &str = "hammer";
pub const TOOL_CUPBOARD: &str = "tool cupboard";
pub const WOOD: &str = "wood";
pub const STONE: &str = "stone";
/// A biome's weather list (todo 21, 32): each weather named as often as
/// its weight, rolled a game hour at a time (modforge weather). The
/// weights are Claude's.
fn weather_list(weights: &[(&str, u32)]) -> Vec<String> {
    weights.iter().flat_map(|(name, n)| std::iter::repeat_n(name.to_string(), *n as usize)).collect()
}

/// The Labyrinth's gate site and its model (todo 11v).
pub const GATE: &str = "gate";
/// The turret (todo 11k): the item set down and the actor it becomes.
pub const TURRET: &str = "turret";
/// What a turret fires.
pub const TURRET_AMMO: &str = "shotgun shell";

/// Power (todo 29, modforge power): the generator, its fuel, what it
/// gives and how long a can runs it, how far its wiring reaches, what
/// the research station draws, where the bunker's generator stands
/// (surface room, south east corner) and the fuel it starts with. All
/// Claude's, for the operator to change.
/// The car (todo 57, modforge vehicle): three times walking, a tank of
/// ten minutes' driving, a can giving three, two seats, one breakdown in
/// a few hours driven; parked a little south of the player's bunker,
/// with a little fuel; repaired with scrap. All Claude's.
pub const CAR: &str = "car";
pub const CAR_DEF: modforge::vehicle::VehicleDef =
    modforge::vehicle::VehicleDef { speed: 3.0, tank: 600.0, per_can: 180.0, seats: 2, breakdown: 0.005 };
pub const CAR_PARKED: [f32; 2] = [0.0, -12.0];
pub const CAR_FUEL: f32 = 120.0;
pub const CAR_REPAIR: (&str, u32) = ("scrap", 3);
pub const GENERATOR: &str = "generator";
pub const FUEL: &str = "fuel";
pub const GENERATOR_DEF: modforge::power::GeneratorDef = modforge::power::GeneratorDef { output: 10.0, burn_secs: 600.0 };
pub const WIRE_REACH: f32 = 12.0;
pub const RESEARCH_DRAW: f32 = 5.0;
pub const GENERATOR_AT: [f32; 2] = [2.5, -3.3];
pub const GENERATOR_FUEL: u32 = 3;

/// What building at a grade costs (Rust's: twig and wood are wood, stone
/// is stone, metal and armored are metal, here scrap; Claude's mapping).
pub fn grade_material(grade: modforge::building::Grade) -> &'static str {
    use modforge::building::Grade;
    match grade {
        Grade::Twig | Grade::Wood => WOOD,
        Grade::Stone => STONE,
        Grade::Metal | Grade::Armored => "scrap",
    }
}

/// The bolt (todo 10i2): the item, how far it is thrown, how fast it
/// flies, and how long an anomaly it lands in shows it has gone off.
/// Claude's numbers, for the operator to change.
pub const BOLT: &str = "bolt";
pub const BOLT_REACH: f32 = 12.0;
pub const BOLT_SPEED: f32 = 15.0;
pub const GOES_OFF_SECS: f32 = 1.5;

pub const STORM: StormDef = StormDef {
    warning_secs: 60.0,
    storm_secs: 20.0,
};

/// Who waits outside the starting bunker's door and where in its plan
/// (x, z): a scavenger 6 m past the door (the 3D game's first fight).
pub const DOOR_ENEMY: (&str, [f32; 2]) = ("scavenger", [0.0, 10.0]);

/// Which actor def stands at a monument's NPC spot of this danger
/// (todo 11q: modforge should decide this; until then it is content).
pub fn def_for_danger(danger: u32) -> &'static str {
    if danger < 3 { "scavenger" } else { "raider" }
}

/// The hatch over the starting bunker's stairs (design.md "The bunker you
/// start in": the player's room's door, lying in the surface room's floor
/// over the stairs): its hole's first tile in the bunker's plan (x, z) and
/// how many tiles across x and z, turning up on its east edge against the
/// wall. As long as the stairs need headroom under it: someone on a step
/// whose top is higher than the room below's ceiling less a body's height
/// is under the hole (the flight starts 0.48 m into it from the south).
pub const HATCH_MIN: (i32, i32) = (1, -2);
pub const HATCH_SIZE: (i32, i32) = (2, 4);

/// The bunker you start in (design.md "The bunker you start in"): two
/// rooms, the surface room on the ground with the one door topside in its
/// south wall, and the player's room right below it; stairs in the
/// player's room climb north along its east wall to the hatch in the
/// surface room's floor. Every bunker is this building. The bed and the
/// food crate in the player's room; the crafting bench in the surface room.
/// The starting bunker dug `depth` levels down (todo 30, the silo;
/// authority.md "Silo", Fallout Shelter's vertical): each level a room
/// of the bunker's footprint under the one above, reached by a flight up
/// through a hatch in the floor above, the hatches turn and turn about
/// the west and the east of the room so no flight lands on another.
pub fn silo_bunker(depth: u32) -> StructureDef {
    let mut def = starting_bunker();
    let flight_length = (BUNKER_LEVEL / STEP_RISE_MAX).ceil() * STEP_DEPTH;
    for n in 1..=depth {
        let level = PLAYER_ROOM_FLOOR - n as f32 * BUNKER_LEVEL;
        def.rooms.push(RoomDef {
            origin: Vec3::new(0.0, level, 0.0),
            interior: Vec3::new(6.0, BUNKER_LEVEL, 8.0),
            wall_thickness: 0.2,
            openings: Vec::new(),
            floor: true,
            ceiling: false,
        });
        let min = if n % 2 == 1 { SILO_HATCH_MIN } else { HATCH_MIN };
        let hole_x = min.0 as f32 + HATCH_SIZE.0 as f32 / 2.0;
        def.stairs.push(StairDef {
            base: Vec3::new(hole_x, level, min.1 as f32 + flight_length),
            side: Side::North,
            width: 1.2,
            rise: BUNKER_LEVEL,
            landing: 0.0,
        });
        def.hatches.push(Hatch { level: level + BUNKER_LEVEL, min, size: HATCH_SIZE, hinge: Side::East });
        def.lights.push(LightDef { position: Vec3::new(0.0, level + 2.7, 0.0), color: [1.0, 0.9, 0.7], intensity: 400_000.0 });
    }
    def
}

/// The silo's other hatch, in the west of the room, clear of the bed
/// and the food crate (the place is Claude's).
pub const SILO_HATCH_MIN: (i32, i32) = (-3, -2);

/// What digging the next silo level takes: deeper costs more, the scrap
/// times the level dug (Claude's numbers), with a shovel held.
pub const SHOVEL: &str = "shovel";
pub fn dig_cost(depth: u32) -> (&'static str, u32) {
    ("scrap", 5 * (depth + 1))
}

/// The stations that can be carried and set down (todo 30): each item's
/// station.
pub fn station_of(item: &str) -> Option<StationKind> {
    match item {
        "crafting bench" => Some(StationKind::CraftingBench),
        "research station" => Some(StationKind::ResearchStation),
        _ => None,
    }
}

pub fn starting_bunker() -> StructureDef {
    let room = |level: f32, openings: Vec<Opening>| RoomDef {
        origin: Vec3::new(0.0, level, 0.0),
        interior: Vec3::new(6.0, BUNKER_LEVEL, 8.0),
        wall_thickness: 0.2,
        openings,
        floor: true,
        ceiling: level >= SURFACE_ROOM_FLOOR,
    };
    // The front door: a 2 m door block in the middle of the south wall.
    let front_door = Opening {
        side: Side::South,
        offset: 0.0,
        width: 2.0,
        sill: 0.0,
        door: true,
    };
    // The flight: 1.2 m wide in the hatch's hole, its top at the hole's
    // north edge, level with the surface room's floor.
    let flight_length = (BUNKER_LEVEL / STEP_RISE_MAX).ceil() * STEP_DEPTH;
    let hole_x = HATCH_MIN.0 as f32 + HATCH_SIZE.0 as f32 / 2.0;
    StructureDef {
        name: "the starting bunker".to_string(),
        wall_color: CONCRETE_WALL,
        floor_color: CONCRETE_FLOOR,
        rooms: vec![room(SURFACE_ROOM_FLOOR, vec![front_door]), room(PLAYER_ROOM_FLOOR, Vec::new())],
        stairs: vec![StairDef {
            base: Vec3::new(hole_x, PLAYER_ROOM_FLOOR, HATCH_MIN.1 as f32 + flight_length),
            side: Side::North,
            width: 1.2,
            rise: BUNKER_LEVEL,
            landing: 0.0,
        }],
        hatches: vec![Hatch {
            level: SURFACE_ROOM_FLOOR,
            min: HATCH_MIN,
            size: HATCH_SIZE,
            hinge: Side::East,
        }],
        furniture: vec![
            // The bed, in the player's room.
            SolidDef {
                center: Vec3::new(-2.4, PLAYER_ROOM_FLOOR + 0.25, 3.0),
                size: Vec3::new(1.0, 0.5, 2.0),
                color: [0.30, 0.35, 0.55],
            },
            // The crafting bench, in the surface room.
            SolidDef {
                center: Vec3::new(2.2, SURFACE_ROOM_FLOOR + 0.45, 3.5),
                size: Vec3::new(1.6, 0.9, 0.7),
                color: [0.45, 0.32, 0.18],
            },
            // The research station, in the surface room (todo 28).
            SolidDef {
                center: Vec3::new(-2.2, SURFACE_ROOM_FLOOR + 0.45, 3.5),
                size: Vec3::new(1.6, 0.9, 0.7),
                color: [0.30, 0.40, 0.42],
            },
            // The food crate, in the player's room.
            SolidDef {
                center: Vec3::new(-2.5, PLAYER_ROOM_FLOOR + 0.35, -3.3),
                size: Vec3::new(0.7, 0.7, 0.7),
                color: [0.55, 0.45, 0.20],
            },
        ],
        lights: [SURFACE_ROOM_FLOOR, PLAYER_ROOM_FLOOR]
            .map(|level| LightDef {
                position: Vec3::new(0.0, level + 2.7, 0.0),
                color: [1.0, 0.9, 0.7],
                intensity: 400_000.0,
            })
            .to_vec(),
        parts: Vec::new(),
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

/// Every item's one fixed value, in scrap (operator, 2026-10-02; modforge
/// `trade`): what trading weighs. Scrap, the currency, is worth 1. Tuning.
#[derive(Resource)]
pub struct ItemValuesRes(pub modforge::trade::Values);

impl Default for ItemValuesRes {
    fn default() -> Self {
        let values = [
            ("scrap", 1),
            ("cloth", 2),
            ("shotgun shell", 3),
            ("wooden handle", 3),
            ("long pole", 3),
            ("pipe", 4),
            ("bandage", 8),
            ("water bottle", 10),
            ("canned food", 12),
            ("storage box", 15),
            ("road sign vest", 25),
            ("pipe shotgun", 60),
        ];
        Self(modforge::trade::Values(values.into_iter().map(|(item, value)| (item.to_string(), value)).collect()))
    }
}

/// The layer registry as an app resource (design.md "What 2D changes":
/// every item is made in layers): every layer a stack carries is a def
/// from here.
#[derive(Resource)]
pub struct LayerRegistryRes(pub LayerRegistry);

impl Default for LayerRegistryRes {
    fn default() -> Self {
        let mut reg = LayerRegistry::default();
        register_layers(&mut reg);
        Self(reg)
    }
}

/// The first heads and the first gun's parts (design.md "What 2D
/// changes"). A head lands its own damage; a handle with no head hits
/// with the handle. A gun's part adds to its numbers: the barrel reach
/// and spread, the stock spread, the grip speed, the sight reach. Each
/// is drawn as its own model over its item. Tuning.
pub fn register_layers(reg: &mut LayerRegistry) {
    let layer = |name: &str, slot: &str, damage: Option<&str>, reach, spread_degrees, delay| LayerDef {
        name: name.to_string(),
        slot: slot.to_string(),
        damage: damage.map(String::from),
        reach,
        spread_degrees,
        delay,
        picture: Some(name.to_string()),
    };
    let heads = [
        ("nails", "nail hit"),
        ("scrap blade", "blade cut"),
        ("axe head", "axe chop"),
        ("spear point", "spear thrust"),
        ("sharpened road sign", "sign cut"),
    ]
    .map(|(name, damage)| layer(name, "head", Some(damage), 0.0, 0.0, 0.0));
    let gun_parts = [
        layer("pipe barrel", "barrel", None, 20.0, -3.0, 0.0),
        layer("short pipe barrel", "barrel", None, 5.0, 2.0, 0.0),
        layer("wooden grip", "grip", None, 0.0, 0.0, -0.1),
        layer("taped grip", "grip", None, 0.0, 0.0, -0.2),
        layer("wooden stock", "stock", None, 0.0, -2.0, 0.0),
        layer("nail sight", "sight", None, 5.0, 0.0, 0.0),
    ];
    for def in heads.into_iter().chain(gun_parts) {
        reg.register(def).expect("layer registered once");
    }
}

/// The layers an item lies in the starting bunker with: the pipe shotgun
/// with its barrel, grip, and stock.
pub fn starting_layers(item: &str) -> &'static [&'static str] {
    match item {
        "pipe shotgun" => &["pipe barrel", "wooden grip", "wooden stock"],
        _ => &[],
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

/// One line as a rule (modforge storyteller `LineDef`; episodes.md "One
/// way for every conversation"): its name, who may say it ("player", an
/// episode part, "" anyone else), what must be true, its ways, what it
/// tells of, and how hearing it feels.
pub fn line(name: &str, part: &str, when: Vec<When>, ways: &[&str], tells: &str, felt: f32) -> LineDef {
    LineDef {
        name: name.to_string(),
        part: part.to_string(),
        when,
        ways: ways.iter().map(|w| w.to_string()).collect(),
        tells: tells.to_string(),
        felt,
        wait: 0.0,
        gives: String::new(),
    }
}

/// Silence as a player's line (episodes.md: doing nothing is always a
/// choice): never shown; said by itself when the player chooses nothing
/// within `wait` real seconds of their choices showing.
pub fn silence(name: &str, when: Vec<When>, wait: f32, felt: f32) -> LineDef {
    LineDef { wait, ..line(name, "player", when, &[], "", felt) }
}

/// Plain talk: what anyone says outside an episode's lines, the same rules
/// (episodes.md "One way for every conversation"). With nothing to talk
/// about, a short line, and it ends (episodes.md "Conversations").
pub fn plain_lines() -> Vec<LineDef> {
    let heard = |name: &str| When::Heard(name.to_string());
    vec![
        line("nothing to say", "", vec![When::Opened], &["Not now.", "What do you want?", "Leave me be.", "I've got nothing for you.", "Busy."], "", 0.0),
        // Thirsty, the player's first word can be to ask; it is not offered
        // after every reply, so the conversation still ends.
        line(
            "asks for water",
            "player",
            vec![When::Needs("thirst".to_string(), THIRSTY), When::First],
            &["Know where there's water?", "Any water around here?"],
            "",
            0.0,
        ),
        // A well they know of and do not keep quiet, by the place it is at
        // when they know it.
        line(
            "gives the well",
            "",
            vec![heard("asks for water"), When::Knows("well".to_string())],
            &["There's a well at the {place}.", "Try the {place}. There's a well.", "There's a well not far from here."],
            "well",
            0.1,
        ),
        line("no water", "", vec![heard("asks for water")], &["No idea.", "Find your own.", "If I knew, I'd be drinking."], "", 0.0),
        // Holding a place, the one warning to anyone they stand hostile to
        // who comes for it (todo 11z), called to them.
        line("warns off", "", vec![When::Warning], &["Stay back. This place is held.", "Not one step closer.", "Turn around. This is ours."], "", 0.0),
        // Joining (todo 11af, draft): the player
        // asks a stranger to stay; they say yes feeling well enough toward
        // the player (relationship::FRIENDLY, orchestrator 2026-10-02), and
        // saying it, they join (`AGREES_TO_STAY`).
        // Not to one who asked for the water, whom the episode's own answers
        // take in (`STRANGER_ASKS`; that stranger speaks up first,
        // talk::speak_up).
        line(
            "asks to stay",
            "player",
            vec![When::To("stranger".to_string()), When::Not(Box::new(When::HasHeard(STRANGER_ASKS.to_string())))],
            &["You can stay with us."],
            "",
            0.0,
        ),
        line(AGREES_TO_STAY, "", vec![heard("asks to stay"), When::Feels(modforge::relationship::FRIENDLY)], &["All right. I'll stay with you.", "Yes. I'd like that."], "", 0.0),
        line("will not stay", "", vec![heard("asks to stay")], &["No. I don't know you well enough.", "I'll keep to myself."], "", 0.0),
    ]
}

/// The line a person says agreeing to stay with the one who asked: said,
/// they join that one's group (todo 11af, `joining::join`).
pub const AGREES_TO_STAY: &str = "agrees to stay";

/// A stranger who came for the water asks for it (an episode's line by
/// this name; plain talk's "asks to stay" is not offered once it is heard).
pub const STRANGER_ASKS: &str = "a stranger asks for water";

/// Thirst at or below which the player can ask anyone for water (modforge
/// `SurvivalStats`, full at 100). Tuning.
pub const THIRSTY: f32 = 50.0;

/// The names people are given (everybody has a name; operator,
/// 2026-09-27), one rolled for each person from who they are.
pub const PERSON_NAMES: [&str; 40] = [
    "Abel", "Ada", "Amos", "Bea", "Bram", "Cass", "Cole", "Dana", "Dex", "Edie", "Eli", "Faye", "Finn", "Gus",
    "Hana", "Hank", "Ida", "Ike", "Jada", "Jude", "Kit", "Lena", "Lou", "Mae", "Milo", "Nell", "Ned", "Opal",
    "Otis", "Pearl", "Quinn", "Rhea", "Rudy", "Sage", "Silas", "Tess", "Tobias", "Vera", "Wade", "Zelda",
];

/// The reality registry as an app resource: the storm hands over one of
/// these (realities.md).
#[derive(Resource)]
pub struct RealityRegistryRes(pub RealityRegistry);

impl Default for RealityRegistryRes {
    fn default() -> Self {
        let mut reg = RealityRegistry::default();
        register_realities(&mut reg);
        Self(reg)
    }
}

/// The realities built so far (docs/realities/), Mixed world first: the
/// world before any storm. Each changes only rules the game already has.
pub fn register_realities(reg: &mut RealityRegistry) {
    let reality = |name: &str, danger, night, death_sticks| RealityDef {
        name: name.to_string(),
        danger,
        night,
        death_sticks,
    };
    for def in [
        reality("Mixed world", Danger::Hard, true, true),
        reality("Endless Day", Danger::Deadly, false, true),
        reality("Loop", Danger::Deadly, true, false),
    ] {
        reg.register(def).expect("each reality is registered once");
    }
}

/// The episode registry as an app resource: after the storm hands over a
/// reality, the storyteller starts one of these that fits it
/// (episodes.md).
#[derive(Resource)]
pub struct EpisodeRegistryRes(pub EpisodeRegistry);

impl Default for EpisodeRegistryRes {
    fn default() -> Self {
        let mut reg = EpisodeRegistry::default();
        register_episodes(&mut reg);
        Self(reg)
    }
}

impl EpisodeRegistryRes {
    /// How hearing a line feels, by its name (modforge relationship::feeling's
    /// `heard`): an episode line's own value, and the talk path's own words
    /// (`SAID_FELT`). The one lookup every feeling uses.
    pub fn felt(&self, line: &str) -> f32 {
        self.0.felt(line) + SAID_FELT.iter().find(|(name, _)| *name == line).map_or(0.0, |(_, f)| *f)
    }
}

/// How the talk path's own words feel to the one who hears them (modforge
/// talk records an Offer as "offered", a Threaten as "threatened"):
/// a gift warms them a little, a threat a lot the other way (orchestrator,
/// 2026-10-02, life.md "How a person feels about others"; for the operator
/// to change).
pub const SAID_FELT: [(&str, f32); 2] = [("offered", 0.2), ("threatened", -0.5)];

/// Where the episodes are (docs/episodes/): one YAML file each,
/// content/episodes/<name>.yaml, and episodes.yaml listing them in order.
/// The order counts: before any storm the first that fits starts (modforge
/// `EpisodeRegistry::pick`), so the first episode is first (operator,
/// 2026-09-27).
pub const EPISODES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/episodes");

/// One episode from its file's text, the one loader. Through a JSON value:
/// serde_yaml reads an enum only from a tag and never one inside another (a
/// condition's Not), so the file holds plain maps and serde_json reads them.
pub fn episode_from_yaml(text: &str) -> Result<EpisodeDef, String> {
    let value: serde_json::Value = serde_yaml::from_str(text).map_err(|e| e.to_string())?;
    serde_json::from_value(value).map_err(|e| e.to_string())
}

/// Every episode, read from its file in the order episodes.yaml lists. A
/// file that does not read stops the game at once, naming it.
pub fn episodes() -> Vec<EpisodeDef> {
    let read = |file: &str| std::fs::read_to_string(format!("{EPISODES_DIR}/{file}")).unwrap_or_else(|e| panic!("{EPISODES_DIR}/{file}: {e}"));
    let order: Vec<String> = serde_yaml::from_str(&read("episodes.yaml")).unwrap_or_else(|e| panic!("{EPISODES_DIR}/episodes.yaml: {e}"));
    order.iter().map(|file| episode_from_yaml(&read(file)).unwrap_or_else(|e| panic!("{EPISODES_DIR}/{file}: {e}"))).collect()
}

/// Every episode, read at start (`episodes`); their own items are
/// registered with every other item (`register_items`).
pub fn register_episodes(reg: &mut EpisodeRegistry) {
    for def in episodes() {
        reg.register(def).expect("each episode is registered once");
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
        (GATE.to_string(), 150),
    ];
    // Nothing in the maze stands over its walls (the gate is lower than
    // them), so no landmark can be in sight from the door.
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
        // Rivers wind across the world, a ford every so often (design.md:
        // a livable world, rivers drinkable).
        river_size: 1500.0,
        river_width: 0.02,
        river_fords: 0.3,
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

/// What people look like (design.md "What 2D changes": built from pieces,
/// and they differ): the bodies and pieces scripts/models.py renders
/// (`person <body> body <facing>`, `person <body> <piece> <facing>`), and
/// the tints: skin multiplies the light skin picture, hair the grey hair.
pub fn look_def() -> LookDef {
    let slot = |names: &[Option<&str>]| names.iter().map(|n| n.map(str::to_string)).collect();
    LookDef {
        bodies: vec![
            BodyLook {
                name: "male".to_string(),
                slots: vec![slot(&[Some("simpleparted"), Some("buzzed"), Some("long"), None]), slot(&[Some("beard"), None, None])],
            },
            BodyLook {
                name: "female".to_string(),
                slots: vec![slot(&[Some("buns"), Some("buzzedfemale"), Some("long")])],
            },
        ],
        skins: vec![[1.0, 1.0, 1.0], [0.85, 0.72, 0.62], [0.65, 0.5, 0.4], [0.45, 0.33, 0.26]],
        hair: vec![
            [0.12, 0.1, 0.09],
            [0.3, 0.2, 0.12],
            [0.5, 0.35, 0.2],
            [0.85, 0.72, 0.45],
            [0.75, 0.75, 0.75],
            [0.6, 0.25, 0.12],
        ],
    }
}

/// The small world (design.md "The small world first"): 32 by 32 chunks,
/// flat grass with forests (added back first, 2026-09-26), rivers with
/// fords (a livable world, 2026-09-26), no lakes, no cliffs.
pub fn small_ground_def() -> GroundDef {
    GroundDef {
        feature_size: 500.0,
        water_level: -2.0,
        cliff_every: 0.0,
        cliff_gaps: 0.0,
        // Small enough that the kilometre across holds several forests.
        climate_size: 250.0,
        climate: vec![
            ClimateRule {
                biome: "forest".to_string(),
                moisture: (0.6, 1.0),
                temperature: (0.0, 1.0),
                land: Ground::ForestFloor,
                patches: Ground::Grass,
            },
            ClimateRule {
                biome: "grassland".to_string(),
                moisture: (0.0, 1.0),
                temperature: (0.0, 1.0),
                land: Ground::Grass,
                patches: Ground::Grass,
            },
        ],
        beach: 0.0,
        patch_size: 80.0,
        chunks_across: Some(32),
        // One or two rivers across the kilometre, about 10 m wide.
        river_size: 600.0,
        river_width: 0.03,
        river_fords: 0.3,
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
pub const SMALL_TOPSIDE_PEOPLE: [(&str, u32); 4] = [("wanderer", 25), ("scavenger", 13), ("raider", 12), ("wolf", 10)];

/// The wild animals (design.md "Bunker people and topside people"): each
/// drawn as its own body, `person <name>` (scripts/models.py), with no
/// rolled look and nothing worn.
pub const ANIMALS: [&str; 1] = ["wolf"];

/// What an armored hostile wears (design.md "Enemy randomization"): the
/// one armor there is.
pub const HOSTILE_ARMOR: &str = "road sign vest";

/// How many game days each food keeps before it turns to `ROTTEN_FOOD`
/// (design.md "Survival pressures": food spoils over time). The 10 is
/// Claude's, for the operator to change.
pub const KEEPS_DAYS: [(&str, f32); 1] = [("canned food", 10.0)];
/// What spoiled food becomes.
pub const ROTTEN_FOOD: &str = "rotten food";

/// The affixes a found weapon rolls on top of its tier (design.md "Loot
/// randomization": a lightweight pipe swings faster, a precise gun is more
/// accurate, and affixes stack), as (name, reach, spread, delay, power).
/// "Sturdy" and "rugged" wait for durability and condition. The numbers and
/// the one in five are Claude's, for the operator to change.
pub const AFFIX_ONE_IN: u64 = 5;
pub fn affixes(gun: bool) -> Vec<modforge::affix::Affix> {
    let melee: &[(&str, f32, f32, f32, f32)] = &[("lightweight", 0.0, 0.0, -0.1, 1.0), ("sharpened", 0.0, 0.0, 0.0, 1.15)];
    let guns: &[(&str, f32, f32, f32, f32)] = &[("precise", 0.0, -3.0, 0.0, 1.0), ("lightweight", 0.0, 0.0, -0.1, 1.0)];
    (if gun { guns } else { melee })
        .iter()
        .map(|&(name, reach, spread_degrees, delay, power)| modforge::affix::Affix {
            name: name.to_string(),
            reach,
            spread_degrees,
            delay,
            power,
        })
        .collect()
}

/// The quality tiers a found item rolls (design.md "Loot randomization",
/// authority.md "Quality": every item rolls a tier, rarer is better), best
/// first; anything else is the base tier. The names are modforge
/// quality's; the odds (per mille), how much harder each hits, and the
/// least danger of a spot it is found at (authority.md "LootTable": no
/// top tier from a safe place) are Claude's, for the operator to change.
pub static QUALITY: std::sync::LazyLock<Vec<modforge::quality::Tier>> = std::sync::LazyLock::new(|| {
    [("legendary", 5, 1.6, 4), ("epic", 20, 1.4, 3), ("rare", 75, 1.25, 2), ("uncommon", 150, 1.1, 1)]
        .into_iter()
        .map(|(name, per_mille, power, min_danger)| modforge::quality::Tier {
            name: name.to_string(),
            per_mille,
            power,
            min_danger,
        })
        .collect()
});

/// What Randy Random rolls while the player walks topside (design.md "The
/// unknown" rule 1, "The first hour" rule 4; todo 10h1). A roll every 100 m
/// walked, hitting one in three: Claude's numbers, for the operator to
/// change. The radio speaks only in a world whose question is the radio.
pub fn encounter_table(seed: u64) -> modforge::randy::EncounterTable {
    let mut encounters = vec![
        modforge::randy::Encounter { name: STRANGER_CROSSES, weight: 1 },
        modforge::randy::Encounter { name: BAND_PASSES, weight: 1 },
    ];
    if starting_question(seed).name == RADIO_QUESTION {
        encounters.push(modforge::randy::Encounter { name: RADIO_SPEAKS, weight: 1 });
    }
    modforge::randy::EncounterTable {
        every_metres: 100.0,
        chance_in_100: 33,
        encounters,
    }
}

/// The radio's clicks become a voice (todo 10h3): once, the radio's words
/// become what the voice says. The words are Claude's, for the operator
/// to change.
pub const RADIO_SPEAKS: &str = "the radio speaks";
pub const RADIO_QUESTION: &str = "the radio";
pub const RADIO_ITEM: &str = "radio";
pub const RADIO_VOICE: &str = "Static. Two clicks. Then a voice, faint: \"...anyone still down there... don't trust the sky... the clicks are the count...\" Then nothing.";

/// A stranger crosses the player's path and leaves.
pub const STRANGER_CROSSES: &str = "a stranger crosses";

/// A band passes in the distance (todo 10h2).
pub const BAND_PASSES: &str = "a band passes";

/// The small world's sites (operator, 2026-09-26: "20 monuments of
/// varying sizes"): every kind but the ruined city, from large to minor,
/// each at a fixed place clear of every bunker's ground.
pub const SMALL_SITES: [(&str, [f32; 2]); 21] = [
    // Large.
    ("launch site", [-300.0, 300.0]),
    ("power plant", [300.0, 300.0]),
    ("airfield", [-300.0, -300.0]),
    // Medium.
    ("military outpost", [150.0, 180.0]),
    ("research station", [-150.0, 180.0]),
    ("train yard", [150.0, -170.0]),
    ("harbor", [-150.0, -170.0]),
    ("sewer", [420.0, 150.0]),
    // Small.
    ("junkyard", [-420.0, 150.0]),
    ("junkyard", [-420.0, -150.0]),
    ("roadside stop", [0.0, 180.0]),
    ("roadside stop", [420.0, -150.0]),
    // Minor.
    ("wreck", [-150.0, 0.0]),
    ("wreck", [420.0, 420.0]),
    ("campsite", [0.0, -150.0]),
    ("campsite", [-420.0, 420.0]),
    ("supply drop", [150.0, 60.0]),
    ("supply drop", [420.0, -420.0]),
    ("body", [-80.0, -90.0]),
    ("body", [-420.0, -420.0]),
    // Medium triangles. Across the way from the bunker door to the launch
    // site's gantry, 180 m out: the gantry is seen over it from the door
    // and lost behind it for the last stretch before it (design.md "The
    // first hour": "a medium triangle between you and it so it vanishes as
    // you walk"). Where it stands is Claude's, for the operator to change.
    ("ridge", [-127.0, 127.0]),
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

/// The gap through the trees from the player's bunker toward the launch
/// site's gantry (SMALL_SITES), so the door opens on the big thing on the
/// horizon; past its end the trees hide it as the player walks (design.md
/// "The first hour"). Its length and width are Claude's, for the operator
/// to change.
pub const DOOR_GAP: (Vec2, f32, f32) = (Vec2::new(-300.0, 300.0), 100.0, 10.0);

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
        .map(|at| Clearing {
            at,
            radius: BUNKER_CLEAR,
            gap: (at == BUNKER_AT && kind == WorldKind::Small).then_some(DOOR_GAP),
        })
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

/// Where the bunker people stand in the bunker's plan (x, z), one each, in
/// the surface room clear of the hatch (`HATCH_MIN`, x 1 to 3).
pub const BUNKER_PEOPLE_AT: [[f32; 2]; BUNKER_PEOPLE] =
    [[-2.0, -1.0], [-0.5, -1.0], [-2.0, 1.0], [-0.5, 1.0], [-1.0, 2.5]];

/// The people of the player's bunker besides the player (operator,
/// 2026-09-27: "two people. me and Roxanne"), each named, where they
/// stand in the bunker's plan (x, z), and their fear of not having enough
/// (0 to 1): Roxanne stores past what is needed (docs/episodes/, "Too much
/// water").
pub const PLAYER_BUNKER_PEOPLE: [(&str, [f32; 2], f32); 1] = [("Roxanne", [-2.2, 3.0], 1.0)];

/// A bunker's store (design.md: factions have real stores): one box in
/// the bunker's plan (x, z) and what is in it, with the water in each
/// (design.md "Water is a fluid", operator 2026-10-03: a water barrel
/// holding 10 L and 2 bottles of 5 L; the bottles start empty, Claude's
/// reading, for the operator to change). Tuning.
pub const BUNKER_STORE_AT: [f32; 2] = [-2.0, -3.0];
pub const BUNKER_STORE: [(&str, u32, f32); 4] = [("canned food", 10, 0.0), ("water barrel", 1, 10.0), ("5 L water bottle", 1, 0.0), ("5 L water bottle", 1, 0.0)];

/// A bunker is short of food when its store holds less than this many days
/// of it for everyone living there: the starting bunker's three days
/// (design.md "The bunker"). Tuning.
pub const BUNKER_SHORT_DAYS: f32 = 3.0;

/// What a bunker needs in water: 10 days of everyone's drinking (operator,
/// 2026-10-02; docs/episodes/, "Too much water": past it is too much). One not
/// afraid of not having enough stocks to it, the most afraid to twice it.
pub const BUNKER_WATER_DAYS: f32 = 10.0;

/// A store another place draws from reads dry holding less than this share
/// of what it held when the drawing began (todo 11ad, docs/episodes/:
/// under a quarter).
pub const DRY_BELOW: f32 = 0.25;

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
    // Their weapon is a handle and its head (design.md "What 2D changes").
    let person = |name: &str, faction: Option<&str>, behaviour, max_health, weapon: &str, head: &[&str]| {
        ActorDef {
            name: name.to_string(),
            faction: faction.map(String::from),
            behaviour,
            max_health,
            protection: Default::default(),
            weapon: Some(weapon.to_string()),
            weapon_layers: head.iter().map(|h| h.to_string()).collect(),
            sight: 25.0,
            reach: 1.8,
        }
    };
    // The player: a bunker person of the player's bunker, under human
    // control, starting with nothing in hand.
    reg.register(ActorDef {
        weapon: None,
        ..person("player", Some(PLAYER_FACTION), Behaviour::Guard, 100.0, "", &[])
    })
    .expect("actor registered once");
    for def in [
        person("bunker person", None, Behaviour::Guard, 100.0, "pipe", &[]),
        person("scavenger", Some(TOPSIDE_HOSTILES), Behaviour::Hunter, 60.0, "pipe", &[]),
        person("raider", Some(TOPSIDE_HOSTILES), Behaviour::Hunter, 100.0, "wooden handle", &["axe head"]),
        person("lookout", Some(TOPSIDE_HOSTILES), Behaviour::Guard, 80.0, "long pole", &["spear point"]),
        person("wanderer", None, Behaviour::Guard, 70.0, "pipe", &[]),
        // A wild animal (design.md "Bunker people and topside people":
        // "Hostiles attack: wild animals"), hunting with its teeth. Its
        // health and reach are Claude's, for the operator to change.
        ActorDef { reach: 1.4, ..person("wolf", Some(TOPSIDE_HOSTILES), Behaviour::Hunter, 50.0, "teeth", &[]) },
        // A turret (todo 11k): an actor that never moves, guarding where
        // it is set with a pipe shotgun; its faction the placer's. Health
        // and sight Claude's.
        ActorDef { sight: 20.0, reach: 12.0, ..person(TURRET, None, Behaviour::Guard, 150.0, "pipe shotgun", &["pipe barrel", "wooden grip", "wooden stock"]) },
        // The car (todo 57): an actor that never thinks for itself; its
        // driver fills its queue. Its health Claude's.
        ActorDef { weapon: None, sight: 0.0, reach: 0.0, ..person(CAR, None, Behaviour::Guard, 400.0, "", &[]) },
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
        // A handle with no head hits with the handle.
        melee("pipe swing", 20.0, DamageType::Blunt),
        melee("handle swing", 15.0, DamageType::Blunt),
        melee("pole swing", 12.0, DamageType::Blunt),
        // A head decides the damage and its kind. Tuning.
        melee("nail hit", 26.0, DamageType::Pierce),
        melee("blade cut", 26.0, DamageType::Slash),
        melee("axe chop", 30.0, DamageType::Slash),
        melee("spear thrust", 28.0, DamageType::Pierce),
        melee("sign cut", 24.0, DamageType::Slash),
        // A wild animal's teeth (todo 13w). Claude's, for the operator to
        // change.
        melee("bite", 15.0, DamageType::Pierce),
        pellet,
        fall,
        wasting("starvation"),
        wasting("dehydration"),
        // Temperature and radiation (todo 20): hurting like hunger.
        wasting("cold"),
        wasting("heat"),
        wasting("radiation"),
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
        // The burner anomaly (`ANOMALY`): one hit kills whoever walks in,
        // no armor helps.
        DamageDef {
            name: ANOMALY.to_string(),
            amount: 1_000_000.0,
            kind: DamageType::Blunt,
            knockback: 0.0,
            ignores_armor: true,
            self_scale: 1.0,
            falloff: Falloff::NONE,
        },
        // The control plane's `kill` op: one hit that kills anyone, no
        // armor helps, so a test can bring a death now.
        DamageDef {
            name: "kill".to_string(),
            amount: 1_000_000.0,
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

/// One question the bunker's starting room asks, and its answer placed
/// within the first hour's walk (design.md "The first hour": "The bunker's
/// starting room carries one rolled question from a registry of
/// questions ... each with its answer placed by history somewhere within
/// the first hour's walk").
pub struct Question {
    pub name: &'static str,
    /// The kind of site its answer lies at: the one of that kind nearest
    /// the bunker.
    pub answer_at: &'static str,
    /// The words of the answer, one of these, signed by the same name as
    /// the starting note. Each asks the next question (design.md "The
    /// first hour": the first trace "answers the bunker's question and
    /// asks the next one"; todo 10g): {next} is the nearest other site to
    /// the answer's, by what it is, {way} where it lies from there.
    pub answers: &'static [&'static str],
    /// The fourth thing in the room that asks it, and what using it shows
    /// (design.md: "The three things in the room (bed, bench, food) stay;
    /// the question is a fourth"); none when the starting note asks it.
    pub in_room: Option<(&'static str, &'static str)>,
}

/// The bunker's questions (design.md "The first hour": "the bolted door,
/// the note, the radio, and more"), each answered at the nearest body by
/// a note in the starting note's name (the first trace "carries the name
/// from the note, or the note's handwriting"). The words are Claude's,
/// for the operator to change.
pub const QUESTIONS: [Question; 3] = [
    Question {
        name: "the note",
        answer_at: "body",
        answers: &[
            "couldn't wait any longer. went out for water. what I found is at the {next}, {way} of here. if you find this, go back",
            "I told you not to come out. it isn't the people. it's the sky. ask at the {next}, {way} of here. they saw it too",
            "three days was all I could leave you. the rest is at the {next}, {way} of here. I'm sorry",
        ],
        in_room: None,
    },
    Question {
        name: "the bolted door",
        answer_at: "body",
        answers: &[
            "I bolted you in so you'd stay. you scratched at it all night. I'm sorry. I'm going on to the {next}, {way} of here",
            "the scratches aren't yours. something wanted in. I bolted it and ran. it came from the {next}, {way} of here",
        ],
        in_room: Some(("bolted door", "Bolted from this side. Scratches in the paint, low down.")),
    },
    Question {
        name: "the radio",
        answer_at: "body",
        answers: &[
            "two clicks means I'm still out here. if the clicks stop, don't come looking. I'll be at the {next}, {way} of here",
            "the clicks aren't me. they started after the sky changed. they come loudest at the {next}, {way} of here",
        ],
        in_room: Some(("radio", "Static. Then two clicks, and nothing.")),
    },
];

/// The question this world's bunker asks: same seed, same question.
pub fn starting_question(seed: u64) -> &'static Question {
    let mut roll = Roll::new(seed ^ 0x5155_4553);
    roll.pick(&QUESTIONS)
}

/// The answer to this world's question, signed as the starting note is,
/// its {slots} filled; None when they cannot be.
pub fn question_answer(seed: u64, slots: &[(&str, &str)]) -> Option<Note> {
    let ways: Vec<String> = starting_question(seed).answers.iter().map(|a| a.to_string()).collect();
    Some(Note {
        text: modforge::storyteller::say(&ways, slots, seed, 0x414e_5357)?,
        signed: starting_note(seed).signed,
    })
}

/// The sites that are no place to go to: the medium triangles, there only
/// to hide what is past them (design.md "The triangle rule").
pub const TRIANGLES: [&str; 1] = ["ridge"];

/// Which way `to` lies from `from`, as a person says it: north, south-east
/// (north is +y, as the map is drawn: minimap.rs).
pub fn compass(from: Vec2, to: Vec2) -> &'static str {
    const WAYS: [&str; 8] = ["east", "north-east", "north", "north-west", "west", "south-west", "south", "south-east"];
    let d = to - from;
    let eighth = (d.y.atan2(d.x) / std::f32::consts::FRAC_PI_4).round().rem_euclid(8.0) as usize;
    WAYS[eighth]
}

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

/// A source's def: what one use of it fills.
fn source(name: &str, hunger: f32, thirst: f32) -> ItemDef {
    ItemDef {
        name: name.to_string(),
        // Never carried, so never shown in the item information panel.
        description: String::new(),
        unique: false,
        kind: ItemKind::Source,
        max_stack: 1,
        quality_siblings: 1,
        combat: None,
        food: Some(FoodStats { hunger, thirst, health: 0.0 }),
        storage: None,
        armor: None,
        good_for: Default::default(),
        picture: None,
        layer_slots: Vec::new(),
        breaks_when_struck: false,
        holds_litres: None,
    }
}

/// A container of water (design.md "Water is a fluid"): it holds up to
/// `litres`, is drunk from a mouthful at a time, filled at a source and
/// poured from (modforge fluid). One to a slot: what each holds is its own.
fn container(name: &str, description: &str, litres: f32) -> ItemDef {
    ItemDef {
        name: name.to_string(),
        description: description.to_string(),
        unique: false,
        kind: ItemKind::Container,
        max_stack: 1,
        quality_siblings: 1,
        combat: None,
        food: None,
        storage: None,
        armor: None,
        good_for: Default::default(),
        picture: None,
        layer_slots: Vec::new(),
        breaks_when_struck: false,
        holds_litres: Some(litres),
    }
}

pub fn register_items(reg: &mut ItemRegistry) {
    let building = |name: &str, description: &str, kind: ItemKind, max_stack: u32, storage: Option<usize>| ItemDef {
        name: name.to_string(),
        description: description.to_string(),
        unique: false,
        kind,
        max_stack,
        quality_siblings: 1,
        combat: None,
        food: None,
        storage,
        armor: None,
        good_for: Default::default(),
        picture: None,
        layer_slots: Vec::new(),
        breaks_when_struck: false,
        holds_litres: None,
    };
    let items = [
        ItemDef {
            name: "note".to_string(),
            description: "Someone's words on a scrap of paper.".to_string(),
            unique: false,
            kind: ItemKind::Note,
            max_stack: 1,
            quality_siblings: 1,
            combat: None,
            food: None,
            storage: None,
            armor: None,
            good_for: Default::default(),
            picture: None,
            layer_slots: Vec::new(),
            breaks_when_struck: false,
            holds_litres: None,
        },
        // The room's question when it is not the note (QUESTIONS): read
        // as a note is, by using it.
        ItemDef {
            name: "bolted door".to_string(),
            description: "A door, bolted shut, scratched.".to_string(),
            unique: false,
            kind: ItemKind::Note,
            max_stack: 1,
            quality_siblings: 1,
            combat: None,
            food: None,
            storage: None,
            armor: None,
            good_for: Default::default(),
            picture: None,
            layer_slots: Vec::new(),
            breaks_when_struck: false,
            holds_litres: None,
        },
        ItemDef {
            name: "radio".to_string(),
            description: "A radio that clicks.".to_string(),
            unique: false,
            kind: ItemKind::Note,
            max_stack: 1,
            quality_siblings: 1,
            combat: None,
            food: None,
            storage: None,
            armor: None,
            good_for: Default::default(),
            picture: None,
            layer_slots: Vec::new(),
            breaks_when_struck: false,
            holds_litres: None,
        },
        ItemDef {
            name: "canned food".to_string(),
            description: "A dented can of food from before. Eat it to fill your hunger.".to_string(),
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
            picture: None,
            layer_slots: Vec::new(),
            breaks_when_struck: false,
            holds_litres: None,
        },
        // What food kept too long turns into (todo Survive 3): it barely
        // fills and hurts. Its numbers are Claude's.
        ItemDef {
            name: ROTTEN_FOOD.to_string(),
            description: "Food kept too long. It has gone bad.".to_string(),
            unique: false,
            kind: ItemKind::Food,
            max_stack: 10,
            quality_siblings: 1,
            combat: None,
            food: Some(FoodStats {
                hunger: 10.0,
                thirst: 0.0,
                health: -10.0,
            }),
            storage: None,
            armor: None,
            good_for: Default::default(),
            picture: None,
            layer_slots: Vec::new(),
            breaks_when_struck: false,
            holds_litres: None,
        },
        // Water bottles come in sizes, each its own def (operator,
        // 2026-10-03); a bunker's barrel holds 200 L.
        container("water bottle", "A plastic bottle for water, 1 litre. Drink from it; fill it at a tap, a well or a river.", 1.0),
        container("5 L water bottle", "A big plastic bottle for water, 5 litres. Drink from it; fill it at a tap, a well or a river.", 5.0),
        container("water barrel", "A bunker's water barrel, 200 litres. Pour water into it; drink or fill a bottle from it.", 200.0),
        // The sources (design.md: a livable world, rivers drinkable,
        // wells scattered, food sources): eaten or drunk from where they
        // stand, a mouthful a use, never used up. Tuning.
        source("well", 0.0, 50.0),
        // Dirty water works but makes you sick (todo Survive 4): each
        // mouthful of river water hurts a little. The 3 is Claude's.
        ItemDef {
            food: Some(FoodStats { hunger: 0.0, thirst: 50.0, health: -3.0 }),
            ..source("river", 0.0, 50.0)
        },
        // Food heals, each food its own amounts (todo Survive 2): berries
        // less hunger and less health than a can. The 2 is Claude's.
        ItemDef {
            food: Some(FoodStats { hunger: 20.0, thirst: 0.0, health: 2.0 }),
            ..source("berry bush", 20.0, 0.0)
        },
        // Made at the bench from cloth; heals far faster than food, a
        // reason to craft before going out (todo 10ca). Tuning.
        ItemDef {
            name: "bandage".to_string(),
            description: "A strip of clean cloth. Use it to heal your wounds.".to_string(),
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
            picture: None,
            layer_slots: Vec::new(),
            breaks_when_struck: false,
            holds_litres: None,
        },
        // Building (todo 9f, Rust's building plan, hammer and tool
        // cupboard; the materials each grade costs, `grade_material`).
        // Descriptions and the cupboard's 6 slots Claude's.
        building(BUILDING_PLAN, "Rolled paper. Hold it and strike to build a twig piece; use it to pick the next piece.", ItemKind::Tool, 1, None),
        building(HAMMER, "A claw hammer. Strike a piece to upgrade it to its next grade.", ItemKind::Tool, 1, None),
        building(TOOL_CUPBOARD, "A cupboard of tools. Set it down: only your group builds near it, and its upkeep keeps your pieces from decay.", ItemKind::Deployable, 1, Some(6)),
        building(WOOD, "Split wood, for building.", ItemKind::Material, 100, None),
        building(STONE, "Broken stone, for building.", ItemKind::Material, 100, None),
        building(TURRET, "A shotgun on a stand. Set it down and it guards the spot, loaded with your shells.", ItemKind::Deployable, 1, None),
        // Power (todo 29; descriptions and the 4 slots Claude's).
        building(GENERATOR, "A fuel generator. Fill it with fuel and it powers what is wired near it.", ItemKind::Deployable, 1, Some(4)),
        building(FUEL, "A can of fuel, for a generator.", ItemKind::Material, 20, None),
        // The silo (todo 30; descriptions Claude's).
        building(SHOVEL, "A shovel. Strike the deepest floor of your bunker to dig a level down.", ItemKind::Tool, 1, None),
        building("crafting bench", "A bench folded for carrying. Set it down and craft at it.", ItemKind::Deployable, 1, None),
        building("research station", "A research desk folded for carrying. Set it down and research at it, with power.", ItemKind::Deployable, 1, None),
        ItemDef {
            name: "storage box".to_string(),
            description: "A wooden box to keep things in. Set it down and fill it.".to_string(),
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
            picture: None,
            layer_slots: Vec::new(),
            breaks_when_struck: false,
            holds_litres: None,
        },
        // Thrown ahead to set an anomaly off (todo 10i2; design.md "Maze,
        // mined": the bolt).
        ItemDef {
            name: BOLT.to_string(),
            description: "A steel bolt. Throw it ahead and watch where it lands.".to_string(),
            unique: false,
            kind: ItemKind::Tool,
            max_stack: 20,
            quality_siblings: 1,
            combat: None,
            food: None,
            storage: None,
            armor: None,
            good_for: Default::default(),
            picture: None,
            layer_slots: Vec::new(),
            breaks_when_struck: false,
            holds_litres: None,
        },
        ItemDef {
            name: "scrap".to_string(),
            description: "Bent metal and screws, for making things.".to_string(),
            unique: false,
            kind: ItemKind::Material,
            max_stack: 20,
            quality_siblings: 1,
            combat: None,
            food: None,
            storage: None,
            armor: None,
            good_for: Default::default(),
            picture: None,
            layer_slots: Vec::new(),
            breaks_when_struck: false,
            holds_litres: None,
        },
        ItemDef {
            name: "cloth".to_string(),
            description: "A roll of old cloth, for making things.".to_string(),
            unique: false,
            kind: ItemKind::Material,
            max_stack: 20,
            quality_siblings: 1,
            combat: None,
            food: None,
            storage: None,
            armor: None,
            good_for: Default::default(),
            picture: None,
            layer_slots: Vec::new(),
            breaks_when_struck: false,
            holds_litres: None,
        },
        ItemDef {
            name: "pipe".to_string(),
            description: "A length of steel pipe to swing. A head can be fixed to it.".to_string(),
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
            picture: None,
            // A handle (design.md "What 2D changes"): the handle decides
            // reach and speed, the head it takes decides the damage.
            layer_slots: vec!["head".to_string()],
            breaks_when_struck: false,
            holds_litres: None,
        },
        // A wolf's teeth: the weapon it is born with (todo 13w). Its delay
        // and reach are Claude's, for the operator to change.
        ItemDef {
            name: "teeth".to_string(),
            description: "A wild animal's teeth.".to_string(),
            unique: false,
            kind: ItemKind::Weapon,
            max_stack: 1,
            quality_siblings: 1,
            combat: Some(CombatStats {
                damage: "bite".to_string(),
                delay: 0.8,
                reach: 1.4,
                pellets: 1,
                spread_degrees: 0.0,
                ammo: None,
            }),
            food: None,
            storage: None,
            armor: None,
            good_for: Default::default(),
            picture: None,
            layer_slots: Vec::new(),
            breaks_when_struck: false,
            holds_litres: None,
        },
        ItemDef {
            name: "wooden handle".to_string(),
            description: "A short wooden handle to swing. A head can be fixed to it.".to_string(),
            unique: false,
            kind: ItemKind::Weapon,
            max_stack: 1,
            quality_siblings: 3,
            combat: Some(CombatStats {
                damage: "handle swing".to_string(),
                delay: 0.7,
                reach: 1.8,
                pellets: 1,
                spread_degrees: 0.0,
                ammo: None,
            }),
            food: None,
            storage: None,
            armor: None,
            good_for: Default::default(),
            picture: None,
            layer_slots: vec!["head".to_string()],
            breaks_when_struck: false,
            holds_litres: None,
        },
        ItemDef {
            name: "long pole".to_string(),
            description: "A long pole that keeps them at a distance. A head can be fixed to it.".to_string(),
            unique: false,
            kind: ItemKind::Weapon,
            max_stack: 1,
            quality_siblings: 3,
            combat: Some(CombatStats {
                damage: "pole swing".to_string(),
                delay: 0.9,
                reach: 2.8,
                pellets: 1,
                spread_degrees: 0.0,
                ammo: None,
            }),
            food: None,
            storage: None,
            armor: None,
            good_for: Default::default(),
            picture: None,
            layer_slots: vec!["head".to_string()],
            breaks_when_struck: false,
            holds_litres: None,
        },
        // Rust's pipe shotgun: one shell, six pellets, wide spread,
        // instant out to 40 m (the hybrid's instant part).
        ItemDef {
            name: "pipe shotgun".to_string(),
            description: "A shotgun made from pipe. Fires shotgun shells; its parts can be swapped.".to_string(),
            unique: false,
            kind: ItemKind::Weapon,
            max_stack: 1,
            quality_siblings: 3,
            // The body alone; its parts add to these (register_layers).
            combat: Some(CombatStats {
                damage: "shotgun pellet".to_string(),
                delay: 1.2,
                reach: 20.0,
                pellets: 6,
                spread_degrees: 10.0,
                ammo: Some("shotgun shell".to_string()),
            }),
            food: None,
            storage: None,
            armor: None,
            good_for: Default::default(),
            picture: None,
            // A gun (design.md "What 2D changes"): the body takes a
            // barrel, a grip, a stock, and a sight.
            layer_slots: ["barrel", "grip", "stock", "sight"].map(String::from).to_vec(),
            breaks_when_struck: false,
            holds_litres: None,
        },
        ItemDef {
            name: "shotgun shell".to_string(),
            description: "Ammunition for the pipe shotgun.".to_string(),
            unique: false,
            kind: ItemKind::Material,
            max_stack: 20,
            quality_siblings: 1,
            combat: None,
            food: None,
            storage: None,
            armor: None,
            good_for: Default::default(),
            picture: None,
            layer_slots: Vec::new(),
            breaks_when_struck: false,
            holds_litres: None,
        },
        // Worn in the chest slot; guards the chest only.
        ItemDef {
            name: "road sign vest".to_string(),
            description: "Road signs strapped over the chest. Wear it for protection.".to_string(),
            unique: false,
            kind: ItemKind::Tool,
            max_stack: 1,
            quality_siblings: 3,
            combat: None,
            food: None,
            storage: None,
            armor: Some(Armor { amount: 40.0, area: modforge::combat::BodyArea::Chest }),
            good_for: Default::default(),
            picture: None,
            layer_slots: Vec::new(),
            breaks_when_struck: false,
            holds_litres: None,
        },
        // A helmet (todo Fight 8): worn armor that guards only the head.
        // Its amount is Claude's, for the operator to change.
        ItemDef {
            name: "helmet".to_string(),
            description: "A dented helmet. Wear it to guard your head.".to_string(),
            unique: false,
            kind: ItemKind::Tool,
            max_stack: 1,
            quality_siblings: 3,
            combat: None,
            food: None,
            storage: None,
            armor: Some(Armor { amount: 40.0, area: modforge::combat::BodyArea::Head }),
            good_for: Default::default(),
            picture: None,
            layer_slots: Vec::new(),
            breaks_when_struck: false,
            holds_litres: None,
        },
    ];
    // What everyone wears (WORN): plain clothes, wearable and guarding
    // nothing.
    let clothes = WORN.iter().map(|name| ItemDef {
        name: name.to_string(),
        description: format!("Plain {name}. Worn."),
        unique: false,
        kind: ItemKind::Tool,
        max_stack: 1,
        quality_siblings: 1,
        combat: None,
        food: None,
        storage: None,
        armor: Some(Armor { amount: 0.0, area: Default::default() }),
        good_for: Default::default(),
        picture: None,
        layer_slots: Vec::new(),
        breaks_when_struck: false,
        holds_litres: None,
    });
    let items = items.into_iter().chain(clothes).collect::<Vec<_>>();
    // With every episode's own items (each episode's file).
    for mut item in items.into_iter().chain(episodes().into_iter().flat_map(|e| e.items)) {
        // Every item is drawn with the picture named after it, rendered
        // by scripts/models.py.
        item.picture = Some(item.name.clone());
        reg.register(item).expect("item defs must not collide");
    }
}

const CONCRETE: &[Rgb] = &[[0.45, 0.45, 0.47], [0.5, 0.48, 0.45], [0.38, 0.4, 0.42]];
const WOOD_COLOURS: &[Rgb] = &[[0.45, 0.32, 0.18], [0.55, 0.45, 0.2], [0.35, 0.25, 0.15]];
const STEEL: &[Rgb] = &[[0.3, 0.32, 0.35], [0.25, 0.3, 0.4], [0.5, 0.2, 0.15]];
const CLEAN: &[Rgb] = &[[0.8, 0.82, 0.85], [0.6, 0.7, 0.75], [0.3, 0.5, 0.6]];

/// The model a wall tile is drawn with, by what it is made of (design.md
/// "Walls and floors by what they are made of"), read from its colour:
/// the bunker's concrete and the concrete palette are concrete, the wood
/// palette wood, the stone palette stone; the rest are the plain wall.
pub fn wall_model(color: Rgb) -> &'static str {
    if color == CONCRETE_WALL || CONCRETE.contains(&color) {
        "wall concrete"
    } else if WOOD_COLOURS.contains(&color) {
        "wall wood"
    } else if STONE_COLOURS.contains(&color) {
        "wall stone"
    } else {
        "wall"
    }
}

/// Stone buildings' colours (todo 15d2; Claude's: the lighthouse is
/// built of stone, for the operator to change).
const STONE_COLOURS: &[Rgb] = &[[0.55, 0.53, 0.5], [0.48, 0.47, 0.45], [0.6, 0.58, 0.52]];

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
        Spec { name: "guard tower", size: Small, columns: (1, 1), rows: (1, 1), floors: (3, 5), basements: (0, 0), width: (3.5, 4.0), length: (5.0, 6.0), height: (2.8, 3.2), windows: 800, clutter: 200, lights: 600, damage: 100, carve: 0, palette: WOOD_COLOURS},
        Spec { name: "shack", size: Small, columns: (1, 1), rows: (1, 1), floors: (1, 1), basements: (0, 0), width: (4.0, 6.0), length: (4.0, 6.0), height: (2.5, 3.0), windows: 400, clutter: 500, lights: 800, damage: 350, carve: 0, palette: WOOD_COLOURS},
        Spec { name: "lighthouse", size: Small, columns: (1, 1), rows: (1, 1), floors: (5, 8), basements: (0, 0), width: (4.0, 5.0), length: (5.0, 6.0), height: (2.8, 3.2), windows: 500, clutter: 200, lights: 500, damage: 100, carve: 0, palette: STONE_COLOURS},
        // Medium: two to six rooms, one to three floors.
        Spec { name: "warehouse", size: Medium, columns: (2, 3), rows: (1, 2), floors: (1, 1), basements: (0, 0), width: (8.0, 12.0), length: (10.0, 16.0), height: (4.5, 6.0), windows: 300, clutter: 800, lights: 600, damage: 200, carve: 200, palette: STEEL },
        Spec { name: "office building", size: Medium, columns: (2, 3), rows: (1, 2), floors: (2, 4), basements: (0, 1), width: (5.0, 6.0), length: (6.0, 8.0), height: (2.8, 3.4), windows: 800, clutter: 600, lights: 400, damage: 150, carve: 200, palette: CONCRETE },
        Spec { name: "workshop", size: Medium, columns: (2, 3), rows: (1, 1), floors: (1, 1), basements: (0, 0), width: (6.0, 8.0), length: (7.0, 10.0), height: (3.5, 4.5), windows: 400, clutter: 900, lights: 600, damage: 200, carve: 0, palette: STEEL },
        Spec { name: "barracks", size: Medium, columns: (2, 3), rows: (1, 2), floors: (1, 2), basements: (0, 0), width: (5.0, 6.0), length: (7.0, 9.0), height: (2.8, 3.2), windows: 500, clutter: 700, lights: 500, damage: 150, carve: 200, palette: WOOD_COLOURS},
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
        Spec { name: "dock building", size: Large, columns: (4, 5), rows: (3, 3), floors: (2, 2), basements: (0, 0), width: (5.0, 7.0), length: (6.0, 9.0), height: (3.5, 4.5), windows: 400, clutter: 700, lights: 200, damage: 300, carve: 150, palette: WOOD_COLOURS},
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
            // The launch site's gantry stands in its yard, the big thing
            // seen from the bunker door (design.md "The first hour").
            props: match name {
                "launch site" => vec![PropDef {
                    size: Vec3::new(6.6, 60.0, 6.6),
                    color: [0.55, 0.55, 0.58],
                    count: (1, 1),
                    radius: 0.0,
                    picture: Some("gantry".to_string()),
                    centred: false,
                }],
                _ => vec![],
            },
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
    // Each prop is drawn as its picture, rendered by scripts/models.py to
    // its size here.
    let prop = |picture: &str, size: [f32; 3], color: Rgb, count: (u32, u32), radius: f32| PropDef {
        size: Vec3::from(size),
        color,
        count,
        radius,
        picture: Some(picture.to_string()),
        centred: false,
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
                prop("car wreck", [4.2, 1.4, 1.9], [0.45, 0.2, 0.15], (1, 2), 4.0),
                prop("barrel", [0.6, 0.9, 0.6], [0.3, 0.3, 0.32], (0, 3), 5.0),
            ],
        ),
        minor(
            "campsite",
            1,
            "Camp",
            vec![
                prop("tent", [2.4, 1.6, 2.4], [0.35, 0.3, 0.2], (1, 2), 4.0),
                prop("fire pit", [1.0, 0.4, 1.0], [0.2, 0.2, 0.2], (1, 1), 2.5),
                prop("crate", [0.5, 0.5, 0.5], [0.4, 0.32, 0.2], (1, 3), 3.5),
            ],
        ),
        minor(
            "supply drop",
            1,
            "Drop",
            vec![
                prop("supply crate", [1.6, 1.2, 1.6], [0.3, 0.4, 0.3], (1, 1), 1.5),
                prop("parachute", [3.0, 0.1, 3.0], [0.6, 0.6, 0.62], (1, 1), 4.0),
            ],
        ),
        minor(
            "body",
            0,
            "Grave",
            vec![
                prop("grave", [1.8, 0.3, 0.6], [0.3, 0.28, 0.25], (1, 1), 1.5),
                prop("stone", [0.5, 0.5, 0.5], [0.4, 0.4, 0.38], (0, 2), 3.0),
            ],
        ),
        // A medium triangle (design.md "The triangle rule": "the hills,
        // ridges, and wall runs that hide what is past them"): one long
        // ridge, its size Claude's, for the operator to change.
        MonumentTypeDef {
            height: 10.0,
            ..minor(
                "ridge",
                0,
                "Ridge",
                // On the site's middle, its box behind it.
                vec![PropDef { centred: true, ..prop("ridge", [110.0, 10.0, 14.0], [0.42, 0.36, 0.28], (1, 1), 0.0) }],
            )
        },
        // The Labyrinth's gate (todo 11v): no buildings, a locked gate
        // across the corridor and a box behind it; turned across the
        // corridor where it stands (structure::spawn_monuments). Its
        // danger and spacing Claude's.
        MonumentTypeDef {
            spacing: 150.0,
            height: 6.0,
            ..minor(
                GATE,
                3,
                "Gate",
                vec![PropDef { centred: true, ..prop(GATE, [9.0, 6.0, 0.4], [0.3, 0.22, 0.18], (1, 1), 0.0) }],
            )
        },
    ];
    for monument in monuments {
        reg.register(monument, buildings)
            .expect("monument type defs must not collide or name unknown buildings");
    }
}

/// The sources every biome has (design.md: a livable world, wells
/// scattered across it, food sources): a well about every 100 m, a berry
/// bush about every 30 m. First in a biome's scatter, so they are rolled
/// before its trees. Tuning.
fn sources() -> [ScatterDef; 2] {
    [
        ScatterDef {
            size: Vec3::new(1.4, 1.0, 1.4),
            color: [0.45, 0.47, 0.5],
            density: 0.0016,
            picture: Some("well".to_string()),
            source: Some("well".to_string()),
        },
        ScatterDef {
            size: Vec3::new(1.0, 0.8, 1.0),
            color: [0.55, 0.15, 0.3],
            density: 0.018,
            picture: Some("berry bush".to_string()),
            source: Some("berry bush".to_string()),
        },
    ]
}

pub fn register_biomes(reg: &mut BiomeRegistry) {
    let biomes = [
        BiomeDef {
            name: "forest".to_string(),
            ground: [0.22, 0.38, 0.18],
            scatter: vec![
                sources()[0].clone(),
                sources()[1].clone(),
                ScatterDef {
                    size: Vec3::new(0.6, 5.0, 0.6),
                    color: [0.3, 0.22, 0.12],
                    // A forest (operator, 2026-09-26): a tree about every
                    // 5 m, so the crowns (4.6 m across) close up.
                    density: 0.6,
                    // scripts/models.py renders it.
                    picture: Some("tree".to_string()),
                    source: None,
                },
                ScatterDef {
                    size: Vec3::new(0.8, 0.5, 0.8),
                    color: [0.4, 0.4, 0.35],
                    density: 0.01,
                    // scripts/models.py renders it.
                    picture: Some("rock".to_string()),
                    source: None,
                },
            ],
            weather: weather_list(&[("clear", 10), ("rain", 6), ("fog", 4), ("cold snap", 1)]),
            monuments: vec![
                "roadside stop".to_string(),
                "research station".to_string(),
                "military outpost".to_string(),
                "campsite".to_string(),
                "body".to_string(),
                // Placed only where a world lists it (the Labyrinth).
                GATE.to_string(),
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
                sources()[0].clone(),
                sources()[1].clone(),
                ScatterDef {
                    size: Vec3::new(1.2, 0.6, 0.8),
                    color: [0.5, 0.42, 0.32],
                    density: 0.005,
                    // scripts/models.py renders it.
                    picture: Some("fallen log".to_string()),
                    source: None,
                },
            ],
            weather: weather_list(&[("dust storm", 1), ("clear", 10), ("heat wave", 4), ("radiation storm", 2), ("fog", 1)]),
            monuments: vec![
                "roadside stop".to_string(),
                "airfield".to_string(),
                "train yard".to_string(),
                "launch site".to_string(),
                "wreck".to_string(),
                "body".to_string(),
                // Placed only where a world lists it (the Labyrinth).
                GATE.to_string(),
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
                sources()[0].clone(),
                sources()[1].clone(),
                ScatterDef {
                    size: Vec3::new(2.0, 1.5, 1.5),
                    color: [0.45, 0.43, 0.4],
                    density: 0.008,
                    // scripts/models.py renders it.
                    picture: Some("broken wall".to_string()),
                    source: None,
                },
            ],
            weather: weather_list(&[("rain", 2), ("fog", 3), ("clear", 10), ("radiation storm", 3)]),
            monuments: vec![
                "ruined city".to_string(),
                "power plant".to_string(),
                "sewer".to_string(),
                "harbor".to_string(),
                "wreck".to_string(),
                "campsite".to_string(),
                "body".to_string(),
                // Placed only where a world lists it (the Labyrinth).
                GATE.to_string(),
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
        // grass with only wells and berry bushes on it (a livable world).
        // Its sites are placed by hand.
        BiomeDef {
            name: "grassland".to_string(),
            ground: [0.3, 0.45, 0.22],
            scatter: sources().to_vec(),
            weather: weather_list(&[("clear", 12), ("rain", 4), ("cold snap", 2), ("heat wave", 2)]),
            monuments: vec![],
            // Claude's (todo 32), for the operator to change.
            npcs: vec!["wanderer".to_string()],
            wildlife: vec!["wolf".to_string()],
            harvest: vec!["cloth".to_string()],
        },
    ];
    for biome in biomes {
        reg.register(biome).expect("biome defs must not collide");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every episode file reads through the one loader, in the order
    /// episodes.yaml lists, The Tap first.
    #[test]
    fn the_episodes_read_from_their_files() {
        let mut reg = EpisodeRegistry::default();
        register_episodes(&mut reg);
        let names: Vec<&str> = reg.all().iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["The Tap", "The Fever", "The Other You", "Nobody Dies", "The Question"]);
    }
}
