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
use modforge::town::{Town, TownDef};
use modforge::house::HouseDef;
use modforge::actor::{ActorDef, ActorRegistry, BodyLook, LookDef};
use modforge::biome::{BiomeDef, BiomeRegistry};
use modforge::combat::{DamageDef, DamageRegistry};
use modforge::faction::{FactionDef, FactionKind, FactionRegistry};
use modforge::item::{Armor, ItemDef, ItemKind, ItemRegistry, LayerDef, LayerRegistry, Note};
use modforge::monument::Roll;
use modforge::reality::{RealityDef, RealityRegistry};
use modforge::storyteller::{EpisodeDef, EpisodeRegistry, LineDef};
use modforge::monument::{BuildingRegistry, BuildingTypeDef, MonumentRegistry, MonumentTypeDef};
use modforge::crafting::{RecipeDef, RecipeRegistry, StationKind};
use modforge::loot::LootTable;
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

/// Every recipe, as data (modforge crafting), from `RECIPES_FILE`.
#[derive(Resource)]
pub struct RecipeRegistryRes(pub RecipeRegistry);

/// Where the recipes are: content/recipes.yaml, its `recipes` a list of
/// modforge `crafting::RecipeDef`, each naming only what it has.
pub const RECIPES_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/recipes.yaml");

impl Default for RecipeRegistryRes {
    fn default() -> Self {
        let mut reg = RecipeRegistry::default();
        for def in read_list::<RecipeDef>(RECIPES_FILE, "recipes") {
            reg.register(def).expect("recipe registered once");
        }
        Self(reg)
    }
}

/// Where the research topics are: content/tech.yaml, its `topics` a list
/// of modforge `tech::TopicDef`.
pub const TECH_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tech.yaml");

/// The research topics (todo 28, modforge tech), from `TECH_FILE`.
pub fn tech_tree() -> modforge::tech::TechTree {
    modforge::tech::TechTree { topics: read_list(TECH_FILE, "topics") }
}

/// Where a monument's loot is: content/loot.yaml, its `loot` a modforge
/// `loot::LootTable`.
pub const LOOT_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/loot.yaml");

/// What a monument's box can hold, from `LOOT_FILE`.
pub fn loot_table() -> LootTable {
    read(LOOT_FILE, "loot")
}

/// One game day in real seconds (the 3D game's Valheim-scale pacing;
/// tune later). A full stomach lasts one, a full canteen half of one,
/// rest one awake.
pub const GAME_DAY_SECS: f32 = 1200.0;

/// The days of history rolled before the player wakes (todo History 1;
/// operator, 2026-11-23: "a history of like 1 week beforehand"). Rolled as
/// events, never played: the clock stands this far in when they wake.
pub const HISTORY_DAYS: u32 = 7;
/// The history's span in game seconds: the clock at waking.
pub const HISTORY_SECS: f32 = HISTORY_DAYS as f32 * GAME_DAY_SECS;
/// Of 1000, the chance that a person of this kind, meeting someone of
/// another group during the history, comes to blows (operator,
/// 2026-11-23: raiders are "by definition hostile. Scavengers might be
/// hostile if they get into a fight, and wolves... might be hostile if
/// there's something. ...all of those is a chance for hostility to
/// occur"). Every number here is Claude's.
pub const HISTORY_FIGHTS: [(&str, u64); 2] = [("scavenger", 300), ("wolf", 300)];

