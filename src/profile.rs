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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Instant;

use bevy::log::tracing::{self, Subscriber, field::Field, span};
use bevy::log::tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};
use bevy::prelude::*;

/// One thing spans measure ("system topside::ai::think", "schedule
/// Update") and its time so far this frame, in ns: added to without a lock
/// by every span of it that ends, on any thread (a counter a name, as
/// metrics crates keep).
struct Slot {
    name: String,
    nanos: AtomicU64,
}

/// Every slot, by name: locked only when a thread meets a name it has not
/// met before, and once a frame to roll them.
static SLOTS: LazyLock<Mutex<HashMap<String, Arc<Slot>>>> = LazyLock::new(Default::default);
/// Every span's average time per frame, in ms.
static PER_FRAME: LazyLock<Mutex<HashMap<String, f64>>> = LazyLock::new(Default::default);

/// What a span measures: its kind and, for a system or schedule, its name;
/// for Bevy's `par_for_each` (every `Query::par_iter`), the query it walks,
/// so each parallel query is its own line, not one sum of all of them.
struct SpanName(Arc<Slot>);

std::thread_local! {
    /// When each span was entered on this thread, by span id: one span (a
    /// `par_iter`'s) is entered on several worker threads at once, and each
    /// entry is timed from its own start.
    static ENTERED: std::cell::RefCell<HashMap<u64, Instant>> = std::cell::RefCell::new(HashMap::new());
    /// The slots this thread has met, by name, and the name being written:
    /// a span made again (every system, every frame) finds its slot with
    /// no lock and nothing allocated.
    static MET: std::cell::RefCell<(HashMap<String, Arc<Slot>>, String)> = std::cell::RefCell::new((HashMap::new(), String::new()));
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
                use std::fmt::Write;
                if matches!(field.name(), "name" | "query" | "data") {
                    self.0.push(' ');
                    // Written in place, then the quotes a string's Debug
                    // adds taken off.
                    let from = self.0.len();
                    let _ = write!(self.0, "{value:?}");
                    let lead = self.0[from..].len() - self.0[from..].trim_start_matches('"').len();
                    self.0.replace_range(from..from + lead, "");
                    let tail = self.0[from..].len() - self.0[from..].trim_end_matches('"').len();
                    self.0.truncate(self.0.len() - tail);
                }
            }
        }
        let Some(span) = ctx.span(id) else {
            return;
        };
        let slot = MET.with(|met| {
            let (met, name) = &mut *met.borrow_mut();
            name.clear();
            name.push_str(attrs.metadata().name());
            attrs.record(&mut Name(name));
            if let Some(slot) = met.get(name.as_str()) {
                return Arc::clone(slot);
            }
            let mut slots = SLOTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let slot = Arc::clone(slots.entry(name.clone()).or_insert_with(|| Arc::new(Slot { name: name.clone(), nanos: AtomicU64::new(0) })));
            met.insert(name.clone(), Arc::clone(&slot));
            slot
        });
        span.extensions_mut().insert(SpanName(slot));
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
        if let Some(name) = span.extensions().get::<SpanName>() {
            name.0.nanos.fetch_add(u64::try_from(entered.elapsed().as_nanos()).unwrap_or(u64::MAX), Ordering::Relaxed);
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
    let (Ok(slots), Ok(mut means)) = (SLOTS.lock(), PER_FRAME.lock()) else {
        return;
    };
    for mean in means.values_mut() {
        *mean -= *mean * SMOOTHING;
    }
    for slot in slots.values() {
        let nanos = slot.nanos.swap(0, Ordering::Relaxed);
        if nanos == 0 {
            continue;
        }
        let ms = nanos as f64 / 1_000_000.0;
        match means.get_mut(slot.name.as_str()) {
            Some(mean) => *mean += ms * SMOOTHING,
            None => {
                means.insert(slot.name.clone(), ms);
            }
        }
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

/// Every measured part's average cost per fixed step, in ms, and its worst
/// single step since launch (an average hides a one-tick stall: 474 ms
/// reads as about 16 and fades); what the parts counted (people walking,
/// ways searched); the whole step, from the start of FixedFirst to the end
/// of FixedLast, average and worst; and how many fixed steps a frame ran.
/// One step over the budget: its tick, its ms, its costliest parts, and
/// the units each costly system worked in it.
#[derive(Clone, serde::Serialize)]
pub struct OverBudget {
    pub tick: u64,
    pub ms: f64,
    pub parts: Vec<(&'static str, f64)>,
    pub units: Vec<(&'static str, f64)>,
}

#[derive(Resource, Default)]
pub struct Laps {
    mark: Option<Instant>,
    step_start: Option<Instant>,
    pub mean_ms: Vec<(&'static str, f64)>,
    /// Each part's worst single step since launch, in ms.
    pub worst_ms: Vec<(&'static str, f64)>,
    /// The parts that are laps of the fixed step, one after another from
    /// its start: these add up to the whole step. The rest are pieces
    /// inside a lap ("steer: ...") or work outside the step.
    pub in_step: Vec<&'static str>,
    /// Average counts per fixed step, by name.
    pub counts: Vec<(&'static str, f64)>,
    /// The whole fixed step, every Fixed schedule, in ms.
    pub step_ms: f64,
    /// The whole fixed step's worst since the world was made ready, in ms.
    pub step_worst_ms: f64,
    /// The tick that worst step ran.
    pub step_worst_tick: u64,
    /// The steps over the budget (1/60 s) since the world was made ready:
    /// how many, and the first `OVER_KEPT` of them, each its tick, ms, its
    /// costliest parts, and its units.
    pub over_budget: u64,
    pub over_budget_steps: Vec<OverBudget>,
    /// Every part recorded this step so far, in ms.
    this_step: Vec<(&'static str, f64)>,
    /// Each count's last value, in `counts`' order.
    counts_now: Vec<f64>,
    /// Fixed steps run this frame, so far.
    steps_this_frame: u32,
    /// Fixed steps a frame, averaged over frames.
    pub steps_per_frame: f64,
    /// Each part's place in `mean_ms` and `worst_ms` (made together), and
    /// whether it is a lap of the step (in `in_step`).
    part_of: HashMap<&'static str, (usize, bool)>,
    /// Each count's place in `counts`.
    count_of: HashMap<&'static str, usize>,
}

impl Laps {
    /// A part's time this step, in ms: a whole system (`lap`) or a part
    /// inside one, timed where it runs; its worst kept too.
    pub fn record(&mut self, name: &'static str, ms: f64) {
        self.this_step.push((name, ms));
        match self.part_of.get(name) {
            Some(&(i, _)) => {
                let mean = &mut self.mean_ms[i].1;
                *mean += (ms - *mean) * SMOOTHING;
                let worst = &mut self.worst_ms[i].1;
                *worst = worst.max(ms);
            }
            None => {
                self.part_of.insert(name, (self.mean_ms.len(), false));
                self.mean_ms.push((name, ms));
                self.worst_ms.push((name, ms));
            }
        }
    }

    /// The worsts start again: the world was just made ready (a launch or a
    /// load), so the steps after it are play, not loading.
    pub fn restart_worsts(&mut self) {
        self.step_worst_ms = 0.0;
        self.over_budget = 0;
        self.over_budget_steps.clear();
        for (_, worst) in &mut self.worst_ms {
            *worst = 0.0;
        }
    }

    /// One part's worst single step since the world was made ready, if
    /// measured.
    pub fn worst(&self, name: &str) -> Option<f64> {
        self.part_of.get(name).map(|&(i, _)| self.worst_ms[i].1)
    }

    /// How many of something happened this step (ways searched, people
    /// walking), averaged like the times.
    pub fn count(&mut self, name: &'static str, n: usize) {
        let n = n as f64;
        match self.count_of.get(name) {
            Some(&i) => {
                let mean = &mut self.counts[i].1;
                *mean += (n - *mean) * SMOOTHING;
                self.counts_now[i] = n;
            }
            None => {
                self.count_of.insert(name, self.counts.len());
                self.counts.push((name, n));
                self.counts_now.push(n);
            }
        }
    }

    /// Every part, the costliest first.
    pub fn sorted(&self) -> Vec<(&'static str, f64)> {
        let mut all = self.mean_ms.clone();
        all.sort_by(|a, b| b.1.total_cmp(&a.1));
        all
    }

    /// One part's average, if measured.
    pub fn of(&self, name: &str) -> Option<f64> {
        self.part_of.get(name).map(|&(i, _)| self.mean_ms[i].1)
    }
}

fn since(from: Option<Instant>, now: Instant) -> f64 {
    from.map_or(0.0, |f| now.duration_since(f).as_secs_f64() * 1000.0)
}

/// The stopwatch starts (the frame's own parts, outside the fixed step).
pub fn start(mut laps: ResMut<Laps>) {
    laps.mark = Some(Instant::now());
}

/// The top of the fixed step, first in FixedFirst: the stopwatch and the
/// whole step start, and the step is counted for this frame.
pub fn start_step(mut laps: ResMut<Laps>) {
    let now = Instant::now();
    laps.mark = Some(now);
    laps.step_start = Some(now);
    laps.steps_this_frame += 1;
    laps.this_step.clear();
}

/// The end of the fixed step, last in FixedLast: the whole step's time.
pub fn finish_step(mut laps: ResMut<Laps>, tick: Res<crate::SimTick>) {
    let ms = since(laps.step_start, Instant::now());
    laps.step_ms += (ms - laps.step_ms) * SMOOTHING;
    if ms > laps.step_worst_ms {
        laps.step_worst_ms = ms;
        laps.step_worst_tick = tick.0;
    }
    if ms > BUDGET_MS {
        laps.over_budget += 1;
        if laps.over_budget_steps.len() < OVER_KEPT {
            let mut parts = laps.this_step.clone();
            parts.sort_by(|a, b| b.1.total_cmp(&a.1));
            parts.truncate(3);
            let units = laps.counts.iter().zip(&laps.counts_now).map(|((name, _), n)| (*name, *n)).collect();
            laps.over_budget_steps.push(OverBudget { tick: tick.0, ms, parts, units });
        }
    }
}

/// A step's budget: one tick of the locked 60 UPS, in ms.
pub const BUDGET_MS: f64 = 1000.0 / 60.0;

/// How many steps over the budget are kept with their tick.
const OVER_KEPT: usize = 20;

/// A system that records the time since the last stamp under `name`
/// and stamps again: put one after each part of the fixed step to time
/// it. The laps follow one another from `start_step`, so they add up to
/// the whole step.
pub fn lap(name: &'static str) -> impl FnMut(ResMut<Laps>) {
    move |mut laps: ResMut<Laps>| {
        let now = Instant::now();
        let ms = since(laps.mark, now);
        laps.record(name, ms);
        let laps = &mut *laps;
        if let Some((_, lap)) = laps.part_of.get_mut(name)
            && !*lap
        {
            *lap = true;
            laps.in_step.push(name);
        }
        laps.mark = Some(now);
    }
}

/// Once a frame (First): how many fixed steps the last frame ran, into
/// its average.
fn count_steps(mut laps: ResMut<Laps>) {
    let steps = std::mem::take(&mut laps.steps_this_frame) as f64;
    laps.steps_per_frame += (steps - laps.steps_per_frame) * SMOOTHING;
}

/// The units each costly system works a step, so a part's ms over its
/// units is its cost per unit (todo Perf 4; Factorio's show-time-usage, each
/// entity type's time beside its count). Counted at the step's start:
/// those whose moment to think this step's tick is (the tick is raised
/// first thing in FixedUpdate).
fn count_thinkers(mut laps: ResMut<Laps>, tick: Res<crate::SimTick>, npcs: Query<&crate::actor::NextThink, (With<crate::actor::Npc>, Without<crate::actor::Dead>)>) {
    let next = tick.0 + 1;
    laps.count("units: thinking", npcs.iter().filter(|t| next >= t.0).count());
}

/// The rest of the units, counted once FixedUpdate has run, before
/// physics: the living people, those moving (a move or a fall), those
/// using something, landmass's agents (walkers) and characters
/// (standing), the things in the world, and every body physics steps.
#[allow(clippy::type_complexity)]
fn count_units(
    mut laps: ResMut<Laps>,
    people: Query<(&crate::input::ThisTick, &crate::player::Fall), Without<crate::actor::Dead>>,
    agents: Query<(), With<bevy_landmass::Agent3d>>,
    characters: Query<(), With<bevy_landmass::Character<bevy_landmass::coords::ThreeD>>>,
    things: Query<(), With<crate::interact::WorldItem>>,
    bodies: Query<(), With<avian3d::prelude::RigidBody>>,
) {
    use crate::input::Action;
    let (mut moving, mut using) = (0, 0);
    for (this_tick, fall) in &people {
        let (x, y) = modforge::actions::ActionQueue::movement(&this_tick.0);
        moving += usize::from(x != 0.0 || y != 0.0 || fall.0 != 0.0);
        using += usize::from(this_tick.0.iter().any(|s| matches!(s.action, Action::Use | Action::Fill { .. })));
    }
    laps.count("units: people", people.iter().count());
    laps.count("units: moving", moving);
    laps.count("units: using", using);
    laps.count("units: landmass agents", agents.iter().count());
    laps.count("units: landmass characters", characters.iter().count());
    laps.count("units: things", things.iter().count());
    laps.count("units: physics bodies", bodies.iter().count());
}

/// The stamps outside topside's FixedUpdate chain: the step starts first
/// in FixedFirst (its land meshes lapped in navmesh.rs, landmass in
/// FixedPreUpdate the same), what of FixedUpdate ran after the chain's
/// last lap is lapped as Avian's step begins, Avian's step is lapped in
/// FixedLast, and the step ends.
pub struct ProfilePlugin;

/// The part for whatever of FixedUpdate ran after the chain's last lap
/// (systems outside the chain, ordered only after some part of it).
const REST_OF_FIXED_UPDATE: &str = "rest of fixed update";

impl Plugin for ProfilePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Laps>();
        app.add_systems(First, (roll_frame, count_steps));
        app.add_systems(FixedFirst, (start_step, count_thinkers).chain());
        app.add_systems(
            FixedPostUpdate,
            (count_units, lap(REST_OF_FIXED_UPDATE)).chain().before(avian3d::schedule::PhysicsSystems::First),
        );
        app.add_systems(FixedLast, (lap("physics"), finish_step, physics_pieces).chain());
    }
}

/// The physics lap's pieces, from the timers Avian keeps itself (its
/// diagnostics, filled whether or not its diagnostics plugin is added):
/// read once a step, then started again, as Avian's own reset does.
fn physics_pieces(
    mut laps: ResMut<Laps>,
    collision: Option<ResMut<avian3d::collision::CollisionDiagnostics>>,
    tree: Option<ResMut<avian3d::collider_tree::ColliderTreeDiagnostics>>,
    solver: Option<ResMut<avian3d::dynamics::solver::SolverDiagnostics>>,
) {
    let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
    if let Some(mut c) = collision {
        laps.record("physics: broad phase", ms(c.broad_phase));
        laps.record("physics: narrow phase", ms(c.narrow_phase));
        *c = default();
    }
    if let Some(mut t) = tree {
        laps.record("physics: collider tree update", ms(t.update));
        laps.record("physics: collider tree optimize", ms(t.optimize));
        *t = default();
    }
    if let Some(mut s) = solver {
        laps.record("physics: solver prepare", ms(s.prepare_constraints + s.update_velocity_increments));
        laps.record("physics: solver integrate", ms(s.integrate_velocities + s.integrate_positions));
        laps.record("physics: solver solve", ms(s.warm_start + s.solve_constraints + s.relax_velocities + s.apply_restitution));
        laps.record("physics: solver finalize", ms(s.finalize + s.store_impulses));
        laps.record("physics: solver swept ccd", ms(s.swept_ccd));
        *s = default();
    }
}
