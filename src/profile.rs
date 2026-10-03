//! Where the time goes, measured, not assumed: a stopwatch stamped
//! after every system of the fixed step (and around Avian's physics
//! step), each cost kept as a running average in milliseconds; and every
//! span Bevy runs (each system, each schedule, the drawing's work; its
//! `trace` feature), added up per frame by a tracing layer (`layer`), so
//! the whole frame is measured, drawing included. The control plane's
//! `perf` op and the headless perf test read it. Prior art: tracing-timing,
//! and Bevy's own Chrome and Tracy layers, which send the same spans
//! elsewhere.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

use bevy::log::tracing::{self, Subscriber, field::Field, span};
use bevy::log::tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};
use bevy::prelude::*;

/// Every span's time so far this frame, in ms, by what it measures
/// ("system topside::ai::think", "schedule Update").
static THIS_FRAME: LazyLock<Mutex<HashMap<String, f64>>> = LazyLock::new(Default::default);
/// Every span's average time per frame, in ms.
static PER_FRAME: LazyLock<Mutex<HashMap<String, f64>>> = LazyLock::new(Default::default);

/// What a span measures: its kind and, for a system or schedule, its name;
/// for Bevy's `par_for_each` (every `Query::par_iter`), the query it walks,
/// so each parallel query is its own line, not one sum of all of them.
struct SpanName(String);

std::thread_local! {
    /// When each span was entered on this thread, by span id: one span (a
    /// `par_iter`'s) is entered on several worker threads at once, and each
    /// entry is timed from its own start.
    static ENTERED: std::cell::RefCell<HashMap<u64, Instant>> = std::cell::RefCell::new(HashMap::new());
}

/// The tracing layer that times every span: plugged into Bevy's log
/// (`LogPlugin::custom_layer`).
pub struct SpanTimer;

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for SpanTimer {
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        struct Name<'a>(&'a mut String);
        impl tracing::field::Visit for Name<'_> {
            fn record_str(&mut self, field: &Field, value: &str) {
                if matches!(field.name(), "name" | "query" | "data") {
                    self.0.push(' ');
                    self.0.push_str(value);
                }
            }
            fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
                if matches!(field.name(), "name" | "query" | "data") {
                    self.0.push(' ');
                    self.0.push_str(format!("{value:?}").trim_matches('"'));
                }
            }
        }
        let mut name = attrs.metadata().name().to_string();
        attrs.record(&mut Name(&mut name));
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(SpanName(name));
        }
    }

    fn on_enter(&self, id: &span::Id, _ctx: Context<'_, S>) {
        ENTERED.with(|e| e.borrow_mut().insert(id.into_u64(), Instant::now()));
    }

    fn on_exit(&self, id: &span::Id, ctx: Context<'_, S>) {
        let Some(entered) = ENTERED.with(|e| e.borrow_mut().remove(&id.into_u64())) else {
            return;
        };
        let Some(span) = ctx.span(id) else {
            return;
        };
        if let Some(name) = span.extensions().get::<SpanName>()
            && let Ok(mut frame) = THIS_FRAME.lock()
        {
            *frame.entry(name.0.clone()).or_insert(0.0) += entered.elapsed().as_secs_f64() * 1000.0;
        }
    }
}

/// The span timer, for `LogPlugin::custom_layer`.
pub fn layer(_: &mut App) -> Option<bevy::log::BoxedLayer> {
    Some(Box::new(SpanTimer))
}

/// Once a frame (First): this frame's span times go into the averages
/// per frame, and the next frame starts from nothing; a span that did not
/// run this frame counts as nothing.
pub fn roll_frame() {
    let (Ok(mut frame), Ok(mut means)) = (THIS_FRAME.lock(), PER_FRAME.lock()) else {
        return;
    };
    for mean in means.values_mut() {
        *mean -= *mean * SMOOTHING;
    }
    for (name, ms) in frame.drain() {
        means.entry(name).and_modify(|mean| *mean += ms * SMOOTHING).or_insert(ms);
    }
}