/// Of 1000, the chance two people meeting during the history fight: never
/// within one group, else the likelier of their two kinds'.
pub fn history_fights(a: (&str, Option<&str>), b: (&str, Option<&str>)) -> u64 {
    if a.1.is_some() && a.1 == b.1 {
        return 0;
    }
    let of = |kind: &str| HISTORY_FIGHTS.iter().find(|(k, _)| *k == kind).map_or(0, |(_, n)| *n);
    of(a.0).max(of(b.0))
}

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
/// A bed, where its owner wakes after dying; struck with a weapon in hand
/// it breaks (topside-town "Death and the bed").
pub const BED: &str = "bed";
/// What else a person sleeps on (topside-town "Everything works": "beds,
/// couches and sleeping bags rest a person").
pub const COUCH: &str = "couch";
pub const SLEEPING_BAG: &str = "sleeping bag";
/// How well each rests a sleeper (modforge `SurvivalStats::slept_on`), a
/// bed fully; sleeping on nothing rests at `GROUND_COMFORT`. Claude's.
pub const COMFORT: [(&str, f32); 3] = [(BED, 1.0), (COUCH, 0.75), (SLEEPING_BAG, 0.6)];
pub const GROUND_COMFORT: f32 = 0.4;
/// How near a bed or a couch a sleeper is to be sleeping on it, metres.
pub const SLEEPS_ON_REACH: f32 = 1.5;
/// A house's water fixtures and the tower they draw from (topside-town
/// "Everything works": "faucets, toilets, showers, from the water tower").
pub const SINK: &str = "sink";
pub const SHOWER: &str = "shower";
pub const TOILET: &str = "toilet";
pub const WATER_TOWER: &str = "water tower";
/// A house's medicine cabinet and what it is found holding (topside-town
/// "Everything works": "medicine cabinets ... and first aid kits treat
/// wounds"; Project Zomboid's). What it holds and the kit's healing are
/// Claude's.
pub const MEDICINE_CABINET: &str = "medicine cabinet";
pub const FIRST_AID_KIT: &str = "first aid kit";
pub const CABINET_HOLDS: [(&str, u32); 2] = [("bandage", 2), (FIRST_AID_KIT, 1)];
/// A house's clothing (topside-town "Everything works": "closets and
/// dressers hold clothes; the washer and dryer clean them").
pub const CLOSET: &str = "closet";
pub const DRESSER: &str = "dresser";
pub const WASHER: &str = "washer";
pub const DRYER: &str = "dryer";
/// What each is found holding (Claude's).
pub const WARDROBE_HOLDS: [(&str, &str, u32); 3] = [(CLOSET, "shirt", 2), (DRESSER, "trousers", 2), (CLOSET, "boots", 1)];
/// A house's storage (topside-town "Everything works": "pantries,
/// cabinets, garages, attics and basements hold things"), and what each
/// is found holding (Claude's).
pub const KITCHEN_CABINET: &str = "kitchen cabinet";
pub const GARAGE_SHELF: &str = "garage shelf";
/// The garage's workbench (a crafting bench) and toolbox (topside-town
/// "Everything works": "garage workbenches, toolboxes ... fix and build
/// things"); a wrench carried is what fixes a car (`CAR_REPAIR`).
pub const GARAGE_WORKBENCH: &str = "garage workbench";
pub const TOOLBOX: &str = "toolbox";
pub const WRENCH: &str = "wrench";
pub const STORAGE_HOLDS: [(&str, &str, u32); 11] = [
    (GARAGE_SHELF, CARROT_SEED, 3),
    (GARAGE_SHELF, POTATO_SEED, 3),
    (KITCHEN_CABINET, EXTINGUISHER, 1),
    (KITCHEN_CABINET, "water bottle", 1),
    (GARAGE_SHELF, "scrap", 3),
    (GARAGE_SHELF, "pipe", 1),
    (GARAGE_SHELF, "wooden handle", 1),
    (TOOLBOX, WRENCH, 1),
    (TOOLBOX, HAMMER, 1),
    (TOOLBOX, "scrap", 2),
    (GARAGE_SHELF, PROPANE_TANK, 1),
];
/// Soap carried doubles what a wash takes off (modforge hygiene).
pub const SOAP: &str = "soap";
/// How much the tower holds, in water barrels of 200 litres: 50, 10,000
/// litres for the town (Claude's; a real town's tower holds far more).
pub const WATER_TOWER_BARRELS: usize = 50;
pub const WOOD: &str = "wood";
pub const STONE: &str = "stone";

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
/// The town's vehicles (topside-town "Everything works": "cars in
/// driveways, bikes, trucks and garage doors"; Project Zomboid's parked
/// cars): each kind's actor name and its rules, by its number on its
/// `Vehicle` (0 the car). A truck is slower with more seats and a bigger
/// tank; a bike is quicker than walking, one seat, needs no fuel and
/// never breaks. All Claude's.
pub const TRUCK: &str = "truck";
pub const BIKE: &str = "bike";
pub const VEHICLES: [(&str, modforge::vehicle::VehicleDef); 3] = [
    (CAR, CAR_DEF),
    (TRUCK, modforge::vehicle::VehicleDef { speed: 2.5, tank: 900.0, per_can: 180.0, seats: 3, breakdown: 0.005 }),
    (BIKE, modforge::vehicle::VehicleDef { speed: 1.8, tank: 1.0e9, per_can: 0.0, seats: 1, breakdown: 0.0 }),
];
/// What stands in a house's driveway, by the house's number in the town:
/// one in three a car, one in three a truck, the rest a bike. Claude's.
pub fn driveway(house: usize) -> usize {
    house % 3
}
/// The rules of a vehicle by its number.
pub fn vehicle_def(kind: u8) -> &'static modforge::vehicle::VehicleDef {
    &VEHICLES.get(kind as usize).unwrap_or(&VEHICLES[0]).1
}
pub const GENERATOR: &str = "generator";
pub const FUEL: &str = "fuel";
/// Fuel (topside-town "Everything works": "gas pumps, gas cans, propane
/// tanks and firewood fuel generators, stoves and vehicles"; Project
/// Zomboid's): a generator burns a can of fuel or a propane tank; a car
/// takes the fuel; fires burn wood (Works 3). The gas station's pumps
/// are a store of fuel cans, standing on the first main street lot until
/// Place 11 builds the gas station. Claude's.
pub const PROPANE_TANK: &str = "propane tank";
pub const GENERATOR_FUELS: [&str; 2] = [FUEL, PROPANE_TANK];
pub const GAS_PUMP: &str = "gas pump";
/// Security (topside-town "Everything works": "locks, keys, deadbolts,
/// safes and the sheriff's armory"; Project Zomboid's house keys): each
/// house's front door is locked, its key on the garage shelf (or the
/// kitchen cabinet over a bunker); each house has a safe in its first
/// bedroom, its key in the dresser, holding shells and scrap. House n's
/// front door is lock 2n + 1, its safe 2n + 2. All Claude's.
pub const SAFE: &str = "safe";
/// Fire (topside-town "Everything works": "fire spreads; smoke
/// detectors, extinguishers, hydrants and the fire station fight it";
/// Project Zomboid's fire): matches in hand set a burnable thing alight
/// with E; it catches what burns within 2 m after 20 s and is gone after
/// 3 minutes; each house's smoke detector sounds while a fire is within
/// 10 m; an extinguisher in hand, E, puts out every fire within 3 m; a
/// hydrant, E, every fire within 25 m (its hose). One extinguisher in
/// each kitchen cabinet, matches in each pantry, a hydrant each 90 m of
/// street. All Claude's.
/// Communication (topside-town "Everything works": "phones, radios, TVs,
/// mailboxes and the post office"; Project Zomboid's channels): each
/// house has a TV and a radio in its living room and a phone in its
/// hall, each turned on with E and saying what is on at that hour; and a
/// mailbox at the street holding a letter. The programmes and the
/// letters are Claude's.
pub const TV: &str = "tv";
pub const PHONE: &str = "phone";
pub const MAILBOX: &str = "mailbox";
pub const TV_CHANNEL: [modforge::broadcast::Programme; 3] = [
    modforge::broadcast::Programme { from: 6.0, to: 12.0, words: "The morning news, on a loop: the same anchor, the same weather, a date weeks old." },
    modforge::broadcast::Programme { from: 12.0, to: 20.0, words: "A cartoon, then a test card, then the cartoon again." },
    modforge::broadcast::Programme { from: 20.0, to: 6.0, words: "The emergency broadcast: stay indoors, stay below ground, wait for the all clear." },
];
pub const RADIO_CHANNEL: [modforge::broadcast::Programme; 2] = [
    modforge::broadcast::Programme { from: 0.0, to: 18.0, words: "Static. Two clicks. Static." },
    modforge::broadcast::Programme { from: 18.0, to: 24.0, words: "A voice reads a list of names, slowly, then the list starts again." },
];
pub const PHONE_LINE: [modforge::broadcast::Programme; 1] =
    [modforge::broadcast::Programme { from: 0.0, to: 24.0, words: "A dial tone. A number rings and rings, then someone breathes, and hangs up." }];
