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
    crate::make_the_world(world);
    crate::navmesh::mesh_the_world(world);
    crate::navmesh::run_landmass(world);
    rebuild_collider_trees(world);
    // The start of the game, after the launch only: the world's history
    // runs, then the game's own first save, a replay of this run plays
    // from it.
    if !LAUNCHED.swap(true, Ordering::Relaxed) {
        run_history(world);
        crate::save::own_save();
    }
    world.resource_mut::<Loading>().on = false;
    LOADING.store(false, Ordering::Relaxed);
    if let Some(mut laps) = world.get_resource_mut::<crate::profile::Laps>() {
        laps.restart_worsts();
    }
    info!("the world is ready in {:.0} ms", start.elapsed().as_secs_f64() * 1000.0);
}

/// Avian's collider trees made ready now, behind the loading screen: every
/// collider the world just made has its box worked out by Avian's own
/// update (`update_moved_collider_aabbs`, the first thing its step does;
/// 43 ms in the bench's first step), so the first step finds none of them
/// moved; each is still a moved proxy for the broad phase, which pairs them
/// in that step as it would have. Then each tree with most of it new is
/// rebuilt whole (77 ms in the bench's first step), the same choice Avian
/// makes (`TreeOptimizationMode::resolve`), made here instead of in a step.
/// The time this takes is the loading's, not a step's.
fn rebuild_collider_trees(world: &mut World) {
    use avian3d::collider_tree::{ColliderTreeDiagnostics, ColliderTreeOptimization, ColliderTrees, TreeOptimizationMode, update_moved_collider_aabbs};
    let _ = world.run_system_cached(update_moved_collider_aabbs::<avian3d::prelude::Collider>);
    if let Some(mut diagnostics) = world.get_resource_mut::<ColliderTreeDiagnostics>() {
        *diagnostics = default();
    }
    let Some(mode) = world.get_resource::<ColliderTreeOptimization>().map(|o| o.optimization_mode) else {
        return;
    };
    let Some(mut trees) = world.get_resource_mut::<ColliderTrees>() else {
        return;
    };
    for tree in trees.iter_trees_mut() {
        if tree.moved_proxies.is_empty() {
            continue;
        }
        let moved = tree.moved_proxies.len() as f32 / tree.proxies.len() as f32;
        if mode.resolve(moved) == TreeOptimizationMode::FullRebuild {
            tree.rebuild_full();
            tree.moved_proxies.clear();
        }
    }
}

/// While the world's history runs: the episode and the storm wait for the
/// player to wake.
#[derive(Resource)]
pub struct History;

/// Whether the player has woken: the world's history is over.
pub fn awake(history: Option<Res<History>>) -> bool {
    history.is_none()
}

/// The world's history (todo History 1; operator, 2026-11-23: "a history of
/// like 1 week beforehand... a calculated history. We don't actually run
/// the game. You just roll the events exactly, and then the events become
/// history"; Dwarf Fortress's world history): the week is rolled as events
/// by modforge `history::roll`, never played, and laid down by
/// `lay_down_history`; the clock then stands a week in. The episode and its
/// storm start when the player wakes.
fn run_history(world: &mut World) {
    let start = std::time::Instant::now();
    let events = world.run_system_cached(lay_down_history).unwrap_or(0);
    let step = world.resource::<Time<Fixed>>().timestep();
    let ticks = (crate::content::HISTORY_SECS / step.as_secs_f32()).round() as u64;
    world.resource_mut::<crate::SimTick>().0 += ticks;
    let now = world.resource::<crate::SimTick>().0;
    let mut episode = world.resource_mut::<crate::episode::Episode>();
    let name = episode.name.clone();
    episode.start(name, now);
    info!("the world's history: {events} events over {} days laid down in {:.0} ms", crate::content::HISTORY_DAYS, start.elapsed().as_secs_f64() * 1000.0);
}

