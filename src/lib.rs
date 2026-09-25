//! topside: the 2D game (docs/design.md "What 2D changes") on Bevy,
//! consuming the modforge crate and the content both games share.
//! Ownership map: docs/authority.md.

pub mod actor;
pub mod ai;
pub mod combat;
pub mod control_plane;
pub mod hud;
pub mod input;
pub mod interact;
pub mod player;
pub mod sight;
pub mod structure;
pub mod survival;
pub mod terrain;

pub use topside_content as content;

use bevy::camera::ScalingMode;
use bevy::prelude::*;

/// Metres across the screen's height, close to Factorio's default zoom.
pub const VIEW_METRES: f32 = 34.0;

/// Simulation tick counter. FixedUpdate runs at a locked 60 UPS; this
/// counts those ticks, independent of the frame rate.
#[derive(Resource, Default)]
pub struct SimTick(pub u64);

/// The seed the world rolls from.
#[derive(Resource)]
pub struct WorldSeed(pub u64);

/// Which reality the world rolls from (`content::WORLDS`).
#[derive(Resource)]
pub struct WorldName(pub String);

impl Default for WorldName {
    fn default() -> Self {
        Self(content::WORLDS[0].to_string())
    }
}

/// Build the app. `headless` skips the window and the drawing so
/// tests can run the simulation bare.
pub fn build_app(headless: bool, seed: u64) -> App {
    let mut app = App::new();
    if headless {
        app.add_plugins(MinimalPlugins);
    } else {
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "topside".to_string(),
                ..default()
            }),
            ..default()
        }));
    }
    // Avian 2D is the one physics system. Seen from above, nothing
    // falls.
    app.add_plugins(avian2d::prelude::PhysicsPlugins::default());
    app.insert_resource(avian2d::prelude::Gravity(Vec2::ZERO));
    if headless {
        // Headless callers drive `update()` themselves and never
        // `run()`, so finish plugin setup here (Avian creates its
        // resources in `Plugin::finish`).
        app.finish();
        app.cleanup();
    }
    app.insert_resource(Time::<Fixed>::from_hz(60.0));
    app.insert_resource(WorldSeed(seed));
    app.init_resource::<SimTick>();
    app.init_resource::<WorldName>();
    app.init_resource::<content::BiomeRegistryRes>();
    app.init_resource::<content::MonumentRegistryRes>();
    app.init_resource::<content::ItemRegistryRes>();
    app.init_resource::<content::DamageRegistryRes>();
    app.init_resource::<content::ActorRegistryRes>();
    app.init_resource::<content::FactionRegistryRes>();
    app.init_resource::<actor::ActorMapRes>();
    app.init_resource::<input::BindingsRes>();
    app.init_resource::<interact::HudRes>();
    app.init_resource::<interact::OpenContainer>();
    app.init_resource::<hud::HoveredSlot>();
    app.init_resource::<survival::Rates>();
    app.init_resource::<content::RecipeRegistryRes>();
    app.init_resource::<interact::OpenStation>();
    app.add_systems(
        Startup,
        (
            terrain::roll,
            (
                terrain::spawn_solids,
                structure::spawn_bunker,
                structure::spawn_monuments,
                player::spawn_player,
            ),
            (actor::populate_monuments, actor::spawn_door_enemy),
        )
            .chain(),
    );
    // Every fixed step: count it, drain each actor's due actions, then
    // every system reads actions, never keys.
    app.add_systems(
        FixedUpdate,
        (
            tick_system,
            ai::think,
            input::drain_actions,
            survival::survival_tick,
            survival::waste,
            hud::hud_keys,
            player::roll,
            player::aim,
            interact::interact_on_use,
            player::movement,
            combat::melee,
            combat::fire,
            combat::fly,
            combat::die,
            player::respawn,
            control_plane::publish,
        )
            .chain(),
    );
    // What the player's person can see, every frame.
    app.add_systems(Update, sight::player_sight);
    if !headless {
        app.add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default());
        control_plane::start();
        app.add_systems(
            Startup,
            (
                terrain::spawn_ground,
                spawn_camera,
                hud::setup,
                hud::setup_death,
                hud::setup_vitals,
                hud::setup_station,
                hud::setup_night,
            )
                .after(terrain::roll),
        );
        // The ONE place raw input is read; headless tests push actions
        // themselves.
        app.add_systems(
            Update,
            (
                input::read_input,
                player::camera_follow,
                hud::render,
                hud::render_death,
                hud::render_vitals,
                hud::render_station,
                hud::render_night,
            ),
        );
    }
    app
}

fn tick_system(mut tick: ResMut<SimTick>) {
    tick.0 += 1;
}

/// The camera looks straight down, like Factorio's, starting at the
/// bunker.
fn spawn_camera(mut commands: Commands, world: Res<terrain::WorldRes>) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: VIEW_METRES,
            },
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_translation(world.0.def.bunker.extend(0.0)),
    ));
}