/// The channel a thing plays when turned on, by its name.
pub fn channel_of(thing: &str) -> Option<&'static [modforge::broadcast::Programme]> {
    match thing {
        TV => Some(&TV_CHANNEL),
        RADIO_ITEM => Some(&RADIO_CHANNEL),
        PHONE => Some(&PHONE_LINE),
        _ => None,
    }
}
pub const LETTERS: [&str; 3] = [
    "Final notice. Your account is past due. Service will be cut off.",
    "Dear neighbour, the block party is cancelled. Keep the kids in. Something is wrong with the sky.",
    "If you find this, we went down to the shelter by the school. Come find us.",
];
/// Trash (topside-town "Everything works": "trash cans, the dump, septic
/// and sewage"; Project Zomboid's bins): each house's trash can at the
/// street; once a day at 6 the truck empties every can into the dump's
/// piles, which stand on the last lot that is not a house until Place 13
/// builds the dump, and start holding junk to scavenge. All Claude's.
/// Growing food (topside-town "Everything works": "gardens, farm fields,
/// barns, animals and seed make food"; Project Zomboid's farming): each
/// house has two garden beds in its back yard; E at an empty bed with a
/// seed carried sows it; ripe after its days, E harvests its food and
/// seed back; seed is found on garage shelves. All Claude's.
pub const GARDEN_BED: &str = "garden bed";
pub const CARROT_SEED: &str = "carrot seed";
pub const POTATO_SEED: &str = "potato seed";
pub const CARROT: &str = "carrot";
pub const POTATO: &str = "potato";
pub const CROPS: [modforge::grow::CropDef; 2] = [
    modforge::grow::CropDef { seed: CARROT_SEED, food: CARROT, days: 2.0, gives: 4, seeds_back: 2 },
    modforge::grow::CropDef { seed: POTATO_SEED, food: POTATO, days: 3.0, gives: 5, seeds_back: 2 },
];
/// The town's places (topside-town "What the town has"), each a real
/// building built by modforge plan.rs from its plan, on a lot of its
/// part, in this order. Each thing a place holds is a box found holding
/// `PLACE_HOLDS`, unless it is a station (`PLACE_STATIONS`). Every plan,
/// room size and stock is Claude's.
pub const SHELF: &str = "store shelf";
pub const CASH_REGISTER: &str = "cash register";
pub const PALLET: &str = "pallet";
pub const WALK_IN_COOLER: &str = "walk-in cooler";
pub const OFFICE_DESK: &str = "office desk";
pub const SEED_BIN: &str = "seed bin";
pub const FEED_TROUGH: &str = "feed trough";
pub const MILK_CAN: &str = "milk can";
pub const NEST_BOX: &str = "nest box";
pub const HAY_BALE: &str = "hay bale";
pub const BUTCHER_BLOCK: &str = "butcher block";
pub const EGG: &str = "egg";
pub const MILK: &str = "milk";
pub const FEED: &str = "animal feed";
pub const GRAIN_HOPPER: &str = "grain hopper";
pub const GRAIN: &str = "grain";
pub const FROZEN_PIZZA: &str = "frozen pizza";
pub const BAKERY_CASE: &str = "bakery case";
pub const BREAD: &str = "bread";
pub const LIQUOR_SHELF: &str = "liquor shelf";
pub const LIQUOR: &str = "bottle of liquor";
pub const WATER_PUMP: &str = "water pump";
pub const WATER_TOWER_TAP: &str = "water tower tap";
pub const SUBSTATION_SWITCH: &str = "substation switch";
pub const TRANSFORMER: &str = "transformer";
/// The things of places that are switches.
pub const PLACE_SWITCHES: [&str; 7] = [SUBSTATION_SWITCH, SEWAGE_PUMP, POND_VALVE, RADIO_ITEM, TV, PHONE, CANDLE];
pub const SEWAGE_PUMP: &str = "sewage pump";
pub const POND_VALVE: &str = "pond valve";
pub const PHARMACY_SHELF: &str = "pharmacy shelf";
pub const GEAR_LOCKER: &str = "gear locker";
pub const SHERIFFS_DESK: &str = "sheriff's desk";
pub const GUN_LOCKER: &str = "gun locker";
/// The armory's lock: past every house's (2 a house, 50 houses).
pub const ARMORY_LOCK: u32 = KEYS;
pub const ARMORY_KEY: &str = "key 128";
/// The boxes of places under lock, each with its lock.
pub const PLACE_LOCKS: [(&str, u32); 1] = [(GUN_LOCKER, ARMORY_LOCK)];
pub const MAIL_SACK: &str = "mail sack";
pub const RECORDS_CABINET: &str = "records cabinet";
pub const GRAVE: &str = "grave";
/// What the graves' stones say, in turn. Every word is Claude's.
pub const EPITAPHS: [&str; 6] = [
    "Walter Pruitt. 1931 - 2004. He kept the elevator running.",
    "Ada May Larkin. Beloved mother. Gone to her rest.",
    "Here lies a stranger, found on the county road. Known to God.",
    "Tom and Ruth Ecker. Together, as they were.",
    "Danny Sheldrake. 1988 - 1997. Our boy.",
    "Reverend Hal Ostrowski. He married half this town and buried the rest.",
];

/// Where the town's places are: content/places.yaml, each place a
/// blueprint (modforge `plan::PlanDef`) keyed by its name, and `town`
/// listing them in the order they take their lots.
pub const PLACES_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/places.yaml");

