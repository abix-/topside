//! topside: a standalone post-apocalyptic survival game on Bevy,
//! consuming the modforge crate. Ownership map: docs/authority.md.

pub mod actor;
pub mod ai;
pub mod bunker;
pub mod cel;
pub mod combat;
pub mod content;
pub mod control_plane;
pub mod entity_map;
pub mod ground;
pub mod hud;
pub mod input;
pub mod interact;
pub mod player;
pub mod profile;
pub mod structure;
pub mod survival;

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
    app.add_plugins(avian3d::prelude::PhysicsPlugins::default());
    if headless {
        // Headless callers drive `update()` themselves and never
        // `run()`, so finish plugin setup here (Avian creates its
        // resources in `Plugin::finish`).
        app.finish();
        app.cleanup();
    }
    app.insert_resource(Time::<Fixed>::from_hz(60.0));
    app.init_state::<AppState>();
    app.init_resource::<SimTick>();
    app.init_resource::<entity_map::GpuSlotPool>();
    app.init_resource::<entity_map::EntityMap>();
    app.init_resource::<survival::Rates>();
    app.init_resource::<content::ItemRegistryRes>();
    app.init_resource::<content::DamageRegistryRes>();
    app.init_resource::<content::MonumentRegistryRes>();
    app.init_resource::<content::ActorRegistryRes>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<ButtonInput<MouseButton>>();
    app.init_resource::<input::BindingsRes>();
    app.init_resource::<input::JournalRes>();
    app.init_resource::<interact::LookingAt>();
    app.insert_resource(hud::HudRes(modforge::hud::HudState::new()));
    app.init_resource::<hud::HoveredSlot>();
    app.init_resource::<interact::OpenContainer>();
    // The ONE place raw input is read (input.rs): every frame, into
    // the player's action queue. Headless tests push actions
    // themselves and never touch keys.
    if !headless {
        app.add_systems(Update, input::read_input.run_if(in_state(AppState::Playing)));
    }
    // Every fixed step: drain each actor's due actions, then every
    // system reads actions, never keys.
    app.add_systems(
        FixedUpdate,
        (
            tick_system,
            actor::populate_monuments.run_if(actor::monuments_to_populate),
            ai::think,
            input::drain_actions,
            survival::survival_tick,
            structure::lock_gates,
            structure::rebake_structures.run_if(any_with_component::<structure::Rebake>),
            hud::toggle_inventory,
            hud::hotbar_keys,
            hud::transfer_hovered.run_if(hud::inventory_open),
            player::look,
            player::movement,
            combat::fire,
            interact::update_looking_at,
            interact::interact_on_press,
            control_plane::publish_system,
        )
            .chain()
            .run_if(in_state(AppState::Playing)),
    );
    if !headless {
        app.add_plugins(MaterialPlugin::<cel::CelMaterial>::default());
        app.add_plugins((
            bevy::diagnostic::FrameTimeDiagnosticsPlugin::default(),
            bevy::diagnostic::EntityCountDiagnosticsPlugin::default(),
            bevy::render::diagnostic::RenderDiagnosticsPlugin,
            profile::ProfilePlugin,
        ));
        control_plane::start();
        app.add_systems(
            OnEnter(AppState::Playing),
            (
                cel::load_shader,
                player::setup_scene,
                bunker::setup,
                bunker::setup_enemy,
                hud::setup,
            )
                .chain(),
        );
        app.add_systems(
            Update,
            (
                player::cursor_grab,
                hud::write_prompt,
                hud::render,
                hud::render_inventory,
            )
                .run_if(in_state(AppState::Playing)),
        );
        // Dropping spawns a mesh, so it stays with the window.
        app.add_systems(
            FixedUpdate,
            hud::drop_hovered
                .after(input::drain_actions)
                .run_if(in_state(AppState::Playing))
                .run_if(hud::inventory_open),
        );
    }
    app
}

