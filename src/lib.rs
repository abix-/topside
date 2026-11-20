//! topside: one 3D world played in first person (docs/authority.md
//! "Presentation") on Bevy, consuming the modforge crate and the content.
//! Ownership map: docs/authority.md.

pub mod actor;
pub mod ai;
pub mod anomaly;
pub mod combat;
pub mod command;
pub mod control_plane;
pub mod edge;
pub mod episode;
pub mod held;
pub mod hud;
pub mod input;
pub mod interact;
pub mod joining;
pub mod loading;
pub mod minimap;
pub mod model;
pub mod nav;
pub mod navmesh;
pub mod outline;
pub mod people;
pub mod player;
pub mod profile;
pub mod radio;
pub mod randy;
pub mod save;
pub mod secrets;
pub mod sight;
pub mod storm;
pub mod structure;
pub mod survival;
pub mod talk;
pub mod terrain;
pub mod trade;

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
#[derive(Resource, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct SimTick(pub u64);

/// The seed the world rolls from.
#[derive(Resource)]
pub struct WorldSeed(pub u64);

/// The seed the game was launched with: which run this is. A storm rolls a
/// new world seed, but the run stays the same; its replays and the saves it
/// makes on its own are named by this.
#[derive(Resource, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct RunSeed(pub u64);

/// The reality the world is in now (realities.md), with its rules: the
/// first registered until a storm hands over another.
#[derive(Resource, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct Reality(pub modforge::reality::RealityDef);

impl FromWorld for Reality {
    fn from_world(world: &mut World) -> Self {
        let realities = world.get_resource_or_init::<content::RealityRegistryRes>();
        Self(realities.0.pick(0, 0, "").expect("content registers at least one reality").clone())
    }
}

pub use episode::Episode;

/// Every physics body whose Transform changed gets its GlobalTransform
/// worked out now, from its own Transform and its parents' (a door under
/// its building too), before Avian reads them; nothing else in the tree
/// under it. In Avian's own Propagate set, in place of its whole-tree
/// propagation (`PhysicsTransformConfig::propagate_before_physics`, off).
fn bodies_placed(
    helper: bevy::transform::helper::TransformHelper,
    mut bodies: Query<(Entity, &mut GlobalTransform), (With<avian3d::prelude::RigidBody>, Changed<Transform>)>,
) {
    for (entity, mut global) in &mut bodies {
        if let Ok(placed) = helper.compute_global_transform(entity) {
            *global = placed;
        }
    }
}