/// The places standing in the town, in order, read from `PLACES_FILE`. A
/// file that does not read, or a `town` name with no blueprint, stops the
/// game at once, naming it.
pub fn places() -> Vec<modforge::plan::PlanDef> {
    type Places = std::collections::BTreeMap<String, modforge::plan::PlanDef>;
    let text = std::fs::read_to_string(PLACES_FILE).unwrap_or_else(|e| panic!("{PLACES_FILE}: {e}"));
    let mut file: std::collections::BTreeMap<String, serde_yaml::Value> = serde_yaml::from_str(&text).unwrap_or_else(|e| panic!("{PLACES_FILE}: {e}"));
    let mut part = |key: &str| file.remove(key).unwrap_or_else(|| panic!("{PLACES_FILE}: no {key}"));
    let (plans, town) = (part("places"), part("town"));
    if let Some(key) = file.keys().next() {
        panic!("{PLACES_FILE}: {key} is not places or town");
    }
    let plans: Places = serde_yaml::from_value(plans).unwrap_or_else(|e| panic!("{PLACES_FILE}: places: {e}"));
    let town: Vec<String> = serde_yaml::from_value(town).unwrap_or_else(|e| panic!("{PLACES_FILE}: town: {e}"));
    town.iter()
        .map(|name| {
            let plan = plans.get(name).unwrap_or_else(|| panic!("{PLACES_FILE}: town names {name}, which has no blueprint"));
            modforge::plan::PlanDef { name: name.clone(), ..plan.clone() }
        })
        .collect()
}
/// The places whose garden beds are found sown and ripe, with what.
/// The small world's tiers, 1 to 6 (topside-town "Tiers": the
/// operator's "tier 6 enemy house"); the count is the map's.
pub const TIERS: u32 = 6;
/// Each kind's usual tier: a house and the shops are quiet, what the town
/// runs on and guards is higher. Every number is Claude's; a kind not
/// named is tier 1.
pub const USUAL_TIERS: [(&str, u32); 9] = [
    ("house", 1),
    ("gas station", 2),
    ("clinic", 2),
    ("substation", 2),
    ("grain elevator", 2),
    ("hardware store", 2),
    ("mechanic's garage", 2),
    ("sheriff's office", 3),
    ("fire station", 2),
];
pub const PLACE_SOWN:[(&str, &str); 1] = [("crop farm", POTATO_SEED)];
/// What each place's boxes are found holding: the place, the box, the
/// item and how many.
pub const PLACE_HOLDS: [(&str, &str, &str, u32); 104] = [
    ("general store", SHELF, "canned food", 4),
    ("general store", SHELF, "bandage", 2),
    ("general store", SHELF, MATCHES, 2),
    ("general store", SHELF, FLASHLIGHT, 1),
    ("general store", CASH_REGISTER, "scrap", 2),
    ("general store", PALLET, "water bottle", 8),
    ("general store", DRESSER, "cloth", 3),
    ("mechanic's garage", TOOLBOX, WRENCH, 2),
    ("mechanic's garage", CASH_REGISTER, "scrap", 2),
    ("mechanic's garage", SHELF, FUEL, 2),
    ("mechanic's garage", SHELF, "pipe", 4),
    ("mechanic's garage", PALLET, "scrap", 10),
    ("hardware store", SHELF, HAMMER, 2),
    ("hardware store", SHELF, WRENCH, 2),
    ("hardware store", SHELF, "rope", 4),
    ("hardware store", SHELF, MATCHES, 3),
    ("hardware store", CASH_REGISTER, "scrap", 2),
    ("hardware store", PALLET, WOOD, 20),
    ("hardware store", PALLET, "pipe", 6),
    ("cemetery", TOOLBOX, "rope", 1),
    ("cemetery", TOOLBOX, "scrap", 2),
    ("church", OFFICE_DESK, MATCHES, 1),
    ("church", CLOSET, BLANKET, 2),
    ("church", PANTRY, "canned food", 6),
    ("church", PANTRY, "water bottle", 4),
    ("church", SHELF, CANDLE, 4),
    ("town hall", CASH_REGISTER, "scrap", 1),
    ("town hall", OFFICE_DESK, "cloth", 1),
    ("town hall", RECORDS_CABINET, "cloth", 2),
    ("school", OFFICE_DESK, "cloth", 1),
    ("school", MEDICINE_CABINET, "bandage", 4),
    ("school", PANTRY, "canned food", 8),
    ("school", SHELF, "rope", 2),
    ("school", TOOLBOX, SOAP, 3),
    ("school", TOOLBOX, WRENCH, 1),
    ("sheriff's office", OFFICE_DESK, "cloth", 1),
    ("sheriff's office", SHERIFFS_DESK, ARMORY_KEY, 1),
    ("sheriff's office", GUN_LOCKER, "shotgun shell", 16),
    ("sheriff's office", GUN_LOCKER, FLASHLIGHT, 1),
    ("sheriff's office", SHELF, "scrap", 3),
    ("fire station", GEAR_LOCKER, "road sign vest", 2),
    ("fire station", GEAR_LOCKER, FLASHLIGHT, 2),
    ("fire station", PANTRY, "canned food", 4),
    ("clinic", OFFICE_DESK, "cloth", 1),
    ("clinic", PHARMACY_SHELF, "bandage", 5),
    ("clinic", PHARMACY_SHELF, FIRST_AID_KIT, 3),
    ("clinic", CASH_REGISTER, "scrap", 1),
    ("clinic", MEDICINE_CABINET, "bandage", 3),
    ("clinic", MEDICINE_CABINET, SOAP, 2),
    ("town dump", OFFICE_DESK, "cloth", 1),
    ("town dump", TOOLBOX, "scrap", 4),
    ("sewage pond", TOOLBOX, WRENCH, 1),
    ("sewage pond", TOOLBOX, "pipe", 2),
    ("gas station", SHELF, "canned food", 3),
    ("gas station", SHELF, "water bottle", 3),
    ("gas station", CASH_REGISTER, "scrap", 2),
    ("gas station", TOOLBOX, WRENCH, 1),
    ("gas station", PALLET, PROPANE_TANK, 2),
    ("substation", OFFICE_DESK, "cloth", 1),
    ("substation", TOOLBOX, WRENCH, 1),
    ("substation", TRANSFORMER, "scrap", 6),
    ("pump house", OFFICE_DESK, "cloth", 1),
    ("pump house", TOOLBOX, WRENCH, 1),
    ("pump house", PALLET, "pipe", 4),
    ("bar", CASH_REGISTER, "scrap", 2),
    ("bar", LIQUOR_SHELF, LIQUOR, 6),
    ("bar", OFFICE_DESK, "cloth", 1),
    ("bar", PALLET, "water bottle", 6),
    ("cafe", CASH_REGISTER, "scrap", 1),
    ("cafe", BAKERY_CASE, BREAD, 6),
    ("cafe", PANTRY, GRAIN, 8),
    ("pizza place", CASH_REGISTER, "scrap", 1),
    ("pizza place", PANTRY, GRAIN, 4),
    ("pizza place", PANTRY, "canned food", 3),
    ("pizza place", FREEZER, FROZEN_PIZZA, 6),
    ("diner", CASH_REGISTER, "scrap", 2),
    ("diner", PANTRY, "canned food", 4),
    ("diner", PANTRY, EGG, 6),
    ("diner", WALK_IN_COOLER, RAW_MEAT, 6),
    ("diner", OFFICE_DESK, "cloth", 1),
    ("grain elevator", SHELF, FEED, 8),
    ("grain elevator", CASH_REGISTER, "scrap", 2),
    ("grain elevator", PALLET, FEED, 16),
    ("grain elevator", GRAIN_HOPPER, GRAIN, 20),
    ("grain elevator", SHELF, CARROT_SEED, 6),
    ("livestock barn", FEED_TROUGH, FEED, 6),
    ("livestock barn", TOOLBOX, "cloth", 2),
    ("livestock barn", MILK_CAN, MILK, 4),
    ("livestock barn", NEST_BOX, EGG, 6),
    ("livestock barn", HAY_BALE, FEED, 10),
    ("livestock barn", BUTCHER_BLOCK, RAW_MEAT, 4),
    ("crop farm", PANTRY, "canned food", 2),
    ("crop farm", OFFICE_DESK, "cloth", 1),
    ("crop farm", TOOLBOX, WRENCH, 1),
    ("crop farm", SEED_BIN, CARROT_SEED, 10),
    ("crop farm", SEED_BIN, POTATO_SEED, 10),
    ("grocery store", SHELF, "canned food", 6),
    ("grocery store", SHELF, "water bottle", 4),
    ("grocery store", SHELF, SOAP, 2),
    ("grocery store", CASH_REGISTER, "scrap", 2),
    ("grocery store", OFFICE_DESK, "cloth", 1),
    ("grocery store", PALLET, "canned food", 12),
    ("grocery store", WALK_IN_COOLER, RAW_MEAT, 6),
    ("grocery store", PALLET, CARROT_SEED, 4),
];
/// The things of places that are crafting stations, not boxes.
pub const PLACE_STATIONS: [(&str, modforge::crafting::StationKind); 2] = [(STOVE, modforge::crafting::StationKind::Stove), (GARAGE_WORKBENCH, modforge::crafting::StationKind::CraftingBench)];
pub const TRASH_CAN: &str = "trash can";
pub const DUMP_PILE: &str = "dump pile";
pub const COLLECTION_HOUR: f32 = 6.0;
pub const DUMP_HOLDS: [(&str, u32); 3] = [("scrap", 6), ("cloth", 3), ("pipe", 1)];
pub const MATCHES: &str = "matches";
pub const EXTINGUISHER: &str = "extinguisher";
pub const SMOKE_DETECTOR: &str = "smoke detector";
pub const HYDRANT: &str = "hydrant";
pub const FIRE: modforge::fire::FireDef = modforge::fire::FireDef { spread_after: 20.0, spread_reach: 2.0, burns_for: 180.0, smoke_reach: 10.0 };
pub const EXTINGUISHER_REACH: f32 = 3.0;
pub const HOSE_REACH: f32 = 25.0;
pub const HYDRANTS_APART: f32 = 90.0;
/// What burns: the town's furniture and boxes.
pub const BURNS: [&str; 11] = [COUCH, BED, CLOSET, DRESSER, KITCHEN_CABINET, GARAGE_SHELF, PANTRY, GARAGE_WORKBENCH, TOOLBOX, "storage box", SLEEPING_BAG];
pub const KEYS: u32 = 128;
pub const SAFE_HOLDS: [(&str, u32); 2] = [("shotgun shell", 6), ("scrap", 4)];
pub fn front_lock(house: usize) -> u32 {
    2 * house as u32 + 1
}
pub fn safe_lock(house: usize) -> u32 {
    2 * house as u32 + 2
}
pub const GENERATOR_DEF: modforge::power::GeneratorDef = modforge::power::GeneratorDef { output: 10.0, burn_secs: 600.0 };
pub const WIRE_REACH: f32 = 12.0;
/// The town's power (topside-town "Everything works": "the substation,
/// the lines, breaker boxes, outlets, switches, generators"; Project
/// Zomboid's house power): the substation, each house's breaker box, each
/// room's light switch and its light.
pub const SUBSTATION: &str = "substation";
pub const BREAKER_BOX: &str = "breaker box";
pub const LIGHT_SWITCH: &str = "light switch";
pub const CEILING_LIGHT: &str = "ceiling light";
/// How far the substation's lines reach: the whole town (Claude's).
pub const LINES_REACH: f32 = 2000.0;
/// A house's heating and cooling (topside-town "Everything works":
/// "furnaces, fireplaces, wood stoves, AC, blankets, thermostats"):
/// fires burn wood; the furnace and the air conditioner run on the
/// house's power while its thermostat is on; a blanket carried keeps one
/// warm.
pub const FIREPLACE: &str = "fireplace";
pub const WOOD_STOVE: &str = "wood stove";
pub const FURNACE: &str = "furnace";
pub const AIR_CONDITIONER: &str = "air conditioner";
pub const THERMOSTAT: &str = "thermostat";
pub const BLANKET: &str = "blanket";
/// The things that burn wood.
pub const FIRES: [&str; 2] = [FIREPLACE, WOOD_STOVE];
/// How long one wood burns in a fire: 5 game minutes (Claude's).
pub const FIRE_DEF: modforge::power::GeneratorDef = modforge::power::GeneratorDef { output: 0.0, burn_secs: 300.0 };
/// Each heat source's heat (degrees C added to the air) and its reach in
/// metres (modforge `exposure::warmth_near`); the AC cools. Claude's.
pub const HEAT: [(&str, f32, f32); 4] = [(FIREPLACE, 20.0, 4.0), (WOOD_STOVE, 25.0, 4.0), (FURNACE, 18.0, 12.0), (AIR_CONDITIONER, -12.0, 12.0)];
/// What a blanket carried adds to the air one feels (Claude's).
pub const BLANKET_WARMTH: f32 = 8.0;
/// The other lights (topside-town "Everything works": "lamps,
/// flashlights, candles, streetlights").
pub const LAMP: &str = "lamp";
pub const CANDLE: &str = "candle";
pub const STREETLIGHT: &str = "streetlight";
pub const FLASHLIGHT: &str = "flashlight";
/// How brightly each light shines, lumens: the ceiling light the house
/// plan's (modforge house.rs), the rest Claude's.
pub const LIGHT_LUMENS: [(&str, f32); 5] =
    [(CEILING_LIGHT, 300_000.0), (LAMP, 100_000.0), (CANDLE, 5_000.0), (STREETLIGHT, 1_000_000.0), (FLASHLIGHT, 200_000.0)];
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

