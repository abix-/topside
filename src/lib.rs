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
pub mod nav;
pub mod player;
pub mod profile;
pub mod sight;
pub mod storm;
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
    app.add_plugins(profile::ProfilePlugin);
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
    app.init_resource::<hud::HoveredSlot>();
    app.init_resource::<nav::Walk>();
    app.init_resource::<nav::PlanQueue>();
    app.init_resource::<nav::DoorIndex>();
    app.init_resource::<nav::FailedWays>();
    app.init_resource::<survival::Day>();
    app.init_resource::<content::RecipeRegistryRes>();
    app.init_resource::<storm::StormRes>();
    app.init_resource::<ai::LifeStats>();
    app.add_systems(
        Startup,
        (
            terrain::roll,
            (structure::spawn_bunker, structure::spawn_other_bunkers, player::spawn_player),
            terrain::make_chunks,
            structure::spawn_sites,
            // The walk grid knows the world before anyone is put in it.
            nav::record_solids,
            (actor::populate_monuments, actor::populate_bunkers, actor::spawn_door_enemy),
        )
            .chain(),
    );
    // Every fixed step: count it, drain each actor's due actions, then
    // every system reads actions, never keys. A lap after each system
    // times it (profile.rs).
    use profile::lap;
    app.add_systems(
        FixedUpdate,
        (
            (
                profile::start_step,
                tick_system,
                // The chunks the world needs now, their sites, and the
                // sites' people.
                (
                    terrain::make_chunks,
                    structure::spawn_sites,
                    actor::populate_monuments,
                    lap("chunks"),
                )
                    .chain(),
                ai::think,
                lap("think"),
                input::drain_actions,
                lap("drain_actions"),
                (nav::record_solids, nav::record_doors),
                nav::steer,
                lap("steer"),
                survival::survival_tick,
                survival::waste,
                lap("survival"),
                storm::storm,
                lap("storm"),
                hud::hud_keys,
                player::roll,
                player::aim,
                lap("keys_roll_aim"),
            )
                .chain(),
            (
                interact::interact_on_use,
                lap("interact"),
                player::movement,
                lap("movement"),
                combat::melee,
                lap("melee"),
                combat::fire,
                combat::fly,
                lap("guns"),
                combat::die,
                player::respawn,
                lap("death"),
                control_plane::publish,
                control_plane::publish_world,
                control_plane::publish_ways,
                control_plane::answer_npc,
                lap("publish"),
            )
                .chain(),
        )
            .chain(),
    );
    // What the player's person can see, every frame.
    app.add_systems(Update, (profile::start, sight::player_sight, lap("player_sight")).chain());
    if !headless {
        app.insert_resource(storm::Windowed);
        app.add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default());
        control_plane::start();
        app.add_systems(Startup, terrain::setup_ground_tiles.before(terrain::make_chunks));
        app.add_systems(
            Startup,
            (
                spawn_camera,
                hud::setup,
                hud::setup_death,
                hud::setup_vitals,
                hud::setup_station,
                hud::setup_sky,
            )
                .after(terrain::roll),
        );
        // The ONE place raw input is read; headless tests push actions
        // themselves.
        app.add_systems(
            Update,
            (
                control_plane::apply_speed,
                input::read_input,
                player::camera_follow,
                hud::render,
                hud::render_death,
                hud::render_vitals,
                hud::render_station,
                hud::render_sky,
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
        Transform::from_translation(world.bunker.extend(0.0)),
    ));
}
