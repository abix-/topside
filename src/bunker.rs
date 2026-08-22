//! The opening content (design.md "The bunker"): the starting
//! bunker and the first multi-room building outside it, both
//! expressed as StructureDefs through the structure vocabulary.

use bevy::prelude::*;

use modforge::item::{Inventory, create, write};

use crate::actor::{SpawnActor, WorldSeed, spawn_actor};
use crate::cel::CelMaterial;
use crate::content::{
    ActorRegistryRes, BiomeRegistryRes, ItemRegistryRes, MonumentRegistryRes, starting_note,
    world_def,
};
use crate::interact::{Container, spawn_item};
use crate::structure::{
    CONCRETE_FLOOR, CONCRETE_WALL, LightSpec, Opening, RoomSpec, Side, SolidSpec, StairSpec,
    StructureDef, spawn_structure,
};
use crate::WorldName;
use crate::terrain::{WorldRes, spawn_world};
use modforge::worldgen::roll_world;

/// One game day in real seconds (Valheim-scale pacing; tune later).
pub const GAME_DAY_SECS: f32 = 1200.0;

/// One scavenger waiting outside the bunker door (the north wall is
/// at z -4; it stands 6 m past it, facing the door), through the one
/// spawn path.
pub fn setup_enemy(mut spawn: SpawnActor, actors: Res<ActorRegistryRes>) {
    let def = actors.0.def("scavenger").expect("scavenger is registered");
    spawn_actor(&mut spawn, def, Vec3::new(0.0, 0.0, -10.0), std::f32::consts::PI, None);
}

