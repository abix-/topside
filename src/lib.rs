//! topside: one 3D world played in first person (docs/authority.md
//! "Presentation") on Bevy, consuming the modforge crate and the content.
//! Ownership map: docs/authority.md.

pub mod actor;
pub mod ai;
pub mod combat;
pub mod control_plane;
pub mod episode;
pub mod held;
pub mod hud;
pub mod input;
pub mod interact;
pub mod minimap;
pub mod model;
pub mod nav;
pub mod people;
pub mod player;
pub mod profile;
pub mod sight;
pub mod storm;
pub mod structure;
pub mod survival;
pub mod talk;
pub mod terrain;

pub use topside_content as content;

use bevy::prelude::*;

/// A place in the world seen from above, and where it stands in 3D. The
/// simulation reasons on the ground (x, y), as modforge does; in 3D, Bevy's
/// y is up, so a ground point (x, y) stands at (x, height, y), the same
/// way modforge's memory keeps places (`Vec3::new(x, 0.0, y)`).
pub trait Ground {
    /// The ground point under this 3D position.
    fn ground(self) -> Vec2;
}

impl Ground for Vec3 {
    fn ground(self) -> Vec2 {
        Vec2::new(self.x, self.z)
    }
}

/// The 3D position `height` metres above the ground point `g`.
pub fn on_ground(g: Vec2, height: f32) -> Vec3 {
    Vec3::new(g.x, height, g.y)
}

/// The turn about the upright that points a thing's +x along the ground
/// direction at `angle` (radians, 0 along +x, a quarter turn along +y).
pub fn turned(angle: f32) -> Quat {
    Quat::from_rotation_y(-angle)
}

/// Where a person's eyes are, above their feet (the first person camera).
pub const EYE_HEIGHT: f32 = 1.6;

/// Simulation tick counter. FixedUpdate runs at a locked 60 UPS; this
/// counts those ticks, independent of the frame rate.
#[derive(Resource, Default)]
pub struct SimTick(pub u64);

/// The seed the world rolls from.
#[derive(Resource)]
pub struct WorldSeed(pub u64);

/// The reality the world is in now (realities.md), with its rules: the
/// first registered until a storm hands over another.
#[derive(Resource)]
pub struct Reality(pub modforge::reality::RealityDef);

impl FromWorld for Reality {
    fn from_world(world: &mut World) -> Self {
        let realities = world.get_resource_or_init::<content::RealityRegistryRes>();
        Self(realities.0.pick(0, 0, "").expect("content registers at least one reality").clone())
    }
}

pub use episode::Episode;

