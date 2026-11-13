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
    crate::navmesh::mesh_the_world(world);
    crate::navmesh::run_landmass(world);
    world.resource_mut::<Loading>().on = false;
    // The start of the game, after the launch only: the game's own first
    // save, a replay of this run plays from it.
    if !LAUNCHED.swap(true, Ordering::Relaxed) {
        crate::save::own_save();
    }
    LOADING.store(false, Ordering::Relaxed);
    info!("the world is ready in {:.0} ms", start.elapsed().as_secs_f64() * 1000.0);
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
