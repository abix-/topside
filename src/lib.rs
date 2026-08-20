//! topside: a standalone post-apocalyptic survival game on Bevy,
//! consuming the modforge crate. Ownership map: docs/authority.md.

pub mod bunker;
pub mod content;
pub mod control_plane;
pub mod entity_map;
pub mod hud;
pub mod interact;
pub mod player;
pub mod structure;

use bevy::prelude::*;

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Playing,
}

/// Simulation tick counter. FixedUpdate runs at a locked 60 UPS;
/// this counts those ticks, independent of render frame rate.
#[derive(Resource, Default)]
pub struct SimTick(pub u64);

/// Build the full app. `headless` skips window, rendering, input,
/// and the control plane so tests can run the simulation bare.
pub fn build_app(headless: bool) -> App {
    let mut app = App::new();
    if headless {
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::state::app::StatesPlugin);
    } else {
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "topside".to_string(),
                ..default()
            }),
            ..default()
        }));
    }
    app.insert_resource(Time::<Fixed>::from_hz(60.0));
    app.init_state::<AppState>();
    app.init_resource::<SimTick>();
    app.init_resource::<entity_map::GpuSlotPool>();
    app.init_resource::<entity_map::EntityMap>();
    app.init_resource::<bunker::FoodStore>();
    app.init_resource::<structure::StaticColliders>();
    app.init_resource::<structure::Walkables>();
    app.init_resource::<interact::LookingAt>();
    app.add_systems(
        FixedUpdate,
        (tick_system, bunker::food_tick_system, control_plane::publish_system)
            .chain()
            .run_if(in_state(AppState::Playing)),
    );
    if !headless {
        control_plane::start();
        app.add_systems(
            OnEnter(AppState::Playing),
            (player::setup_scene, bunker::setup, hud::setup),
        );
        app.add_systems(
            Update,
            (
                player::cursor_grab,
                player::mouse_look,
                player::movement,
                interact::update_looking_at,
                interact::interact_on_press,
                hud::update,
            )
                .run_if(in_state(AppState::Playing)),
        );
    }
    app
}

fn tick_system(mut tick: ResMut<SimTick>) {
    tick.0 += 1;
}