/// Build the app. `headless` skips the window and the drawing so
/// tests can run the simulation bare; `world` is the 2D world to make.
pub fn build_app(headless: bool, seed: u64, world: content::WorldKind) -> App {
    let mut app = App::new();
    if headless {
        app.add_plugins(MinimalPlugins);
    } else {
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "topside".to_string(),
                        ..default()
                    }),
                    ..default()
                })
                // The repo's assets folder, wherever the exe is built to
                // (the build target is shared, so next to the exe is not
                // the repo).
                .set(AssetPlugin {
                    file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets").to_string(),
                    ..default()
                }),
        );
    }
    // Avian 3D is the one physics system. Bodies are kinematic and walk
    // the ground; nothing falls yet (one floor, flat ground).
    app.add_plugins(avian3d::prelude::PhysicsPlugins::default());
    app.add_plugins(profile::ProfilePlugin);
    app.insert_resource(avian3d::prelude::Gravity(Vec3::ZERO));
    if headless {
        // Headless callers drive `update()` themselves and never
        // `run()`, so finish plugin setup here (Avian creates its
        // resources in `Plugin::finish`).
        app.finish();
        app.cleanup();
    }
    app.insert_resource(Time::<Fixed>::from_hz(60.0));
    app.insert_resource(WorldSeed(seed));
    app.insert_resource(world);
    app.init_resource::<SimTick>();
    app.init_resource::<content::RealityRegistryRes>();
    app.init_resource::<Reality>();
    app.init_resource::<content::EpisodeRegistryRes>();
    app.init_resource::<Episode>();
    app.init_resource::<content::BiomeRegistryRes>();
    app.init_resource::<content::MonumentRegistryRes>();
    app.init_resource::<content::ItemRegistryRes>();
    app.init_resource::<content::LayerRegistryRes>();
    app.init_resource::<content::DamageRegistryRes>();
    app.init_resource::<content::ActorRegistryRes>();
    app.init_resource::<content::FactionRegistryRes>();
    app.init_resource::<content::brain::BrainTreeRes>();
    app.init_resource::<actor::ActorMapRes>();
    app.init_resource::<input::BindingsRes>();
    app.init_resource::<hud::HoveredSlot>();
    app.init_resource::<nav::Walk>();
    app.init_resource::<sight::BunkerMaps>();
    app.init_resource::<nav::PlanQueue>();
    app.init_resource::<nav::DoorIndex>();
    app.init_resource::<nav::FailedWays>();
    app.init_resource::<survival::Day>();
    app.init_resource::<content::RecipeRegistryRes>();
    app.init_resource::<storm::StormRes>();
    app.add_systems(
        Startup,
        (
            terrain::roll,
            (structure::spawn_bunker, structure::spawn_other_bunkers, player::spawn_player),
            terrain::make_chunks,
            structure::spawn_sites,
            // The walk grid knows the world before anyone is put in it.
            nav::record_solids,
            (
                actor::populate_monuments,
                actor::populate_topside_people,
                actor::populate_bunkers,
                actor::spawn_door_enemy,
            ),
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
                    actor::populate_topside_people,
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
                (storm::storm, episode::cast_episode, episode::place_the_tap, episode::dell_tells_of_the_tap, episode::the_first_drink).chain(),
                lap("storm"),
                hud::hud_keys,
                player::roll,
                player::look,
                player::aim,
                lap("keys_roll_aim"),
            )
                .chain(),
            (
                (interact::interact_on_use, talk::talk).chain(),
                lap("interact"),
                (player::movement, sight::look_around, sight::share_maps).chain(),
                lap("movement"),
                nav::doorways,
                lap("doorways"),
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
                (control_plane::answer_npc, control_plane::answer_kill, control_plane::answer_use, control_plane::answer_talk).chain(),
                lap("publish"),
            )
                .chain(),
        )
            .chain(),
    );
    app.add_systems(Update, profile::start);
    if !headless {
        app.insert_resource(storm::Windowed);
        app.add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default());
        control_plane::start();
        app.add_systems(Startup, terrain::setup_ground.before(terrain::make_chunks));
        app.add_systems(Startup, minimap::setup_map);
        app.add_systems(FixedUpdate, minimap::toggle_map.after(input::drain_actions));
        app.add_systems(Update, minimap::draw_map);
        app.add_plugins(MaterialPlugin::<model::Cel>::default());
        app.init_resource::<model::Tinted>();
        app.init_resource::<model::CelShaded>();
        app.init_resource::<people::Animations>();
        app.add_systems(
            Update,
            (
                spawn_camera,
                input::cursor_grab,
                people::draw_people,
                model::draw_item_layers,
                model::attach,
                (model::tint, people::dress).before(model::cel_shade),
                people::redress,
                model::cel_shade,
                people::animate,
                held::hold,
                held::swing,
                control_plane::take_screenshot,
                control_plane::publish_view,
                control_plane::publish_items,
                control_plane::publish_hud,
            ),
        );
        app.add_systems(
            Startup,
            (
                spawn_sun,
                hud::setup,
                hud::setup_death,
                hud::setup_vitals,
                hud::setup_station,
                hud::setup_talk,
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
                hud::render,
                hud::render_item_panel,
                hud::render_death,
                hud::render_vitals,
                hud::render_station,
                hud::render_talk,
                hud::render_sky,
            ),
        );
    }
    app
}

fn tick_system(mut tick: ResMut<SimTick>) {
    tick.0 += 1;
}

/// The sun, shining from `model::SUN` (the cel shade's light), no shadows
/// (design.md), and a soft light everywhere so nothing is black.
fn spawn_sun(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            illuminance: 10_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::default().looking_to(-model::SUN, Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight {
        brightness: 400.0,
        ..default()
    });
}

/// The first person camera: at the player's eyes, looking where they face
/// (the camera looks along its -z; the player's +x is where they face).
fn spawn_camera(mut commands: Commands, players: Query<Entity, Added<player::Player>>) {
    for player in &players {
        commands.entity(player).with_children(|c| {
            c.spawn((
                Camera3d::default(),
                player::Eyes::default(),
                Transform::from_xyz(0.0, EYE_HEIGHT - player::BODY_CENTRE, 0.0)
                    .with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)),
            ));
        });
    }
}
