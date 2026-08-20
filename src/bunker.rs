//! The opening bunker (design.md "The bunker"), expressed as a
//! StructureDef: pure data through the structure vocabulary, the
//! same path every generated building will use. Also owns the food
//! clock, the first survival pressure.

use bevy::prelude::*;

use crate::structure::{
    LightSpec, Opening, RoomSpec, Side, SolidSpec, StaticColliders, StructureDef, spawn_structure,
};

/// One game day in real seconds (Valheim-scale pacing; tune later).
pub const GAME_DAY_SECS: f32 = 1200.0;

/// The first clock: three days of food forcing the player topside.
#[derive(Resource)]
pub struct FoodStore {
    pub days: f32,
}

impl Default for FoodStore {
    fn default() -> Self {
        Self { days: 3.0 }
    }
}

pub fn food_tick_system(time: Res<Time>, mut food: ResMut<FoodStore>) {
    food.days = (food.days - time.delta_secs() / GAME_DAY_SECS).max(0.0);
}

/// The starting bunker: a room with exactly a bed, a crafting
/// bench, and the food crate (design.md: three things), door north
/// toward topside.
pub fn starting_bunker() -> StructureDef {
    StructureDef {
        name: "the starting bunker".to_string(),
        room: RoomSpec {
            interior: Vec3::new(6.0, 3.0, 8.0),
            wall_thickness: 0.2,
            openings: vec![Opening {
                side: Side::North,
                offset: 0.0,
                width: 1.2,
                door: true,
            }],
        },
        furniture: vec![
            SolidSpec {
                center: Vec3::new(-2.4, 0.25, 3.0),
                size: Vec3::new(1.0, 0.5, 2.0),
                color: Color::srgb(0.30, 0.35, 0.55),
            },
            SolidSpec {
                center: Vec3::new(2.2, 0.45, 3.5),
                size: Vec3::new(1.6, 0.9, 0.7),
                color: Color::srgb(0.45, 0.32, 0.18),
            },
            SolidSpec {
                center: Vec3::new(2.5, 0.35, -3.3),
                size: Vec3::new(0.7, 0.7, 0.7),
                color: Color::srgb(0.55, 0.45, 0.20),
            },
        ],
        lights: vec![LightSpec {
            position: Vec3::new(0.0, 2.7, 0.0),
            color: Color::srgb(1.0, 0.9, 0.7),
            intensity: 400_000.0,
        }],
    }
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut colliders: ResMut<StaticColliders>,
) {
    spawn_structure(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut colliders,
        &starting_bunker(),
        Vec3::ZERO,
    );
}