fn tick_system(mut tick: ResMut<SimTick>) {
    tick.0 += 1;
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use modforge::hud::InteractKind;
    use modforge::structure::Aabb;

    struct GameplayOutcome {
        player_position: Vec3,
        door_opened: bool,
        selected_kind: Option<InteractKind>,
        inventory_count: u32,
    }

    fn run_fixed_gameplay_with_updates_per_tick(updates_per_tick: usize) -> GameplayOutcome {
        let mut app = build_app(true);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / (60.0 * updates_per_tick as f64),
        )));

        let player = app
            .world_mut()
            .spawn((
                Transform::from_xyz(0.0, 1.7, 2.0),
                player::Player,
            ))
            .id();
        let door = structure::Door {
            open: false,
            origin: Vec3::ZERO,
            closed_center: Vec3::new(0.0, 1.3, 1.0),
            hinge: Vec3::new(-0.5, 1.3, 1.0),
            closed_rotation: Quat::IDENTITY,
            collider: Aabb::from_center_size(Vec3::new(0.0, 1.3, 1.0), Vec3::new(1.0, 2.6, 0.1)),
        };
        let door_transform = door.transform();
        let door = app.world_mut().spawn((door, door_transform)).id();

        // Push actions the way the keyboard does: for the next fixed
        // tick, once per frame. Use is a tap; Move is held.
        let act = |app: &mut App, action: input::Action| {
            let next = app.world().resource::<SimTick>().0 + 1;
            app.world_mut()
                .get_mut::<input::Actions>(player)
                .unwrap()
                .0
                .push(next, action);
        };
        act(&mut app, input::Action::Use);
        for _ in 0..(7 * updates_per_tick) {
            act(&mut app, input::Action::Move { x: 0.0, y: 1.0 });
            app.update();
        }

        let player_position = app.world().get::<Transform>(player).unwrap().translation;
        let door_opened = app.world().get::<structure::Door>(door).unwrap().open;
        app.world_mut().despawn(door);

        let item_position = player_position + Vec3::NEG_Z;
        let scrap = modforge::item::create(
            app.world()
                .resource::<content::ItemRegistryRes>()
                .0
                .def("scrap")
                .unwrap(),
            1,
            &[],
            0.0,
            0,
        );
        app.world_mut().spawn((
            interact::Interactable {
                kind: InteractKind::Pickup,
                aabb: Aabb::from_center_size(item_position, Vec3::splat(0.4)),
            },
            interact::WorldItem { stack: scrap },
        ));
        act(&mut app, input::Action::Use);
        for _ in 0..updates_per_tick {
            app.update();
        }

        GameplayOutcome {
            player_position,
            door_opened,
            selected_kind: app.world().resource::<interact::LookingAt>().kind,
            inventory_count: app
                .world()
                .get::<actor::Inventory>(player)
                .expect("the player entity owns its inventory")
                .0
                .count_of("scrap"),
        }
    }

    #[test]
    fn gameplay_result_is_independent_of_update_count() {
        let one_update = run_fixed_gameplay_with_updates_per_tick(1);
        let four_updates = run_fixed_gameplay_with_updates_per_tick(4);

        assert!(
            one_update.player_position.z < 2.0,
            "held movement must advance the player during fixed ticks"
        );
        assert!(
            one_update.door_opened,
            "one interact press must open the door"
        );
        assert_eq!(one_update.selected_kind, Some(InteractKind::Pickup));
        assert_eq!(one_update.inventory_count, 1);
        assert!(
            one_update
                .player_position
                .abs_diff_eq(four_updates.player_position, 1e-6),
            "player position changed with Update count: {:?} vs {:?}",
            one_update.player_position,
            four_updates.player_position
        );
        assert_eq!(one_update.door_opened, four_updates.door_opened);
        assert_eq!(one_update.selected_kind, four_updates.selected_kind);
        assert_eq!(one_update.inventory_count, four_updates.inventory_count);
    }
}
