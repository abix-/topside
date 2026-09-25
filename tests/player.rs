//! The player walks through the one input path: actions pushed onto
//! its queue, drained each fixed tick, moved by `movement`.

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use topside::input::{Action, Actions};
use topside::player::{Player, WALK_SPEED};
use topside::{SimTick, build_app};

#[test]
fn pushed_move_actions_walk_the_player() {
    let mut app = build_app(true, 1);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 60.0)));
    // Startup rolls the world and spawns the player at the bunker.
    app.update();
    let player = app
        .world_mut()
        .query_filtered::<Entity, With<Player>>()
        .single(app.world())
        .expect("one player");
    let start = app.world().get::<Transform>(player).unwrap().translation;

    // Hold W for one second: one move pushed for each next fixed tick.
    let steps = 60;
    for _ in 0..steps {
        let next = app.world().resource::<SimTick>().0 + 1;
        app.world_mut()
            .get_mut::<Actions>(player)
            .unwrap()
            .0
            .push(next, Action::Move { x: 0.0, y: 1.0 });
        app.update();
    }
    let end = app.world().get::<Transform>(player).unwrap().translation;
    let walked = end.y - start.y;
    let ticks = app.world().resource::<SimTick>().0 as f32;
    assert!(walked > 0.0, "W moves the player up the screen");
    assert_eq!(end.x, start.x, "W does not move sideways");
    assert!(
        (walked - WALK_SPEED * ticks / 60.0).abs() < 0.2,
        "walked {walked} m in {ticks} ticks"
    );
}