/// Where the storm is: content/storm.yaml, its `storm` a modforge
/// `storm::StormDef`.
pub const STORM_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/storm.yaml");

/// The storm, read from `STORM_FILE` the first time it is asked for.
pub static STORM: std::sync::LazyLock<StormDef> = std::sync::LazyLock::new(|| read(STORM_FILE, "storm"));

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
                model: None,
            },
            // The crafting bench, in the surface room.
            SolidDef {
                center: Vec3::new(2.2, SURFACE_ROOM_FLOOR + 0.45, 3.5),
                size: Vec3::new(1.6, 0.9, 0.7),
                color: [0.45, 0.32, 0.18],
                model: None,
            },
            // The research station, in the surface room (todo 28).
            SolidDef {
                center: Vec3::new(-2.2, SURFACE_ROOM_FLOOR + 0.45, 3.5),
                size: Vec3::new(1.6, 0.9, 0.7),
                color: [0.30, 0.40, 0.42],
                model: None,
            },
            // The food crate, in the player's room.
            SolidDef {
                center: Vec3::new(-2.5, PLAYER_ROOM_FLOOR + 0.35, -3.3),
                size: Vec3::new(0.7, 0.7, 0.7),
                color: [0.55, 0.45, 0.20],
                model: None,
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
    for def in read_list::<LayerDef>(LAYERS_FILE, "layers") {
        let picture = Some(def.picture.clone().unwrap_or_else(|| def.name.clone()));
        reg.register(LayerDef { picture, ..def }).expect("layer registered once");
    }
}

/// Where the layers are: content/layers.yaml, its `layers` a list of
/// modforge `item::LayerDef`, each naming only what it has.
pub const LAYERS_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/layers.yaml");

/// The one list `key` in the content file `path`, each entry read as a
/// `T` naming only what it has.
pub fn read_list<T: serde::de::DeserializeOwned>(path: &str, key: &str) -> Vec<T> {
    read(path, key)
}

