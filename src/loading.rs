//! The loading screen (authority.md "Running tests": loading; todo 9j;
//! every game's loading screen, Bethesda's and Factorio's map generation
//! among them): after a launch and after loading a saved game, never for a
//! storm (whose new world builds over ticks), it shows until the world is
//! ready to play, and nobody acts while it shows. Ready means everything worked out from the world is
//! built: the chunks round the people, every chunk's mesh, and landmass's
//! archipelago holding them.

use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;

/// Whether the loading screen shows, for the `status` op: true from the
/// launch until the first world is ready.
pub static LOADING: AtomicBool = AtomicBool::new(true);

/// Whether the world launched with has been made ready once.
static LAUNCHED: AtomicBool = AtomicBool::new(false);

/// The loading screen: whether it shows, and whether a frame has drawn it
/// yet (the world is made ready on the frame after, so the screen is up
/// while that work holds the frame).
#[derive(Resource, Default)]
pub struct Loading {
    on: bool,
    drawn: bool,
}

impl Loading {
    /// It shows from now: the launch, or a saved game loaded.
    pub fn start() -> Self {
        LOADING.store(true, Ordering::Relaxed);
        Self { on: true, drawn: false }
    }
}

/// Nobody acts while the loading screen shows: Bevy's fixed loop, every
/// tick, runs only when it does not.
pub fn not_loading(loading: Option<Res<Loading>>) -> bool {
    !loading.is_some_and(|l| l.on)
}

/// The frame after the loading screen is drawn: the world made ready, once,
/// the same way after a launch and a load. The world systems the
/// fixed step runs every tick run once, as the first tick would (the chunks
/// round the people, their sites, the ground opened, the sites' people);
/// every chunk's mesh lands (`navmesh::mesh_the_world`); landmass takes them
/// in, its schedule run exactly once (the same every load, and nobody
/// moves, as no tick runs); then the screen goes.
pub fn ready_the_world(world: &mut World) {
    let Some(mut loading) = world.get_resource_mut::<Loading>() else {
        return;
    };
    if !loading.on {
        return;
    }
    if !loading.drawn {
        loading.drawn = true;
        return;
    }
    let start = std::time::Instant::now();
    let _ = world.run_system_cached(crate::terrain::make_chunks);
    let _ = world.run_system_cached(crate::structure::spawn_sites);
    let _ = world.run_system_cached(crate::terrain::open_the_ground);
    let _ = world.run_system_cached(crate::actor::populate_monuments);
    let _ = world.run_system_cached(crate::actor::populate_topside_people);
    let _ = world.run_system_cached(crate::interact::populate_town_water);
    crate::navmesh::mesh_the_world(world);
    crate::navmesh::run_landmass(world);
    // The start of the game, after the launch only: the world's history
    // runs, then the game's own first save, a replay of this run plays
    // from it.
    if !LAUNCHED.swap(true, Ordering::Relaxed) {
        run_history(world);
        crate::save::own_save();
    }
    world.resource_mut::<Loading>().on = false;
    LOADING.store(false, Ordering::Relaxed);
    info!("the world is ready in {:.0} ms", start.elapsed().as_secs_f64() * 1000.0);
}

/// While the world's history runs: the episode and the storm wait for the
/// player to wake.
#[derive(Resource)]
pub struct History;

/// Whether the player has woken: the world's history is over.
pub fn awake(history: Option<Res<History>>) -> bool {
    history.is_none()
}

/// The world's history (design.md "History, not decoration": "before the
/// player wakes, the world is run: the people live for some days in the
/// rolled world"; Dwarf Fortress's world history, run before play): the
/// game's own ticks, `HISTORY_SECS` of them, run as Bevy's fixed loop runs
/// them, behind the loading screen. The episode and its storm start when
/// the player wakes.
fn run_history(world: &mut World) {
    let start = std::time::Instant::now();
    let step = world.resource::<Time<Fixed>>().timestep();
    let ticks = (crate::content::HISTORY_SECS / step.as_secs_f32()).round() as u64;
    // The player has not woken: they wake with the needs they lay down
    // with (Claude's, for the operator to change).
    let mut player = world.query_filtered::<&crate::actor::Persona, With<crate::player::Player>>();
    let slept = player.iter(world).next().map(|p| p.0.needs);
    world.insert_resource(History);
    // Every tick, or days of thirst kill them before they wake.
    let mut player = world.query_filtered::<&mut crate::actor::Persona, With<crate::player::Player>>();
    let _ = world.try_schedule_scope(bevy::app::FixedMain, |world, schedule| {
        for _ in 0..ticks {
            world.resource_mut::<Time<Fixed>>().advance_by(step);
            *world.resource_mut::<Time>() = world.resource::<Time<Fixed>>().as_generic();
            schedule.run(world);
            if let Some(slept) = &slept {
                for mut person in player.iter_mut(world) {
                    person.0.needs = *slept;
                }
            }
        }
    });
    *world.resource_mut::<Time>() = world.resource::<Time<Virtual>>().as_generic();
    world.remove_resource::<History>();
    let now = world.resource::<crate::SimTick>().0;
    let mut episode = world.resource_mut::<crate::episode::Episode>();
    let name = episode.name.clone();
    episode.start(name, now);
    info!("the world's history ran {ticks} ticks in {:.0} ms", start.elapsed().as_secs_f64() * 1000.0);
}

/// The loading screen itself: the whole window, "Loading" in its middle.
#[derive(Component)]
pub struct LoadingScreen;

pub fn spawn_screen(mut commands: Commands) {
    commands
        .spawn((
            LoadingScreen,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::BLACK),
            GlobalZIndex(i32::MAX),
        ))
        .with_child(Text::new("Loading"));
}

/// Shown while loading, hidden once the world is ready.
pub fn show_screen(loading: Option<Res<Loading>>, mut screens: Query<&mut Visibility, With<LoadingScreen>>) {
    let on = loading.is_some_and(|l| l.on);
    for mut visibility in &mut screens {
        visibility.set_if_neq(if on { Visibility::Visible } else { Visibility::Hidden });
    }
}