/// The starting bunker: one room with exactly a bed, a crafting
/// bench, and the food crate (design.md: three things), door north
/// toward topside.
pub fn starting_bunker() -> StructureDef {
    StructureDef {
        name: "the starting bunker".to_string(),
        wall_color: CONCRETE_WALL,
        floor_color: CONCRETE_FLOOR,
        rooms: vec![RoomSpec {
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
            SolidSpec {
                center: Vec3::new(-2.4, 0.25, 3.0),
                size: Vec3::new(1.0, 0.5, 2.0),
                color: [0.30, 0.35, 0.55],
            },
            SolidSpec {
                center: Vec3::new(2.2, 0.45, 3.5),
                size: Vec3::new(1.6, 0.9, 0.7),
                color: [0.45, 0.32, 0.18],
            },
            SolidSpec {
                center: Vec3::new(2.5, 0.35, -3.3),
                size: Vec3::new(0.7, 0.7, 0.7),
                color: [0.55, 0.45, 0.20],
            },
        ],
        lights: vec![LightSpec {
            position: Vec3::new(0.0, 2.7, 0.0),
            color: [1.0, 0.9, 0.7],
            intensity: 400_000.0,
        }],
    }
}

/// The first multi-room building: a ground room, a full-height
/// stairwell east of it, and an upper room stacked above, connected
/// by matching openings. Proves rooms, hallway-style connection,
/// stairs, and a second floor through the same one spawn path.
pub fn two_story_house() -> StructureDef {
    StructureDef {
        name: "the two-story house".to_string(),
        wall_color: CONCRETE_WALL,
        floor_color: CONCRETE_FLOOR,
        rooms: vec![
            // Ground room. Ceiling skipped: the upper room's floor
            // slab covers it.
            RoomSpec {
                origin: Vec3::ZERO,
                interior: Vec3::new(6.0, 3.0, 8.0),
                wall_thickness: 0.2,
                openings: vec![
                    Opening {
                        side: Side::South,
                        offset: 0.0,
                        width: 1.2,
                        sill: 0.0,
                        door: true,
                    },
                    Opening {
                        side: Side::East,
                        offset: 0.9,
                        width: 1.2,
                        sill: 0.0,
                        door: true,
                    },
                ],
                floor: true,
                ceiling: false,
            },
            // Stairwell: full height, east of the ground room, wide
            // enough for flat approach room in front of the bottom
            // step (stairs must never end at a wall).
            RoomSpec {
                origin: Vec3::new(5.65, 0.0, 0.0),
                interior: Vec3::new(4.5, 6.0, 3.0),
                wall_thickness: 0.2,
                openings: vec![
                    Opening {
                        side: Side::West,
                        offset: 0.9,
                        width: 1.2,
                        sill: 0.0,
                        door: false,
                    },
                    Opening {
                        side: Side::West,
                        offset: -0.4,
                        width: 1.8,
                        sill: 3.1,
                        door: false,
                    },
                ],
                floor: true,
                ceiling: true,
            },
            // Upper room, stacked on the ground room.
            RoomSpec {
                origin: Vec3::new(0.0, 3.0, 0.0),
                interior: Vec3::new(6.0, 3.0, 8.0),
                wall_thickness: 0.2,
                openings: vec![Opening {
                    side: Side::East,
                    offset: -0.4,
                    width: 1.8,
                    sill: 0.0,
                    door: false,
                }],
                floor: true,
                ceiling: true,
            },
        ],
        // Bottom step at x 6.7 leaves 1.2m of flat floor before the
        // east wall (interior edge 7.9); the run tops out at 3.7 and
        // the landing bridges the wall band into the upper room.
        stairs: vec![StairSpec {
            base: Vec3::new(6.7, 0.0, -0.4),
            side: Side::West,
            width: 1.8,
            rise: 3.1,
            landing: 0.5,
        }],
        // The loot crate upstairs is spawned by `setup` through
        // `spawn_container`, not as furniture: it has an inventory.
        furniture: vec![],
        lights: vec![
            LightSpec {
                position: Vec3::new(0.0, 2.6, 0.0),
                color: [1.0, 0.9, 0.7],
                intensity: 400_000.0,
            },
            LightSpec {
                position: Vec3::new(5.65, 5.4, 0.0),
                color: [1.0, 0.9, 0.7],
                intensity: 400_000.0,
            },
            LightSpec {
                position: Vec3::new(0.0, 5.6, 0.0),
                color: [1.0, 0.9, 0.7],
                intensity: 400_000.0,
            },
        ],
    }
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut cel_materials: ResMut<Assets<CelMaterial>>,
    items: Res<ItemRegistryRes>,
    monuments: Res<MonumentRegistryRes>,
    biomes: Res<BiomeRegistryRes>,
    world_name: Res<WorldName>,
    time: Res<Time>,
) {
    spawn_structure(
        &mut commands,
        &mut meshes,
        &mut cel_materials,
        &starting_bunker(),
        Vec3::ZERO,
    )
    .expect("the starting bunker def is valid");
    let house_origin = Vec3::new(0.0, 0.0, -30.0);
    spawn_structure(
        &mut commands,
        &mut meshes,
        &mut cel_materials,
        &two_story_house(),
        house_origin,
    )
    .expect("the two-story house def is valid");

    // Starting supplies on the bunker floor: three days of food as
    // real items (design.md). A can restores half a day of hunger
    // and a bottle half a day of thirst, so six of each is three
    // days. No quality tiers exist yet, so the odds are empty and
    // every stack is base quality.
    let now = time.elapsed_secs();
    let def = |name: &str| {
        items
            .0
            .def(name)
            .unwrap_or_else(|| panic!("item '{name}' is not registered"))
    };
    let floor = [
        ("canned food", 6, 1, Vec3::new(-1.0, 0.3, 2.0), [0.2, 0.6, 0.2]),
        ("water bottle", 5, 3, Vec3::new(-1.0, 0.3, 2.6), [0.3, 0.5, 0.8]),
        ("water bottle", 1, 4, Vec3::new(-1.6, 0.3, 2.6), [0.3, 0.5, 0.8]),
        ("scrap", 3, 2, Vec3::new(1.0, 0.3, 2.0), [0.5, 0.5, 0.5]),
        // The first gun, a pipe, and a vest: enough to meet the
        // scavenger at the door (todo row 8).
        ("pipe shotgun", 1, 9, Vec3::new(1.6, 0.3, 2.0), [0.3, 0.3, 0.3]),
        ("shotgun shell", 8, 10, Vec3::new(1.6, 0.3, 2.6), [0.8, 0.3, 0.2]),
        ("pipe", 1, 11, Vec3::new(1.0, 0.3, 2.6), [0.4, 0.4, 0.45]),
        ("road sign vest", 1, 12, Vec3::new(2.2, 0.3, 2.0), [0.2, 0.5, 0.2]),
    ];
    for (name, count, salt, position, color) in floor {
        spawn_item(
            &mut commands,
            &mut meshes,
            &mut cel_materials,
            def(name),
            create(def(name), count, &[], now, salt),
            position,
            color,
        );
    }

    // The world's seed changes with every launch (design.md "The
    // world"); the storm will roll another. The note and the world
    // both come from it.
    let seed = now.to_bits() as u64 ^ 0xA5A5;

    // The first question, on the bench (design.md "The first hour"):
    // the note that tells you not to go outside.
    spawn_item(
        &mut commands,
        &mut meshes,
        &mut cel_materials,
        def("note"),
        write(def("note"), starting_note(seed), now, 13).expect("the note def is a note"),
        Vec3::new(2.2, 0.9 + 0.2, 3.5),
        [0.9, 0.88, 0.8],
    );

    // The storage box upstairs in the house, with loot in it: a
    // deployed WorldItem whose container is filled after spawn.
    let mut loot = Inventory::new(def("storage box").storage.expect("a box has storage"));
    for (name, count, salt) in [("scrap", 8, 5), ("cloth", 4, 6), ("pipe", 1, 7)] {
        let rest = loot.add(create(def(name), count, &[], now, salt), def(name).max_stack);
        debug_assert!(rest.is_none(), "the box has room for its loot");
    }
    let box_ = spawn_item(
        &mut commands,
        &mut meshes,
        &mut cel_materials,
        def("storage box"),
        create(def("storage box"), 1, &[], now, 8),
        house_origin + Vec3::new(0.0, 3.45, 2.5),
        [0.55, 0.45, 0.20],
    );
    commands.entity(box_).insert(Container(loot));

    // The world rolls around the bunker. Terrain, roads, scatter, and
    // every monument come from the one path in terrain.rs.
    let def = world_def(&world_name.0).expect("the world name was checked at startup");
    let world = roll_world(&def, seed, &biomes.0, &monuments.monuments)
        .expect("the world def rolls with the registered content");
    spawn_world(
        &mut commands,
        &mut meshes,
        &mut cel_materials,
        &world,
        &biomes.0,
        &monuments.monuments,
        &monuments.buildings,
        &items.0,
        now,
    )
    .expect("a rolled world spawns");
    commands.insert_resource(WorldSeed(world.seed));
    commands.insert_resource(WorldRes(world));
}