/// Every span's average ms per frame, the costliest first.
pub fn per_frame() -> Vec<(String, f64)> {
    let mut all: Vec<(String, f64)> = PER_FRAME.lock().map(|m| m.iter().map(|(n, v)| (n.clone(), *v)).collect()).unwrap_or_default();
    all.sort_by(|a, b| b.1.total_cmp(&a.1));
    all
}

/// How much of each new measurement moves the average (the rest is
/// history): about the last 30 samples.
const SMOOTHING: f64 = 1.0 / 30.0;

/// Every measured part's average cost per fixed step, in ms, what the
/// parts counted (people walking, ways searched), and the whole step.
#[derive(Resource, Default)]
pub struct Laps {
    mark: Option<Instant>,
    step_start: Option<Instant>,
    physics_start: Option<Instant>,
    pub mean_ms: Vec<(&'static str, f64)>,
    /// Average counts per fixed step, by name.
    pub counts: Vec<(&'static str, f64)>,
    /// The whole fixed step, physics included, in ms.
    pub step_ms: f64,
}

fn smooth(list: &mut Vec<(&'static str, f64)>, name: &'static str, value: f64) {
    match list.iter_mut().find(|(n, _)| *n == name) {
        Some((_, mean)) => *mean += (value - *mean) * SMOOTHING,
        None => list.push((name, value)),
    }
}

impl Laps {
    /// A part's time this step, in ms: a whole system (`lap`) or a part
    /// inside one, timed where it runs.
    pub fn record(&mut self, name: &'static str, ms: f64) {
        smooth(&mut self.mean_ms, name, ms);
    }

    /// How many of something happened this step (ways searched, people
    /// walking), averaged like the times.
    pub fn count(&mut self, name: &'static str, n: usize) {
        smooth(&mut self.counts, name, n as f64);
    }

    /// Every part, the costliest first.
    pub fn sorted(&self) -> Vec<(&'static str, f64)> {
        let mut all = self.mean_ms.clone();
        all.sort_by(|a, b| b.1.total_cmp(&a.1));
        all
    }

    /// One part's average, if measured.
    pub fn of(&self, name: &str) -> Option<f64> {
        self.mean_ms.iter().find(|(n, _)| *n == name).map(|(_, v)| *v)
    }
}

fn since(from: Option<Instant>, now: Instant) -> f64 {
    from.map_or(0.0, |f| now.duration_since(f).as_secs_f64() * 1000.0)
}

/// The stopwatch starts (the frame's own parts, outside the fixed step).
pub fn start(mut laps: ResMut<Laps>) {
    laps.mark = Some(Instant::now());
}

/// The top of the fixed step: the stopwatch and the whole step start.
pub fn start_step(mut laps: ResMut<Laps>) {
    let now = Instant::now();
    laps.mark = Some(now);
    laps.step_start = Some(now);
}

/// The end of the fixed step, after physics: the whole step's time.
pub fn finish_step(mut laps: ResMut<Laps>) {
    let ms = since(laps.step_start, Instant::now());
    laps.step_ms += (ms - laps.step_ms) * SMOOTHING;
}

/// A system that records the time since the last stamp under `name`
/// and stamps again: put one after each system to time it.
pub fn lap(name: &'static str) -> impl FnMut(ResMut<Laps>) {
    move |mut laps: ResMut<Laps>| {
        let now = Instant::now();
        let ms = since(laps.mark, now);
        laps.record(name, ms);
        laps.mark = Some(now);
    }
}

/// Around Avian's step: FixedPostUpdate to FixedLast.
pub fn physics_start(mut laps: ResMut<Laps>) {
    laps.physics_start = Some(Instant::now());
}

pub fn physics_end(mut laps: ResMut<Laps>) {
    let ms = since(laps.physics_start, Instant::now());
    laps.record("physics", ms);
}

/// Stamps around the physics step, the one piece outside topside's
/// chain.
pub struct ProfilePlugin;

impl Plugin for ProfilePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Laps>();
        app.add_systems(First, roll_frame);
        app.add_systems(
            FixedPostUpdate,
            physics_start.before(avian3d::schedule::PhysicsSystems::First),
        );
        app.add_systems(FixedLast, (physics_end, finish_step).chain());
    }
}