/// Everyone living outside a bunker, and the town's houses and their camps
/// as the places of the history: the week rolled (modforge
/// `history::roll`) and laid down. Each ends standing where they ended the
/// week; each death through
/// the one hit path (`combat::hit_actor`), so `die` leaves the body there;
/// a take moves a stack from a box in that house into the taker's bag; and
/// every event goes into the memory of those it happened to, at its day.
/// Answers how many events there were.
#[allow(clippy::type_complexity)]
fn lay_down_history(
    world: Res<crate::terrain::WorldRes>,
    tick: Res<crate::SimTick>,
    factions: Res<crate::content::FactionRegistryRes>,
    items: Res<crate::content::ItemRegistryRes>,
    layers: Res<crate::content::LayerRegistryRes>,
    damage: Res<crate::content::DamageRegistryRes>,
    step: Res<Time<Fixed>>,
    mut people: Query<
        (
            &crate::actor::Actor,
            &crate::actor::Npc,
            &mut Transform,
            &crate::combat::Protection,
            &crate::combat::Equipment,
            &mut crate::combat::Health,
            &mut crate::actor::Persona,
            &mut crate::actor::Inventory,
        ),
        (
            Without<crate::player::Player>,
            Without<crate::actor::Dead>,
            Without<crate::interact::Container>,
            // A bunker's people keep to their bunker; a car is no one.
            Without<crate::actor::Store>,
            Without<crate::vehicle::Vehicle>,
        ),
    >,
    mut boxes: Query<(&Transform, &mut crate::interact::Container), Without<crate::actor::Npc>>,
) -> usize {
    let Some(town) = world.world.town.as_ref() else {
        return 0;
    };
    let houses: Vec<(usize, &modforge::town::Lot)> = town.lots.iter().enumerate().filter(|(_, l)| l.house).collect();
    let ground = |t: &Transform| Vec2::new(t.translation.x, t.translation.z);
    let residents = town.residents();
    // The places: the town's houses, then where each one living out of the
    // town (a site's scavengers, raiders, wolves) stands, their camp.
    let mut camps: Vec<Vec2> = Vec::new();
    let mut figures: Vec<modforge::history::Figure> = Vec::new();
    // In ActorId order, never the query's (authority.md rule 5).
    let mut order: Vec<_> = people.iter().map(|(actor, npc, at, ..)| (actor.0.0, npc.def.clone(), npc.faction.clone(), ground(at))).collect();
    order.sort_by_key(|o| o.0);
    for (id, kind, faction, p) in order {
        let home = if residents.iter().any(|(_, r)| r.distance(p) < 0.5) {
            houses.iter().enumerate().min_by(|a, b| a.1.1.at.distance(p).total_cmp(&b.1.1.at.distance(p))).map(|(i, _)| i)
        } else {
            camps.push(p);
            Some(houses.len() + camps.len() - 1)
        };
        if let Some(home) = home {
            figures.push(modforge::history::Figure { id, kind, group: faction, home });
        }
    }
    // Where a place is: a room of the house (the town's `home_spots`), or
    // the camp.
    let place = |i: usize, id: u64| match houses.get(i) {
        Some(&(index, lot)) => {
            let spots = town.home_spots(index);
            lot.at + spots.get((id as usize) % spots.len().max(1)).copied().unwrap_or(Vec2::ZERO)
        }
        None => camps[i - houses.len()],
    };
    // The faction registry first: two it holds hostile fight. Else the
    // chance hostility occurs between their kinds (content history_fights).
    let fights = |a: &modforge::history::Figure, b: &modforge::history::Figure| {
        if factions.0.relation(a.group.as_deref(), b.group.as_deref()) == modforge::faction::Relation::Hostile {
            return 1000;
        }
        crate::content::history_fights((&a.kind, a.group.as_deref()), (&b.kind, b.group.as_deref()))
    };
    let events = modforge::history::roll(&figures, houses.len() + camps.len(), crate::content::HISTORY_DAYS, &fights, world.seed ^ 0x4157_0123);
    let ends = modforge::history::ends_at(&figures, &events);
    let day_ticks = (crate::content::GAME_DAY_SECS / step.timestep().as_secs_f32()).round() as u64;
    let when = |day: u32| tick.0 + u64::from(day) * day_ticks + day_ticks / 2;
    // Where each ends.
    for (f, &end) in figures.iter().zip(&ends) {
        if end == f.home {
            continue;
        }
        if let Some((.., mut at, _, _, _, _, _)) = people.iter_mut().find(|p| p.0.0.0 == f.id) {
            // Their height kept: the body stands as `settle` stood it.
            let to = place(end, f.id);
            at.translation = crate::on_ground(to, at.translation.y);
        }
    }
    let kill = damage.0.def("kill");
    for event in &events {
        match *event {
            modforge::history::Event::Moved { .. } => {}
            modforge::history::Event::Fought { day, who, with, .. } => {
                for (me, them) in [(who, with), (with, who)] {
                    if let Some((.., mut persona, _)) = people.iter_mut().find(|p| p.0.0.0 == me) {
                        persona.0.memory.did(modforge::memory::Did::Hit(modforge::actor::ActorId(them), 0.0), when(day));
                        persona.0.memory.did(modforge::memory::Did::WasHit(Some(modforge::actor::ActorId(them)), 0.0), when(day));
                    }
                }
            }
            modforge::history::Event::Died { day, who, by, .. } => {
                if let Some((.., mut persona, _)) = people.iter_mut().find(|p| p.0.0.0 == by) {
                    persona.0.memory.did(modforge::memory::Did::Killed(modforge::actor::ActorId(who)), when(day));
                }
                if let (Some(def), Some((_, _, _, own, worn, mut health, mut persona, _))) = (kill, people.iter_mut().find(|p| p.0.0.0 == who)) {
                    let hit = modforge::combat::Hit { def, self_inflicted: false, distance: 0.0, location_scale: 1.0 };
                    crate::combat::hit_actor(&hit, own, worn, &items.0, &layers.0, Default::default(), &mut health, Some(&mut *persona), Some(modforge::actor::ActorId(by)), when(day));
                }
            }
            modforge::history::Event::Took { day, who, at } => {
                // A box in the house, or within 10 m of the camp.
                let near = |p: Vec2| houses.get(at).map_or_else(|| camps[at - houses.len()].distance(p) < 10.0, |(_, lot)| lot.covers(p));
                let Some((_, mut held)) = boxes.iter_mut().find(|(t, c)| near(ground(t)) && c.0.slots.iter().any(Option::is_some)) else {
                    continue;
                };
                let Some(slot) = held.0.slots.iter().position(Option::is_some) else {
                    continue;
                };
                let Some((.., mut persona, mut bag)) = people.iter_mut().find(|p| p.0.0.0 == who) else {
                    continue;
                };
                let name = held.0.slots[slot].as_ref().map(|s| (s.item.clone(), s.count)).unwrap_or_default();
                let max = items.0.def(&name.0).map_or(1, |d| d.max_stack);
                modforge::item::transfer(&mut held.0, &mut bag.0, slot, max);
                persona.0.memory.did(modforge::memory::Did::Took(name.0, name.1), when(day));
            }
        }
    }
    events.len()
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
