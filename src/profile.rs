//! Where the main-world frame goes, measured, not assumed. Marker
//! systems stamp the clock at known points of each frame; the spans
//! between them land in the diagnostics store next to Bevy's own
//! render timings, so the `diagnostics` op reports both:
//!
//! - `main/first_to_transform`: First through Update and the fixed
//!   loop, until transform propagation starts (gameplay).
//! - `main/transform_propagate`: TransformSystems::Propagate.
//! - `main/visibility`: visibility propagation and frustum culling.
//! - `main/post_to_last`: the rest of PostUpdate and Last.
//! - `main/extract_and_wait`: from Last to the next frame's First:
//!   render-world extraction plus waiting on the GPU.

use std::time::Instant;

use bevy::camera::visibility::VisibilitySystems;
use bevy::diagnostic::{Diagnostic, DiagnosticPath, Diagnostics, RegisterDiagnostic};
use bevy::prelude::*;
use bevy::transform::TransformSystems;

pub const FIRST_TO_TRANSFORM: DiagnosticPath = DiagnosticPath::const_new("main/first_to_transform");
/// The fixed loop alone (every fixed step this frame, physics
/// included) and how many steps it ran.
pub const FIXED_LOOP: DiagnosticPath = DiagnosticPath::const_new("main/fixed_loop");
pub const FIXED_STEPS: DiagnosticPath = DiagnosticPath::const_new("main/fixed_steps_per_frame");
/// Inside one fixed step, summed over the frame: topside's own
/// FixedUpdate chain, and FixedPostUpdate (Avian's physics step).
pub const FIXED_GAMEPLAY: DiagnosticPath = DiagnosticPath::const_new("main/fixed_gameplay");
pub const FIXED_PHYSICS: DiagnosticPath = DiagnosticPath::const_new("main/fixed_physics");
pub const TRANSFORM_PROPAGATE: DiagnosticPath = DiagnosticPath::const_new("main/transform_propagate");
pub const VISIBILITY: DiagnosticPath = DiagnosticPath::const_new("main/visibility");
pub const POST_TO_LAST: DiagnosticPath = DiagnosticPath::const_new("main/post_to_last");
pub const EXTRACT_AND_WAIT: DiagnosticPath = DiagnosticPath::const_new("main/extract_and_wait");

#[derive(Resource, Default)]
struct Marks {
    first: Option<Instant>,
    fixed_start: Option<Instant>,
    fixed_steps: u32,
    step_start: Option<Instant>,
    step_gameplay_end: Option<Instant>,
    gameplay_ms: f64,
    physics_ms: f64,
    transform_start: Option<Instant>,
    transform_end: Option<Instant>,
    visibility_end: Option<Instant>,
    last: Option<Instant>,
}

pub struct ProfilePlugin;

impl Plugin for ProfilePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Marks>();
        for path in [
            FIRST_TO_TRANSFORM,
            FIXED_LOOP,
            TRANSFORM_PROPAGATE,
            VISIBILITY,
            POST_TO_LAST,
            EXTRACT_AND_WAIT,
        ] {
            app.register_diagnostic(Diagnostic::new(path).with_suffix("ms"));
        }
        app.register_diagnostic(Diagnostic::new(FIXED_STEPS));
        app.register_diagnostic(Diagnostic::new(FIXED_GAMEPLAY).with_suffix("ms"));
        app.register_diagnostic(Diagnostic::new(FIXED_PHYSICS).with_suffix("ms"));
        app.add_systems(First, mark_first);
        app.add_systems(
            RunFixedMainLoop,
            (
                mark_fixed_start.in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
                mark_fixed_end.in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
            ),
        );
        // FixedFirst to the end of FixedUpdate is topside's chain;
        // FixedPostUpdate through FixedLast is Avian's step.
        app.add_systems(FixedFirst, mark_step_start);
        app.add_systems(FixedPostUpdate, mark_gameplay_end.before(avian3d::schedule::PhysicsSystems::Prepare));
        app.add_systems(FixedLast, mark_step_end);
        app.add_systems(
            PostUpdate,
            (
                mark_transform_start.before(TransformSystems::Propagate),
                mark_transform_end
                    .after(TransformSystems::Propagate)
                    .before(VisibilitySystems::VisibilityPropagate),
                mark_visibility_end.after(VisibilitySystems::CheckVisibility),
            ),
        );
        app.add_systems(Last, mark_last);
    }
}

fn ms(from: Option<Instant>, to: Instant) -> Option<f64> {
    from.map(|f| to.duration_since(f).as_secs_f64() * 1000.0)
}

fn mark_first(mut marks: ResMut<Marks>, mut diagnostics: Diagnostics) {
    let now = Instant::now();
    if let Some(v) = ms(marks.last, now) {
        diagnostics.add_measurement(&EXTRACT_AND_WAIT, || v);
    }
    marks.first = Some(now);
}

fn mark_fixed_start(mut marks: ResMut<Marks>) {
    marks.fixed_start = Some(Instant::now());
    marks.fixed_steps = 0;
    marks.gameplay_ms = 0.0;
    marks.physics_ms = 0.0;
}

fn mark_step_start(mut marks: ResMut<Marks>) {
    marks.fixed_steps += 1;
    marks.step_start = Some(Instant::now());
}

fn mark_gameplay_end(mut marks: ResMut<Marks>) {
    let now = Instant::now();
    if let Some(v) = ms(marks.step_start, now) {
        marks.gameplay_ms += v;
    }
    marks.step_gameplay_end = Some(now);
}

fn mark_step_end(mut marks: ResMut<Marks>) {
    let now = Instant::now();
    if let Some(v) = ms(marks.step_gameplay_end, now) {
        marks.physics_ms += v;
    }
}

fn mark_fixed_end(marks: Res<Marks>, mut diagnostics: Diagnostics) {
    let now = Instant::now();
    if let Some(v) = ms(marks.fixed_start, now) {
        diagnostics.add_measurement(&FIXED_LOOP, || v);
    }
    let steps = marks.fixed_steps as f64;
    diagnostics.add_measurement(&FIXED_STEPS, || steps);
    let (gameplay, physics) = (marks.gameplay_ms, marks.physics_ms);
    diagnostics.add_measurement(&FIXED_GAMEPLAY, || gameplay);
    diagnostics.add_measurement(&FIXED_PHYSICS, || physics);
}

fn mark_transform_start(mut marks: ResMut<Marks>, mut diagnostics: Diagnostics) {
    let now = Instant::now();
    if let Some(v) = ms(marks.first, now) {
        diagnostics.add_measurement(&FIRST_TO_TRANSFORM, || v);
    }
    marks.transform_start = Some(now);
}

fn mark_transform_end(mut marks: ResMut<Marks>, mut diagnostics: Diagnostics) {
    let now = Instant::now();
    if let Some(v) = ms(marks.transform_start, now) {
        diagnostics.add_measurement(&TRANSFORM_PROPAGATE, || v);
    }
    marks.transform_end = Some(now);
}

fn mark_visibility_end(mut marks: ResMut<Marks>, mut diagnostics: Diagnostics) {
    let now = Instant::now();
    if let Some(v) = ms(marks.transform_end, now) {
        diagnostics.add_measurement(&VISIBILITY, || v);
    }
    marks.visibility_end = Some(now);
}

fn mark_last(mut marks: ResMut<Marks>, mut diagnostics: Diagnostics) {
    let now = Instant::now();
    if let Some(v) = ms(marks.visibility_end, now) {
        diagnostics.add_measurement(&POST_TO_LAST, || v);
    }
    marks.last = Some(now);
}