/// The one thing `key` in the content file `path`, read as a `T`. A file
/// that does not read, has no `key`, or has anything else in it stops the
/// game at once, naming it.
pub fn read<T: serde::de::DeserializeOwned>(path: &str, key: &str) -> T {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut file: std::collections::BTreeMap<String, serde_yaml::Value> = serde_yaml::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
    let value = file.remove(key).unwrap_or_else(|| panic!("{path}: no {key}"));
    if let Some(other) = file.keys().next() {
        panic!("{path}: {other} is not {key}");
    }
    serde_yaml::from_value(value).unwrap_or_else(|e| panic!("{path}: {key}: {e}"))
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

/// Where plain talk is: content/lines.yaml, its `lines` a list of modforge
/// storyteller `LineDef`, written as an episode's lines are.
pub const LINES_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/lines.yaml");

/// Plain talk: what anyone says outside an episode's lines, the same rules
/// (episodes.md "One way for every conversation"), from `LINES_FILE`.
/// Through a JSON value, as an episode is (`episode_from_yaml`): a line's
/// conditions hold one inside another. A file that does not read stops
/// the game at once, naming it.
pub fn plain_lines() -> Vec<LineDef> {
    let text = std::fs::read_to_string(LINES_FILE).unwrap_or_else(|e| panic!("{LINES_FILE}: {e}"));
    let mut file: std::collections::BTreeMap<String, serde_json::Value> = serde_yaml::from_str(&text).unwrap_or_else(|e| panic!("{LINES_FILE}: {e}"));
    let lines = file.remove("lines").unwrap_or_else(|| panic!("{LINES_FILE}: no lines"));
    serde_json::from_value(lines).unwrap_or_else(|e| panic!("{LINES_FILE}: lines: {e}"))
}

/// The line a person says agreeing to stay with the one who asked: said,
/// they join that one's group (todo 11af, `joining::join`). Its name in
/// `LINES_FILE`.
pub const AGREES_TO_STAY: &str = "agrees to stay";

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

/// Where the realities are: content/realities.yaml, its `realities` a
/// list of modforge `reality::RealityDef`.
pub const REALITIES_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/realities.yaml");

/// The realities built so far, from `REALITIES_FILE`, Mixed world first:
/// the world before any storm.
pub fn register_realities(reg: &mut RealityRegistry) {
    for def in read_list::<RealityDef>(REALITIES_FILE, "realities") {
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

    /// Where the other bunkers stand in a world with no town; the small
    /// world's are under its houses (`Town::bunkers`).
    pub fn other_bunkers(self) -> Vec<Vec2> {
        match self {
            Self::Small => Vec::new(),
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
pub const KEEPS_DAYS: [(&str, f32); 3] = [("canned food", 10.0), (RAW_MEAT, 1.0), (COOKED_MEAT, 2.0)];
/// A house's kitchen (topside-town "Everything works": food "found, kept
/// in pantries, fridges and freezers, cooked on stoves, eaten"; Project
/// Zomboid's kitchens).
pub const PANTRY: &str = "pantry";
pub const FRIDGE: &str = "fridge";
pub const FREEZER: &str = "freezer";
pub const STOVE: &str = "stove";
pub const RAW_MEAT: &str = "raw meat";
pub const COOKED_MEAT: &str = "cooked meat";
/// How much a powered fridge and freezer slow food's ageing (modforge
/// `item::keep_cold`). Claude's.
pub const COLD: [(&str, f32); 2] = [(FRIDGE, 0.75), (FREEZER, 1.0)];
/// What a house's kitchen is found holding: (where, what, how many).
/// Claude's.
pub const KITCHEN_FOOD: [(&str, &str, u32); 4] = [(PANTRY, "canned food", 3), (FRIDGE, RAW_MEAT, 2), (PANTRY, SOAP, 1), (PANTRY, MATCHES, 1)];
/// What spoiled food becomes.
pub const ROTTEN_FOOD: &str = "rotten food";

/// Where the gear is: content/gear.yaml (modforge `gear`: bases, affixes
/// rolled X to Y, runes and rune words; design.md "Loot randomization").
pub const GEAR_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/gear.yaml");

/// `GEAR_FILE`, read the first time it is asked for. A file that does not
/// read stops the game at once, naming it.
pub static GEAR: std::sync::LazyLock<modforge::gear::GearDefs> = std::sync::LazyLock::new(|| {
    let text = std::fs::read_to_string(GEAR_FILE).unwrap_or_else(|e| panic!("{GEAR_FILE}: {e}"));
    serde_yaml::from_str(&text).unwrap_or_else(|e| panic!("{GEAR_FILE}: {e}"))
});

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

/// The small world's town (topside-town, operator 2026-11-23: "let's just
/// start with 50 houses"), laid around the player's bunker so it stands
/// under a house. The 50 houses are the operator's; every size here is
/// Claude's, for the operator to change.
pub fn small_town_def() -> TownDef {
    TownDef {
        houses: 50,
        lot: Vec2::new(20.0, 30.0),
        street: 8.0,
        main_lots: 8,
        main_lot: Vec2::new(25.0, 40.0),
        cul_de_sacs: (2, 4),
        cul_de_sac_lots: 3,
        // 3 a side at each end, for the places at the edge (Place 2,
        // Claude's; it was 2).
        edge_lots: 3,
        edge_lot: Vec2::new(40.0, 60.0),
        house: house_def(),
        // Up to 4 a house (operator, 2026-11-23: "4 people per house"),
        // in the houses with no bunker; the townspeople are wanderers,
        // topside people who belong to no side, until the town has its
        // own kind. The 1 to 4 and the kind are Claude's.
        residents: ("wanderer".to_string(), (1, 4)),
        places: places(),
        tiers: TIERS,
        usual_tiers: USUAL_TIERS.iter().map(|&(n, t)| (n.to_string(), t)).collect(),
        pieces: PIECES.pieces.clone(),
        briefs: PIECES.briefs.clone(),
    }
}

/// Where the room pieces and the places' briefs are: content/pieces.yaml
/// (modforge `layout`).
pub const PIECES_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/pieces.yaml");

/// What `PIECES_FILE` holds.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PiecesFile {
    pieces: Vec<modforge::layout::RoomPiece>,
    briefs: Vec<modforge::layout::Brief>,
}

/// `PIECES_FILE`, read the first time it is asked for. A file that does
/// not read stops the game at once, naming it.
static PIECES: std::sync::LazyLock<PiecesFile> = std::sync::LazyLock::new(|| {
    let text = std::fs::read_to_string(PIECES_FILE).unwrap_or_else(|e| panic!("{PIECES_FILE}: {e}"));
    serde_yaml::from_str(&text).unwrap_or_else(|e| panic!("{PIECES_FILE}: {e}"))
});

/// What the town's houses vary by (topside-town "The houses": "each
/// varied by colour, rotation and small details"). The colours and
/// chances are Claude's, for the operator to change.
pub fn house_def() -> HouseDef {
    HouseDef {
        walls: vec![
            [0.85, 0.82, 0.74],
            [0.70, 0.76, 0.80],
            [0.80, 0.72, 0.60],
            [0.72, 0.78, 0.66],
            [0.88, 0.86, 0.84],
            [0.62, 0.55, 0.50],
        ],
        floors: vec![[0.45, 0.33, 0.22], [0.55, 0.50, 0.45], [0.38, 0.30, 0.25]],
        furniture: vec![
            [0.45, 0.35, 0.25],
            [0.30, 0.32, 0.40],
            [0.60, 0.58, 0.55],
            [0.40, 0.25, 0.20],
            [0.85, 0.85, 0.82],
        ],
        extras: 600,
        back_windows: 700,
        height: 2.6,
    }
}

/// How many of the town's houses stand over a bunker, the player's among
/// them, unless a new game is given another count (operator, 2026-11-23:
/// "let's start with 10% of the houses will have bunkers, so 5 houses").
pub const DEFAULT_BUNKERS: u32 = 5;

/// How far a small site keeps from the town's square, metres; a site
/// nearer is left out until Tier 3 stands the old monuments in the town.
/// Claude's, for the operator to change.
pub const TOWN_CLEAR: f32 = 40.0;

/// The gap through the trees from the player's bunker toward the launch
/// site's gantry (SMALL_SITES), so the door opens on the big thing on the
/// horizon; past its end the trees hide it as the player walks (design.md
/// "The first hour"). Its length and width are Claude's, for the operator
/// to change.
pub const DOOR_GAP: (Vec2, f32, f32) = (Vec2::new(-300.0, 300.0), 100.0, 10.0);

/// The world a seed gives (design.md "2D world generation"): the ground,
/// the sites (rolled in the big world, placed by hand in the small one),
/// and every bunker on its cleared ground, the player's first. The small
/// world's town is laid from `town`, its seed and how many of its houses
/// stand over a bunker: the town's seed is the game's own, so a storm,
/// rolling the ground again, leaves the town and its bunkers where they
/// are (topside-town "The bunkers"). The one way a world is made, for the
/// game and for the tests.
pub fn roll_world2d(
    kind: WorldKind,
    seed: u64,
    town: (u64, u32),
    biomes: &BiomeRegistry,
    monuments: &MonumentRegistry,
) -> Result<World2d, String> {
    let def = match kind {
        WorldKind::Small => small_ground_def(),
        WorldKind::Big => ground_def(),
    };
    let ground = GroundGen::new(def, seed, biomes)?;
    let town = (kind == WorldKind::Small).then(|| Town::around(&small_town_def(), BUNKER_AT, town.0, town.1));
    let at: Vec<Vec2> = match &town {
        Some(t) => t.bunkers(),
        None => std::iter::once(BUNKER_AT).chain(kind.other_bunkers()).collect(),
    };
    let clearings: Vec<Clearing> = at
        .into_iter()
        .map(|at| Clearing {
            at,
            radius: BUNKER_CLEAR,
            gap: (at == BUNKER_AT && kind == WorldKind::Small).then_some(DOOR_GAP),
        })
        .collect();
    let keep_clear = clearings.iter().map(|c| (c.at, c.radius)).collect();
    let off_town = |at: Vec2| {
        town.as_ref().is_none_or(|t| {
            let (lo, hi) = t.bounds();
            at.clamp(lo, hi).distance(at) > TOWN_CLEAR
        })
    };
    let places = match kind {
        WorldKind::Small => Places::new(Vec::new(), keep_clear).with_fixed(
            SMALL_SITES
                .iter()
                .map(|(m, [x, y])| (m.to_string(), Vec2::new(*x, *y)))
                .filter(|(_, at)| off_town(*at))
                .collect(),
        ),
        WorldKind::Big => Places::new(site_kinds(monuments), keep_clear),
    }
    // Every site rolls its tier from its type's danger (topside-town
    // "Tiers": "an airfield is always tier 3, but it doesn't have to be
    // only tier 3").
    .with_tiers(TIERS);
    let people = match kind {
        WorldKind::Small => SMALL_TOPSIDE_PEOPLE.iter().map(|(d, n)| (d.to_string(), *n)).collect(),
        WorldKind::Big => Vec::new(),
    };
    let world = World2d::new(ground, places, clearings).with_people(people);
    Ok(match town {
        Some(t) => world.with_town(t),
        None => world,
    })
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

/// The topside's own factions (operator, 2026-11-23: "The raiders are one
/// group, and there might be multiple raiders, and scavenger another group,
/// and wildlife is their own separate group" and "every single character
/// in this world belongs to some sort of faction"): the raiders, hostile to
/// everyone (operator: "by definition hostile"); the scavengers and the
/// wildlife, neutral until something makes them hostile (operator:
/// "Scavengers might be hostile if they get into a fight, and wolves...
/// might be hostile if there's something"); and the townspeople. Not
/// bunkers; the faction table is how hostility is read.
pub const RAIDERS: &str = "raiders";
pub const SCAVENGERS: &str = "scavengers";
pub const WILDLIFE: &str = "wildlife";
pub const TOWNSPEOPLE: &str = "townspeople";
/// The topside's people found at sites: the ones who roll a hostile's
/// traits (armored, pack leader).
pub const TOPSIDE_HOSTILES: [&str; 3] = [RAIDERS, SCAVENGERS, WILDLIFE];

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
/// them changes that, todo 11t). The raiders are hostile to everyone; the
/// scavengers, the wildlife and the townspeople start neutral.
pub fn register_factions(reg: &mut FactionRegistry) {
    // Enough for the other bunkers of either world: the small world's
    // can be any of its houses.
    let most = OTHER_BUNKERS.len().max(small_town_def().houses as usize);
    let bunkers = (0..most).map(|i| FactionDef { name: bunker_faction(i), kind: FactionKind::Builder });
    for def in read_list::<FactionDef>(FACTIONS_FILE, "factions").into_iter().chain(bunkers) {
        reg.register(def).expect("faction registered once");
    }
}

/// Where the factions are: content/factions.yaml, its `factions` a list
/// of modforge `faction::FactionDef`.
pub const FACTIONS_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/factions.yaml");

/// Where the kinds of actor are: content/actors.yaml, its `actors` a list
/// of modforge `actor::ActorDef`, each naming only what it has.
pub const ACTORS_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/actors.yaml");

/// The people (the bunker people, their bunker's faction given at spawn;
/// the topside hostiles found at monuments; the wanderer), the animals,
/// the turret and the cars, from `ACTORS_FILE`. A file that does not read
/// stops the game at once, naming it.
pub fn register_actors(reg: &mut ActorRegistry) {
    for def in read_list::<ActorDef>(ACTORS_FILE, "actors") {
        reg.register(def).expect("actor registered once");
    }
}

/// Where the kinds of hurt are: content/damage.yaml, its `damage` a list
/// of modforge `combat::DamageDef`, each naming only what it has.
pub const DAMAGE_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/damage.yaml");

/// Every kind of hurt in `DAMAGE_FILE`. A file that does not read stops
/// the game at once, naming it.
pub fn register_damage(reg: &mut DamageRegistry) {
    for def in read_list::<DamageDef>(DAMAGE_FILE, "damage") {
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

/// Where the items are: content/items.yaml, its `items` a list of
/// modforge `item::ItemDef`, each naming only what it has.
pub const ITEMS_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/items.yaml");

/// Every item in `ITEMS_FILE`, in its order. A file that does not read
/// stops the game at once, naming it.
pub fn items() -> Vec<ItemDef> {
    read_list(ITEMS_FILE, "items")
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
        // Drawn as its own model, named as it is (scripts/models.py).
        picture: Some(name.to_string()),
        layer_slots: Vec::new(),
        breaks_when_struck: false,
        holds_litres: None,
    };
    // Every item in content/items.yaml.
    let items = items();
    // What everyone wears (WORN): plain clothes, wearable and guarding
    // nothing.
    // Each also dirty and washed wet (modforge hygiene: the washer and
    // dryer clean them).
    let clothes = WORN.iter().flat_map(|name| [name.to_string(), format!("dirty {name}"), format!("wet {name}")]).map(|name| ItemDef {
        description: format!("Plain {name}. Worn."),
        name,
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
    // The keys, one to each lock (Works 14), drawn as one key.
    for lock in 1..=KEYS {
        let name = modforge::hud::key_for(lock);
        let def = ItemDef {
            picture: Some("key".to_string()),
            ..building(&name, "A key. It opens one lock.", ItemKind::Material, 1, None)
        };
        reg.register(def).expect("a key is registered once");
    }
    // The base resources, Atlas's, and their kinds (topside Decisions
    // 2026-11-23): a recipe asking for a category takes any kind in it,
    // one asking for a kind takes only that (Minecraft's item tags).
    for (category, kinds) in RESOURCES {
        for kind in kinds.iter() {
            if reg.def(kind).is_some() {
                continue;
            }
            let def = ItemDef {
                picture: Some(kind.to_string()),
                ..building(kind, &format!("{kind}, for crafting and building."), ItemKind::Material, 100, None)
            };
            reg.register(def).expect("a resource is registered once");
        }
        reg.categorize(category, kinds);
    }
    // What fiber makes.
    if reg.def("rope").is_none() {
        let rope = ItemDef {
            picture: Some("rope".to_string()),
            ..building("rope", "Rope twisted from fiber.", ItemKind::Material, 20, None)
        };
        reg.register(rope).expect("rope is registered once");
    }
}

/// Atlas's base resources, each a category and the kinds in it (topside
/// Decisions 2026-11-23: "trees and rocks and fiber and metal and oil and
/// all the various resource types... multiple types of fiber"). The kinds
/// are Claude's, after Atlas's (its woods, its stones, its fibers, its
/// metals, its oils), for the operator to change.
pub const RESOURCES: [(&str, &[&str]); 7] = [
    ("wood", &[WOOD, "softwood", "hardwood"]),
    ("stone", &[STONE, "flint", "granite"]),
    ("fiber", &["plant fiber", "hemp fiber", "cotton"]),
    ("thatch", &["thatch", "reeds"]),
    ("metal", &["scrap", "iron ore", "copper ore"]),
    ("oil", &["crude oil", "animal fat"]),
    ("hide", &["hide", "pelt"]),
];

/// Where the building types are: content/buildings.yaml, its `buildings`
/// a list of modforge `monument::BuildingTypeDef` and its `palettes` the
/// colours they are drawn from, by name.
pub const BUILDINGS_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/buildings.yaml");

/// What `BUILDINGS_FILE` holds.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BuildingsFile {
    palettes: std::collections::BTreeMap<String, Vec<Rgb>>,
    buildings: Vec<BuildingTypeDef>,
}

/// `BUILDINGS_FILE`, read the first time it is asked for. A file that
/// does not read stops the game at once, naming it.
static BUILDINGS: std::sync::LazyLock<BuildingsFile> = std::sync::LazyLock::new(|| {
    let text = std::fs::read_to_string(BUILDINGS_FILE).unwrap_or_else(|e| panic!("{BUILDINGS_FILE}: {e}"));
    serde_yaml::from_str(&text).unwrap_or_else(|e| panic!("{BUILDINGS_FILE}: {e}"))
});

/// The model a wall tile is drawn with, by what it is made of (design.md
/// "Walls and floors by what they are made of"), read from its colour:
/// the bunker's concrete and the concrete palette are concrete, the wood
/// palette wood, the stone palette stone; the rest are the plain wall.
pub fn wall_model(color: Rgb) -> &'static str {
    let made_of = |palette: &str| BUILDINGS.palettes.get(palette).is_some_and(|p| p.contains(&color));
    if color == CONCRETE_WALL || made_of("concrete") {
        "wall concrete"
    } else if made_of("wood") {
        "wall wood"
    } else if made_of("stone") {
        "wall stone"
    } else {
        "wall"
    }
}

/// The building types, from `BUILDINGS_FILE`.
pub fn register_buildings(reg: &mut BuildingRegistry) {
    for building in BUILDINGS.buildings.iter().cloned() {
        reg.register(building).expect("building type defs must not collide");
    }
}

/// Where the monument types are: content/monuments.yaml, its
/// `monuments` a list of modforge `monument::MonumentTypeDef`, its `name
/// words` what their rolled names are made from, and its `lists` the
/// building lists named in them.
pub const MONUMENTS_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/monuments.yaml");

/// What `MONUMENTS_FILE` holds.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct MonumentsFile {
    #[serde(rename = "name words")]
    name_words: NameWords,
    /// Written once and named in each monument; read there.
    #[allow(dead_code)]
    lists: serde_yaml::Value,
    monuments: Vec<MonumentTypeDef>,
}

/// A rolled monument name's words: one of each, then its suffix.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NameWords {
    first: Vec<String>,
    second: Vec<String>,
}

/// The monument types (design.md "Monument types", Rust's adapted):
/// which building types each picks from, how many, how they sit,
/// how dangerous, whether a gated room waits at the back, and the
/// word that ends its rolled name.
pub fn register_monuments(reg: &mut MonumentRegistry, buildings: &BuildingRegistry) {
    let text = std::fs::read_to_string(MONUMENTS_FILE).unwrap_or_else(|e| panic!("{MONUMENTS_FILE}: {e}"));
    let file: MonumentsFile = serde_yaml::from_str(&text).unwrap_or_else(|e| panic!("{MONUMENTS_FILE}: {e}"));
    reg.set_name_words(file.name_words.first, file.name_words.second);
    for monument in file.monuments {
        reg.register(monument, buildings)
            .expect("monument type defs must not collide or name unknown buildings");
    }
}

/// Where the biomes are: content/biomes.yaml, its `biomes` a list of
/// modforge `biome::BiomeDef`, and its `sources` the scatter every biome
/// shares, named in each biome's scatter.
pub const BIOMES_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/biomes.yaml");

/// What `BIOMES_FILE` holds.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BiomesFile {
    /// Written once and named in each biome; read there.
    #[allow(dead_code)]
    sources: serde_yaml::Value,
    biomes: Vec<BiomeDef>,
}

/// Every biome in `BIOMES_FILE`. A file that does not read stops the
/// game at once, naming it.
pub fn register_biomes(reg: &mut BiomeRegistry) {
    let text = std::fs::read_to_string(BIOMES_FILE).unwrap_or_else(|e| panic!("{BIOMES_FILE}: {e}"));
    let file: BiomesFile = serde_yaml::from_str(&text).unwrap_or_else(|e| panic!("{BIOMES_FILE}: {e}"));
    for biome in file.biomes {
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