/// Build the app. `headless` skips the window and the drawing so
/// tests can run the simulation bare; `world` is the 2D world to make;
/// `port` is the control plane's.
pub fn build_app(headless: bool, seed: u64, world: content::WorldKind, port: u16) -> App {
    let mut app = App::new();
    if headless {
        app.add_plugins(MinimalPlugins);
    } else {
        app.add_plugins(
            DefaultPlugins
                .set(bevy::render::RenderPlugin {
                    render_creation: {
                        let mut settings = bevy::render::settings::WgpuSettings::default();
                        // On this Windows renderer, Vulkan buffer uploads spent
                        // hundreds of ms in the driver; DX12 stayed near 60 FPS.
                        // Keep WGPU_BACKEND available for measured comparisons.
                        if cfg!(target_os = "windows") && std::env::var_os("WGPU_BACKEND").is_none() {
                            settings.backends = Some(bevy::render::settings::Backends::DX12);
                        }
                        settings.into()
                    },
                    ..default()
                })
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
                })
                // Every span Bevy runs timed, for the `perf` op (profile.rs).
                .set(bevy::log::LogPlugin { custom_layer: profile::layer, ..default() })
                // Chunk meshes are built on Bevy's async compute threads
                // (todo 9s): by default a quarter of the cores, at most 4,
                // so a load's 1156 meshes queued on 2 of 8. Every core for
                // them, and the compute pool, which runs the frame's own
                // systems, keeps every core too: the async threads sit idle
                // unless meshes are building. Each mesh still lands at its
                // tick (authority.md rule 5).
                .set(bevy::app::TaskPoolPlugin {
                    task_pool_options: {
                        let cores = bevy::tasks::available_parallelism();
                        let mut options = bevy::app::TaskPoolOptions::default();
                        options.async_compute.min_threads = cores;
                        options.async_compute.max_threads = cores;
                        options.compute.min_threads = cores;
                        options
                    },
                }),
        );
        // Bevy quits on every render error (bevy_render error_handler.rs,
        // its default handler). DX12 failing to resize the swap chain while
        // the window is being resized or minimized ("window is in use",
        // wgpu-hal dx12) is not a broken renderer: the next frame
        // configures the surface again. wgpu reports it as a Validation
        // error from Surface::configure whose cause reads "Invalid surface"
        // (wgpu-core present.rs; the "window is in use" words only reach
        // the log). That one error is let pass; every other still quits
        // (todo: keep the game running through a window error, 2026-11-19).
        app.insert_resource(bevy::render::error_handler::RenderErrorHandler(|error, main_world, _| {
            if error.description.contains("Surface::configure") || error.description.contains("Invalid surface") {
                bevy::log::warn!("render error let pass, the window could not be configured: {}", error.description);
                return bevy::render::error_handler::RenderErrorPolicy::Ignore;
            }
            bevy::log::error!("Quitting the application due to {:?} RenderError: {}", error.ty, error.description);
            main_world.write_message(bevy::app::AppExit::error());
            bevy::render::error_handler::RenderErrorPolicy::StopRendering
        }));
    }
    // Avian 3D is the one physics system. Bodies are kinematic and walk
    // the ground; nothing falls yet (one floor, flat ground).
    app.add_plugins(avian3d::prelude::PhysicsPlugins::default());
    // Before each physics step Avian needs every body's GlobalTransform
    // current (its transform_to_position reads it), and by default it
    // propagates every transform tree to get it: a person who moved has
    // their whole drawn body under them (rig bones, worn pieces, what they
    // hold) worked out again, every tick. Only the bodies are needed, so
    // only theirs is worked out (`bodies_placed`); the drawn trees are
    // propagated once a frame by Bevy in PostUpdate, which only drawing
    // reads. Avian's own config for this: `propagate_before_physics`.
    app.insert_resource(avian3d::physics_transform::PhysicsTransformConfig { propagate_before_physics: false, ..default() });
    app.add_systems(FixedPostUpdate, bodies_placed.in_set(avian3d::physics_transform::PhysicsTransformSystems::Propagate));
    // The navigation mesh from those colliders (docs/pathing.md).
    app.add_plugins(navmesh::NavPlugin);
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
    app.insert_resource(RunSeed(seed));
    app.insert_resource(world);
    app.init_resource::<SimTick>();
    app.init_resource::<content::RealityRegistryRes>();
    app.init_resource::<Reality>();
    app.init_resource::<content::EpisodeRegistryRes>();
    app.init_resource::<Episode>();
    app.init_resource::<talk::Lines>();
    app.init_resource::<content::BiomeRegistryRes>();
    app.init_resource::<content::MonumentRegistryRes>();
    app.init_resource::<content::ItemRegistryRes>();
    app.init_resource::<content::ItemValuesRes>();
    app.init_resource::<trade::PeopleTrades>();
    app.init_resource::<ai::Warnings>();
    app.init_resource::<joining::Joins>();
    app.init_resource::<edge::EdgeRes>();
    app.init_resource::<edge::WordToEdge>();
    app.init_resource::<randy::RandyRes>();
    app.init_resource::<content::LayerRegistryRes>();
    app.init_resource::<content::DamageRegistryRes>();
    app.init_resource::<content::ActorRegistryRes>();
    app.init_resource::<content::FactionRegistryRes>();
    app.init_resource::<content::brain::BrainTreeRes>();
    app.init_resource::<actor::ActorMapRes>();
    app.init_resource::<input::BindingsRes>();
    // Things' own ids, lasting across save and load (interact::ThingId).
    app.init_resource::<interact::ThingIds>();
    app.init_resource::<interact::ThingIssuer>();
    app.add_observer(interact::index_thing_ids);
    app.add_observer(interact::unindex_thing_ids);
    app.add_observer(interact::item_body);
    app.add_observer(interact::door_body);
    save::plugin(&mut app);
    app.init_resource::<hud::HoveredSlot>();
    app.init_resource::<nav::Walk>();
    app.init_resource::<sight::BunkerMaps>();
    app.init_resource::<nav::DoorIndex>();
    app.init_resource::<survival::Day>();
    app.init_resource::<content::RecipeRegistryRes>();
    app.init_resource::<storm::StormRes>();
    app.init_resource::<anomaly::AnomalyPlaced>();
    app.init_resource::<secrets::SecretsPlaced>();
    app.init_resource::<command::GivenThisRun>();
    app.init_resource::<command::Playing>();
    app.init_resource::<command::DoneThisRun>();
    app.init_resource::<command::PlayingDone>();
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
    // The loading screen (authority.md "Running tests": loading): from the
    // launch, and after a load, until the world is ready; no tick runs
    // while it shows.
    app.insert_resource(loading::Loading::start());
    app.configure_sets(RunFixedMainLoop, RunFixedMainLoopSystems::FixedMainLoop.run_if(loading::not_loading));
    app.add_systems(Update, loading::ready_the_world);
    // Every fixed step: count it, drain each actor's due actions, then
    // every system reads actions, never keys. A lap after each system
    // times it (profile.rs).
    use profile::lap;
    app.add_systems(
        FixedUpdate,
        (
            (
                tick_system,
                // The chunks the world needs now, their sites, and the
                // sites' people.
                (
                    terrain::make_chunks,
                    structure::spawn_sites,
                    terrain::open_the_ground,
                    actor::populate_monuments,
                    actor::populate_topside_people,
                    lap("chunks"),
                )
                    .chain(),
                ai::think,
                (trade::between_people, ai::warn_off).chain(),
                lap("think"),
                (input::drain_actions, lap("drain_actions"), hud::learn_hints, episode::cues.run_if(loading::awake), command::give, command::carry_out, ai::follow, ai::lead, save::save_at_steps).chain(),
                (nav::record_solids, nav::record_doors),
                // Everyone sent somewhere walks the way on the navigation
                // mesh (docs/pathing.md).
                navmesh::walk,
                lap("walk"),
                survival::survival_tick,
                survival::waste,
                survival::rot,
                lap("survival"),
                (
                    // The episode and its storm start when the player
                    // wakes, after the world's history.
                    (
                        storm::storm,
                        anomaly::place,
                        anomaly::burn,
                        anomaly::throw,
                        anomaly::fly,
                        anomaly::flare,
                        secrets::place,
                        radio::clicks,
                        episode::cast_episode,
                        episode::name_the_cast,
                        episode::place_things,
                        episode::pivots_said,
                        episode::held_conditions,
                        episode::errands,
                        episode::deeds,
                        episode::the_ending,
                    )
                        .chain()
                        .run_if(loading::awake),
                    talk::lines,
                )
                    .chain(),
                lap("storm"),
                hud::hud_keys,
                player::roll,
                player::look,
                player::aim,
                lap("keys_roll_aim"),
            )
                .chain(),
            (
                (
                    talk::look_at,
                    interact::interact_on_use,
                    hud::close_read_notes,
                    hud::show_act_result,
                    interact::knock,
                    talk::talk,
                    trade::trade,
                    talk::conversations,
                    talk::end_conversations,
                    talk::speak_up,
                    talk::word_spreads,
                    // Joining (todo 11af): agreed in talk, or leaving for a
                    // group felt better toward; made after.
                    joining::leave_for_better,
                    joining::join,
                    // Word of a place reaching the edge brings newcomers
                    // (todo 11i).
                    edge::newcomers,
                    // While the player walks topside, Randy Random rolls
                    // encounters (todo 10h1).
                    randy::encounters.run_if(loading::awake),
                )
                    .chain(),
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
                combat::knock_back,
                combat::die,
                player::respawn,
                lap("death"),
                control_plane::publish,
                control_plane::publish_world,
                control_plane::publish_ways,
                (control_plane::answer_kill, control_plane::answer_give, control_plane::answer_needs, control_plane::answer_acts, control_plane::answer_act_results).chain(),
                lap("publish"),
            )
                .chain(),
        )
            .chain(),
    );
    app.add_systems(Update, profile::start);
    // Saving and loading answer every frame, paused or not; stepping a
    // paused game hands the fixed loop its ticks before it runs and answers
    // after.
    app.add_systems(First, save::answer_save);
    // The ops that only read answer every frame, paused or not; the ops
    // that change the game act at the next tick (in the fixed chain).
    app.add_systems(Update, (control_plane::answer_npc, control_plane::answer_walk, control_plane::answer_ground_on_floors));
    app.add_systems(
        RunFixedMainLoop,
        (
            control_plane::step_ticks.in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
            control_plane::answer_step.in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
        ),
    );
    // A conversation ends on real time, so it can be read at any speed.
    if !headless {
        app.insert_resource(storm::Windowed);
        // 60 frames a second, as the world's 60 updates (Factorio's UPS):
        // drawn faster, the frames cost the CPU and show nothing new.
        app.add_plugins(bevy_framepace::FramepacePlugin);
        app.insert_resource(bevy_framepace::FramepaceSettings { limiter: bevy_framepace::Limiter::from_framerate(60.0) });
        app.add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default());
        control_plane::start(port);
        // The `pointer` and `key` ops drive the game as the player's hands
        // do: after Bevy reads the real input, before picking reads pointers.
        app.add_systems(Startup, control_plane::spawn_driven_pointer);
        app.add_systems(
            PreUpdate,
            control_plane::drive_input
                .after(bevy::input::InputSystems)
                .before(bevy::picking::PickingSystems::Input),
        );
        app.add_systems(Startup, terrain::setup_ground.before(terrain::make_chunks));
        app.add_systems(Startup, minimap::setup_map);
        app.add_systems(FixedUpdate, minimap::toggle_map.after(input::drain_actions));
        app.add_systems(Update, minimap::draw_map);
        app.add_plugins(MaterialPlugin::<model::Cel>::default());
        app.add_plugins(outline::OutlinePlugin);
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
                combat::flash,
                control_plane::take_screenshot,
                control_plane::apply_window,
                // After the cel pass, so a mesh that appeared this frame is
                // counted with the material it is drawn with.
                control_plane::publish_view.after(model::cel_shade),
                control_plane::publish_items,
                control_plane::publish_hud,
            ),
        );
        app.add_systems(
            Startup,
            (
                spawn_sun,
                combat::setup_flash,
                hud::setup,
                hud::setup_death,
                hud::setup_vitals,
                hud::setup_station,
                hud::setup_talk,
                hud::setup_prompt,
                hud::setup_hints,
                hud::setup_sky,
            )
                .after(terrain::roll),
        );
        // The ONE place raw input is read: gathered every frame, made into
        // actions first in every fixed step (authority.md rule 5); headless
        // tests push actions themselves.
        app.init_resource::<input::PendingInput>();
        app.add_systems(PreUpdate, input::gather_input.after(bevy::input::InputSystems));
        app.add_systems(FixedPreUpdate, input::input_to_actions);
        app.add_systems(Update, control_plane::stand_still.after(control_plane::apply_speed));
        // A stop for a test (a step, a load, a pause) is answered last in
        // the frame, after the HUD has drawn what the player sees.
        app.add_systems(Last, control_plane::answer_at_stop);
        app.add_systems(Startup, (loading::spawn_screen, hud::setup_ending));
        app.add_systems(Update, (loading::show_screen.after(loading::ready_the_world), hud::render_ending));
        app.add_systems(
            Update,
            (
                control_plane::apply_speed,
                hud::render,
                hud::render_item_panel,
                hud::render_trade,
                hud::render_death,
                hud::render_vitals,
                hud::render_station,
                hud::render_talk,
                hud::render_prompt,
                hud::render_hints,
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
                // The outline pass reads the depth and normal prepasses,
                // which it reads one sample a pixel: no MSAA.
                bevy::core_pipeline::prepass::DepthPrepass,
                bevy::core_pipeline::prepass::NormalPrepass,
                Msaa::Off,
                outline::Outlined,
                player::Eyes::default(),
                Transform::from_xyz(0.0, EYE_HEIGHT - player::BODY_CENTRE, 0.0)
                    .with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)),
            ));
        });
    }
}
