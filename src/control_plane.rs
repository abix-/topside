//! The modforge control plane embedded in the 2D game: the one runtime
//! inspection surface (the runtime-control-http doctrine).
//!
//! POST http://127.0.0.1:15703/topside with {"op": "...", "args": {}}.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use modforge::envelope::OpResponse;
use modforge::ops::{OP_REGISTRY, OpDef};
use serde_json::{Value as Json, json};

use crate::Ground;
use crate::actor::{Actor, ActorMapRes, Npc, Personality, Survival};
use crate::combat::Health;
use crate::input::ThisTick;
use crate::player::Player;

pub const PORT: u16 = 15703;

// The server runs on its own thread and cannot borrow the ECS world,
// so the simulation publishes a snapshot here each tick.
static TICK: AtomicU64 = AtomicU64::new(0);
static PEOPLE: AtomicUsize = AtomicUsize::new(0);
static HEALTH_BITS: AtomicU32 = AtomicU32::new(0);
static HUNGER_BITS: AtomicU32 = AtomicU32::new(0);
static THIRST_BITS: AtomicU32 = AtomicU32::new(0);
static X_BITS: AtomicU32 = AtomicU32::new(0);
static Y_BITS: AtomicU32 = AtomicU32::new(0);
/// How high the player's feet stand above the ground (a floor upstairs).
static FEET_BITS: AtomicU32 = AtomicU32::new(0);
static FPS_BITS: AtomicU32 = AtomicU32::new(0);
/// How many times real time the game actually ran over the last second.
static RUNS_AT_BITS: AtomicU32 = AtomicU32::new(0);
static DIAGNOSTICS: Mutex<Json> = Mutex::new(Json::Null);
static PEOPLE_LIST: Mutex<Json> = Mutex::new(Json::Null);
static PERF: Mutex<Json> = Mutex::new(Json::Null);
static WORLD: Mutex<Json> = Mutex::new(Json::Null);
static PLAYER: Mutex<Json> = Mutex::new(Json::Null);
static WAYS: Mutex<Json> = Mutex::new(Json::Null);
/// The `kill` op's request: the ActorId to kill on the game's next step.
static KILL_ASK: Mutex<Option<u64>> = Mutex::new(None);
/// The `needs` op's requests: whose (ActorId), and their hunger and thirst
/// set, when given.
static NEEDS_ASK: Mutex<Vec<(u64, Option<f32>, Option<f32>)>> = Mutex::new(Vec::new());
/// The `forget` op's requests: whose things known are forgotten.
static FORGET_ASK: Mutex<Vec<u64>> = Mutex::new(Vec::new());

/// The `needs` and `forget` ops' requests, on the game's next step: a
/// person's hunger and thirst set; every thing a person knows, and all they
/// learned, forgotten (so a test sets up what someone needs and knows).
pub fn answer_needs(mut people: Query<(&Actor, &mut crate::actor::Survival, &mut crate::actor::Memory)>) {
    let needs: Vec<(u64, Option<f32>, Option<f32>)> = NEEDS_ASK.lock().map(|mut ask| ask.drain(..).collect()).unwrap_or_default();
    for (id, hunger, thirst) in needs {
        if let Some((_, mut survival, _)) = people.iter_mut().find(|(a, ..)| a.0.0 == id) {
            if let Some(h) = hunger {
                survival.0.hunger = h;
            }
            if let Some(t) = thirst {
                survival.0.thirst = t;
            }
        }
    }
    let forget: Vec<u64> = FORGET_ASK.lock().map(|mut ask| ask.drain(..).collect()).unwrap_or_default();
    for id in forget {
        if let Some((_, _, mut memory)) = people.iter_mut().find(|(a, ..)| a.0.0 == id) {
            memory.0.known.clear();
            memory.0.learned = Default::default();
        }
    }
}

/// The `give` op's requests: to whom (ActorId), what item, how many, and
/// whether their inventory is emptied first.
static GIVE_ASK: Mutex<Vec<(u64, String, u32, bool)>> = Mutex::new(Vec::new());

/// The `give` op's requests, on the game's next step: the inventory emptied
/// if asked, then each item made (modforge `item::create`, the one way
/// things come into being) into it, as far as it has room.
pub fn answer_give(items: Res<crate::content::ItemRegistryRes>, mut people: Query<(&Actor, &mut crate::actor::Inventory)>) {
    let asks: Vec<(u64, String, u32, bool)> = GIVE_ASK.lock().map(|mut ask| ask.drain(..).collect()).unwrap_or_default();
    for (id, item, count, clear) in asks {
        let (Some(def), Some((_, mut inventory))) = (items.0.def(&item), people.iter_mut().find(|(a, _)| a.0.0 == id)) else {
            warn!("give: no item {item}, or nobody by id {id}");
            continue;
        };
        if clear {
            inventory.0.drain_all();
        }
        if let Some(left) = inventory.0.add(modforge::item::create(def, count, &[], 0.0, 0), def.max_stack) {
            warn!("give: no room for {} {item}", left.count);
        }
    }
}
/// Where the `use` op's person uses: a point, or wherever a person stands
/// when the game takes the request (people keep walking).
#[derive(Clone, Copy)]
enum UseAt {
    /// A point on the ground, and the height of the floor it is on.
    Point(Vec2, f32),
    Person(u64),
}

/// The `use` op's request: the ActorId, and where to use.
static USE_ASK: Mutex<Option<(u64, UseAt)>> = Mutex::new(None);
/// One step of driving the mouse the way the player does (the `pointer`
/// op): the pointer moved to a window point, its button pressed or
/// released; `Next` ends this frame's steps, so a press is seen one frame
/// before its release. (The `key` op's keys go to input::press_from_op.)
#[derive(Clone, Copy, Debug)]
enum Drive {
    Move(Vec2),
    Press,
    Release,
    Next,
}
static DRIVE: Mutex<Vec<Drive>> = Mutex::new(Vec::new());

/// The pointer the `pointer` op drives: a pointer of its own beside the
/// real mouse, so the player's hand on the mouse never moves it (the real
/// mouse moving over the window replaced the op's position on the one
/// mouse pointer). Bevy picking hovers and clicks the UI for every pointer
/// the same way.
pub const DRIVEN: bevy::picking::pointer::PointerId =
    bevy::picking::pointer::PointerId::Custom(bevy::asset::uuid::Uuid::from_u128(0x7070_1de0_0000_4000_8000_000000000001));

/// The driven pointer, once, with a window.
pub fn spawn_driven_pointer(mut commands: Commands) {
    commands.spawn(DRIVEN);
}

/// A key's name, as the `key` op takes it, to Bevy's code.
fn key_code(name: &str) -> Option<KeyCode> {
    Some(match name {
        "W" => KeyCode::KeyW,
        "A" => KeyCode::KeyA,
        "S" => KeyCode::KeyS,
        "D" => KeyCode::KeyD,
        "E" => KeyCode::KeyE,
        "I" => KeyCode::KeyI,
        "M" => KeyCode::KeyM,
        "T" => KeyCode::KeyT,
        "O" => KeyCode::KeyO,
        "Tab" => KeyCode::Tab,
        "Escape" => KeyCode::Escape,
        "Enter" => KeyCode::Enter,
        "Space" => KeyCode::Space,
        "1" => KeyCode::Digit1,
        "2" => KeyCode::Digit2,
        "3" => KeyCode::Digit3,
        "4" => KeyCode::Digit4,
        "5" => KeyCode::Digit5,
        "6" => KeyCode::Digit6,
        _ => return None,
    })
}

/// Once a frame, right after Bevy has read the real mouse and keys: the
/// `pointer` op's steps up to the next frame's, as Bevy picking's
/// `PointerInput` on its own pointer (as Bevy's mouse backend writes
/// them). Nothing downstream can tell them from the player's hand.
pub fn drive_input(
    mut last: Local<Vec2>,
    windows: Query<Entity, With<bevy::window::PrimaryWindow>>,
    mut pointers: MessageWriter<bevy::picking::pointer::PointerInput>,
) {
    use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerInput};
    let steps: Vec<Drive> = {
        let Ok(mut queue) = DRIVE.lock() else {
            return;
        };
        let end = queue.iter().position(|s| matches!(s, Drive::Next)).map_or(queue.len(), |i| i + 1);
        queue.drain(..end).collect()
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(target) = bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(window)).normalize(Some(window)) else {
        return;
    };
    for step in steps {
        let location = |position| Location { target: target.clone(), position };
        match step {
            Drive::Move(to) => {
                pointers.write(PointerInput::new(DRIVEN, location(to), PointerAction::Move { delta: to - *last }));
                *last = to;
            }
            Drive::Press => {
                pointers.write(PointerInput::new(DRIVEN, location(*last), PointerAction::Press(PointerButton::Primary)));
            }
            Drive::Release => {
                pointers.write(PointerInput::new(DRIVEN, location(*last), PointerAction::Release(PointerButton::Primary)));
            }
            Drive::Next => {}
        }
    }
}

/// The `talk` op's request: who speaks, to whom, saying what.
static TALK_ASK: Mutex<Option<(u64, u64, modforge::actions::Said)>> = Mutex::new(None);
/// The `npc` op's question (an ActorId) and the game's answer.
static NPC_ASK: Mutex<Option<u64>> = Mutex::new(None);
static NPC_ANSWER: Mutex<Option<(u64, Json)>> = Mutex::new(None);

/// The `walk` op's question (a ground point) and the game's answer.
static WALK_ASK: Mutex<Option<Vec2>> = Mutex::new(None);
static WALK_ANSWER: Mutex<Option<Json>> = Mutex::new(None);

/// What a solid thing is, to name what blocks a tile.
type Things<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static crate::structure::StructureRoot>,
        Option<&'static crate::interact::WorldItem>,
        Has<crate::terrain::WorldPiece>,
    ),
>;

/// The solids on the walk grid's tile under `at`, each named with what
/// it is and where it lies.
fn solids_on(walk: &mut crate::nav::Walk, things: &Things, at: Vec2) -> Vec<Json> {
    walk.0
        .solids_on(at)
        .into_iter()
        .map(|(key, r)| {
            let what = Entity::try_from_bits(key)
                .and_then(|e| things.get(e).ok())
                .map_or("gone".to_string(), |(building, item, piece)| match (building, item) {
                    (Some(_), _) => "a building (wall or furniture)".to_string(),
                    (_, Some(item)) => format!("an item: {}", item.stack.item),
                    _ if piece => "the ground (water, trees, rocks) or a monument's props".to_string(),
                    _ => "something else".to_string(),
                });
            json!({"what": what, "min": [r.min.x, r.min.y], "max": [r.max.x, r.max.y]})
        })
        .collect()
}

/// Answer the `walk` op's question, if one is waiting: the walk grid's
/// chunk at the point, drawn, and every solid on the tiles within 3 m.
pub fn answer_walk(mut walk: ResMut<crate::nav::Walk>, things: Things) {
    let Some(at) = WALK_ASK.lock().ok().and_then(|mut ask| ask.take()) else {
        return;
    };
    let mut solids: Vec<Json> = Vec::new();
    for y in -3..=3 {
        for x in -3..=3 {
            for s in solids_on(&mut walk, &things, at + Vec2::new(x as f32, y as f32)) {
                if !solids.contains(&s) {
                    solids.push(s);
                }
            }
        }
    }
    let p = walk.0.picture(at, '@');
    let answer = json!({
        "at": [at.x, at.y],
        "chunk": [p.chunk.0, p.chunk.1],
        "tile": [p.tile.0, p.tile.1],
        "tiles": p.rows,
        "solids": solids,
    });
    if let Ok(mut slot) = WALK_ANSWER.lock() {
        *slot = Some(answer);
    }
}

/// Everything about one person, for the `npc` op.
#[derive(bevy::ecs::system::SystemParam)]
pub struct PersonView<'w, 's> {
    people: Query<
        'w,
        's,
        (
            (&'static Actor, &'static Npc, &'static Transform, &'static Health, &'static Survival),
            (
                &'static crate::actor::ActorRecord,
                &'static crate::actor::Memory,
                &'static crate::actor::Inventory,
                &'static crate::actor::Hotbar,
            ),
            (
                Option<&'static crate::navmesh::Walker>,
                Option<&'static crate::actor::Home>,
                Option<&'static crate::actor::Store>,
                Has<crate::actor::Dead>,
                Option<&'static crate::actor::SpawnedAt>,
                Option<&'static crate::actor::ActorCondition>,
            ),
        ),
    >,
    /// Their walks on the mesh, through their `Walker`.
    ways: crate::navmesh::Ways<'w, 's>,
    /// What a solid thing is, to name what blocks a tile.
    things: Things<'w, 's>,
    /// What a person could use, to name what is in their reach.
    usable: Query<
        'w,
        's,
        (
            &'static crate::interact::Interactable,
            Option<&'static crate::interact::Door>,
            Option<&'static crate::interact::WorldItem>,
            &'static Transform,
        ),
    >,
    /// What they do this tick, and their commands left.
    this_tick: Query<'w, 's, (&'static Actor, &'static crate::input::ThisTick, Option<&'static crate::command::Commanded>)>,
    chunks: Option<Res<'w, crate::terrain::Chunks>>,
    /// A thing's id, what memory knows it by (their store's).
    thing_ids: Query<'w, 's, &'static crate::interact::ThingId>,
}

/// The `kill` op's request, if one is waiting: that person takes the
/// `kill` hit through the one hit path, and `die` does the rest.
pub fn answer_kill(
    tick: Res<crate::SimTick>,
    items: Res<crate::content::ItemRegistryRes>,
    layers: Res<crate::content::LayerRegistryRes>,
    damage: Res<crate::content::DamageRegistryRes>,
    mut people: Query<
        (&Actor, &crate::combat::Protection, &crate::combat::Equipment, &mut Health, Option<&mut crate::actor::Memory>),
        Without<crate::actor::Dead>,
    >,
) {
    let Some(id) = KILL_ASK.lock().ok().and_then(|mut ask| ask.take()) else {
        return;
    };
    let Some(def) = damage.0.def("kill") else {
        return;
    };
    for (actor, own, worn, mut health, mut memory) in &mut people {
        if actor.0.0 != id {
            continue;
        }
        let hit = modforge::combat::Hit {
            def,
            self_inflicted: false,
            distance: 0.0,
            location_scale: 1.0,
        };
        crate::combat::hit_actor(&hit, own, worn, &items.0, &layers.0, &mut health, memory.as_deref_mut(), None, tick.0);
    }
}

/// The `use` op's request, if one is waiting: that person stands a metre
/// from the point (or from the person, where they stand now) and pushes
/// Use aimed at it, the same Use the keys and the brain push, so the one
/// interact path does the rest.
pub fn answer_use(
    tick: Res<crate::SimTick>,
    nav: Res<crate::navmesh::Nav>,
    archipelagos: Query<&bevy_landmass::prelude::Archipelago3d>,
    mut people: Query<(&Actor, &mut Transform, &mut crate::input::Actions, Has<Player>), Without<crate::actor::Dead>>,
    mut eyes: Query<(&mut crate::player::Eyes, &mut Transform), Without<Actor>>,
) {
    let Some((id, to)) = USE_ASK.lock().ok().and_then(|mut ask| ask.take()) else {
        return;
    };
    // The point on the ground, and the floor it is on: as asked, or a
    // person's own.
    let (at, floor) = match to {
        UseAt::Point(at, floor) => (at, floor),
        UseAt::Person(whom) => match people.iter().find(|(a, ..)| a.0.0 == whom) {
            Some((_, there, ..)) => (there.translation.ground(), crate::interact::feet_of(&there).y),
            None => return,
        },
    };
    // A metre off, on the navigation mesh at its level (never inside a
    // wall).
    let target = crate::on_ground(at, floor);
    let stand = archipelagos
        .get(nav.archipelago)
        .ok()
        .and_then(|a| crate::navmesh::stand_by(a, target, 1.0, 0.0))
        .unwrap_or(target + Vec3::X);
    for (actor, mut transform, mut actions, is_player) in &mut people {
        if actor.0.0 != id {
            continue;
        }
        transform.translation = stand + Vec3::Y * crate::player::BODY_CENTRE;
        // The player's eyes on the middle of whatever stands there, a
        // metre off, so what they look at is what is there.
        if is_player && let Ok((mut eye, mut view)) = eyes.single_mut() {
            eye.pitch = (crate::player::BODY_CENTRE - crate::EYE_HEIGHT).atan2(1.0);
            view.rotation = crate::player::eye_rotation(eye.pitch);
        }
        actions.0.push(tick.0, crate::input::Action::Aim { x: at.x, y: at.y });
        actions.0.push(tick.0, crate::input::Action::Use);
    }
}

/// The `talk` op's request, if one is waiting: the speaker pushes the
/// `Talk`, the same the keys and the brain push; `talk::talk` does the rest.
pub fn answer_talk(tick: Res<crate::SimTick>, mut people: Query<(&Actor, &mut crate::input::Actions), Without<crate::actor::Dead>>) {
    let Some((id, to, said)) = TALK_ASK.lock().ok().and_then(|mut ask| ask.take()) else {
        return;
    };
    for (actor, mut actions) in &mut people {
        if actor.0.0 == id {
            actions.0.push(tick.0, crate::input::Action::Talk { to, said });
        }
    }
}

/// Answer the `npc` op's question, if one is waiting: everything the game
/// knows about that person, and their chunk as the walk grid sees it.
pub fn answer_npc(
    view: PersonView,
    mut walk: ResMut<crate::nav::Walk>,
    tick: Res<crate::SimTick>,
    tree: Res<crate::content::brain::BrainTreeRes>,
    episodes: Res<crate::content::EpisodeRegistryRes>,
    leaders: Query<(&Actor, &crate::ai::Leading)>,
) {
    let Some(id) = NPC_ASK.lock().ok().and_then(|mut ask| ask.take()) else {
        return;
    };
    let found = view.people.iter().find(|((actor, ..), _, _)| actor.0.0 == id);
    let answer = match found {
        None => json!({"id": id, "error": "no NPC with that id"}),
        Some(((_, npc, at, health, needs), (record, memory, inventory, hotbar), (walker, home, store, dead, spawned, condition))) => {
            let way = crate::navmesh::way_of(walker, &view.ways);
            let (walking, going_to) = (way.map(|w| w.0), way.map(|w| w.1));
            let here = at.translation.ground();
            // What blocks the tile they stand on, if anything.
            let standing_on = solids_on(&mut walk, &view.things, here);
            let stacks = |held: &modforge::item::Inventory| -> Vec<Json> {
                held.slots.iter().flatten().map(|s| json!({"item": s.item, "count": s.count})).collect()
            };
            let chunk = |p: modforge::walk::ChunkPicture| {
                json!({
                    "chunk": [p.chunk.0, p.chunk.1],
                    "tile": [p.tile.0, p.tile.1],
                    "doorways": p.doorways,
                    "doorways_reachable": p.reached,
                    "tiles": p.rows,
                })
            };
            let mut values: Vec<_> = memory.0.learned.values.iter().collect();
            values.sort_by(|a, b| b.1.total_cmp(a.1));
            let learned: Vec<Json> = values
                .into_iter()
                .map(|((s, c), v)| json!({"choice": format!("{c:?}"), "value": v, "situation": format!("{s:?}")}))
                .collect();
            // What Use would reach where they stand (interact_on_use): the
            // things within reach, and a river's water.
            let in_reach: Vec<String> = view
                .usable
                .iter()
                .filter(|(_, door, _, t)| crate::interact::in_reach(crate::interact::feet_of(at), crate::interact::use_point(*door, t)))
                .map(|(thing, _, item, _)| format!("{:?} {}", thing.kind, item.map_or("", |i| i.stack.item.as_str())))
                .collect();
            let water_in_reach = view.chunks.as_ref().and_then(|c| crate::interact::nearest_water(c, here, here));
            let acting: Vec<String> = view
                .this_tick
                .iter()
                .find(|(a, ..)| a.0.0 == id)
                .map(|(_, t, _)| t.0.iter().map(|s| format!("{:?}", s.action)).collect())
                .unwrap_or_default();
            let commands_left: Option<Vec<String>> = view
                .this_tick
                .iter()
                .find(|(a, ..)| a.0.0 == id)
                .and_then(|(.., c)| c)
                .map(|c| c.left.iter().map(|c| format!("{c:?}")).collect());
            let picture = chunk(walk.0.picture(here, '@'));
            json!({
                "id": id,
                "tick": tick.0,
                "def": npc.def,
                "faction": npc.faction,
                "dead": dead,
                "at": [here.x, here.y],
                "feet": crate::interact::feet_of(at).y,
                "spawned_at": spawned.map(|s| [s.0.x, s.0.z]),
                "standing_on": standing_on,
                "in_reach": in_reach,
                "water_in_reach": water_in_reach.map(|w| [w.x, w.y]),
                "this_tick": acting,
                "commands": commands_left,
                "health": health.0.current,
                "needs": {"hunger": needs.0.hunger, "thirst": needs.0.thirst, "rest": needs.0.rest, "safety": needs.0.safety},
                "doing": doing_name(&tree, &record.0),
                "about": format!("{:?}", record.0.target),
                "asleep": condition.is_some_and(|c| c.0.asleep),
                "home": home.map(|h| [h.0.x, h.0.z]),
                "store": store.and_then(|s| view.thing_ids.get(s.0).ok()).map(|id| id.0),
                "carrying": stacks(&inventory.0),
                "hotbar": stacks(&hotbar.0.slots),
                "knows": memory.0.known.iter().map(|k| json!({
                    "key": k.key,
                    "kind": k.kind,
                    "at": [k.position.x, k.position.z],
                    "seen_at": k.seen_at,
                    "checked_at": k.checked_at,
                    "held": k.held,
                    "told_by": k.told_by.map(|a| a.0),
                })).collect::<Vec<_>>(),
                "quiet": memory.0.quiet,
                // Who they lead, to where, and where they wait for them.
                "leading": leaders.iter().find(|(a, _)| a.0.0 == id).map(|(_, l)| json!({
                    "who": l.who.0,
                    "to": [l.to.x, l.to.z],
                    "waiting_at": l.waiting_at.map(|w| [w.x, w.z]),
                })),
                // Their walk on the navigation mesh: landmass's state, and
                // where they are going [x, height, z].
                "way": {
                    "state": walking.map(|s| format!("{s:?}")),
                    "to": going_to.and_then(crate::navmesh::going_to).map(|p| [p.x, p.y, p.z]),
                },
                "chunk": picture,
                "did": memory.0.done.iter().map(|(t, d)| json!({"tick": t, "did": d.words()})).collect::<Vec<_>>(),
                // How they feel toward each person in their memory (life.md
                // "How a person feels about others"), -1 to 1, and the
                // last hostile they saw.
                "feelings": modforge::relationship::feelings(&memory.0, |w| episodes.0.felt(w))
                    .into_iter()
                    .map(|(who, felt)| json!({"toward": who.0, "feeling": felt, "relation": format!("{:?}", modforge::relationship::relation_of(felt))}))
                    .collect::<Vec<_>>(),
                "last_threat": memory.0.last_threat.map(|(who, _, when)| json!({"who": who.0, "tick": when})),
                // What they learned (life.md "Learning to stay alive"): how
                // long they could stay alive at their last think, how long
                // each life lasted, and each choice's value in each
                // situation, best first.
                "alive_for": memory.0.learned.last.map(|(_, secs)| secs),
                "lives": lives(&memory.0),
                "learned": learned,
            })
        }
    };
    if let Ok(mut slot) = NPC_ANSWER.lock() {
        *slot = Some((id, answer));
    }
}

/// How long each of a person's lives lasted, in seconds, from their
/// memory: from the start of the game or their last death to each death.
/// The life they are living is not in it.
pub fn lives(memory: &modforge::memory::Memory) -> Vec<f32> {
    let mut from = 0;
    let mut out = Vec::new();
    for (tick, did) in &memory.done {
        if matches!(did, modforge::memory::Did::Died(..)) {
            out.push(tick.saturating_sub(from) as f32 / 60.0);
            from = *tick;
        }
    }
    out
}

/// Once a second: everyone sent somewhere the navigation mesh gives no way
/// to (no way, the place not on the mesh, or they themselves off it), for
/// the `ways` op: who, where they are and are going [x, height, z], and why.
#[allow(clippy::type_complexity)]
pub fn publish_ways(
    tick: Res<crate::SimTick>,
    walkers: Query<(&Actor, Option<&Npc>, &Transform, &crate::navmesh::Walker)>,
    ways: crate::navmesh::Ways,
) {
    if !due(tick.0) {
        return;
    }
    use bevy_landmass::AgentState;
    let list: Vec<Json> = walkers
        .iter()
        .filter_map(|(actor, npc, at, walker)| crate::navmesh::way_of(Some(walker), &ways).map(|(state, target)| (actor, npc, at, state, target)))
        .filter(|(.., state, _)| matches!(state, AgentState::NoPath | AgentState::TargetNotOnNavMesh | AgentState::AgentNotOnNavMesh))
        .map(|(actor, npc, at, state, target)| {
            let feet = crate::interact::feet_of(at);
            json!({
                "id": actor.0.0,
                "who": npc.map_or("the player".to_string(), |n| format!("{} ({:?})", n.def, n.faction)),
                "from": [feet.x, feet.y, feet.z],
                "to": crate::navmesh::going_to(target).map(|p| [p.x, p.y, p.z]),
                "why": format!("{state:?}"),
            })
        })
        .collect();
    if let Ok(mut slot) = WAYS.lock() {
        *slot = Json::Array(list);
    }
}
/// How many times real time the game runs, set by the `speed` op
/// (Factorio's game speed); 1 is normal.
/// The game starts at real time, 1x (operator, 2026-10-01; it started at
/// 4x from 2026-09-26).
static SPEED_BITS: AtomicU32 = AtomicU32::new(0x3F80_0000);

/// The `screenshot` op asked for one, taken the next frame: the path to
/// save it to.
static SCREENSHOT: Mutex<Option<String>> = Mutex::new(None);
/// Where screenshots are saved: logs/<name>.png, `screenshot` unnamed.
const SCREENSHOT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/logs");

/// What the camera sees, once a second, for the `view` op.
static VIEW: Mutex<Json> = Mutex::new(Json::Null);

/// Once a second: where the camera is and looks, and how many meshes
/// there are, by material, and how many the camera sees.
pub fn publish_view(
    tick: Res<crate::SimTick>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    meshes: Query<(
        Has<MeshMaterial3d<StandardMaterial>>,
        Has<MeshMaterial3d<crate::model::Cel>>,
        &ViewVisibility,
    ), With<Mesh3d>>,
    models: Query<(&crate::model::Model, &Transform)>,
    map_view: Option<Res<crate::minimap::MapView>>,
    players: Query<(&Transform, &crate::actor::Map), With<Player>>,
) {
    if !due(tick.0) {
        return;
    }
    // What the map last drew, against where the player stands and what
    // their map holds.
    let map = map_view.map(|v| {
        let player = players.single().ok().map(|(t, m)| {
            let tile = modforge::walk::cell_of(t.translation.ground());
            json!({"tile": [tile.0, tile.1], "tiles_seen": m.map.tiles_seen()})
        });
        json!({
            "full": v.full,
            "centre": [v.drawn.centre.0, v.drawn.centre.1],
            "span": v.drawn.span,
            "shown": v.drawn.shown,
            "player": player,
        })
    });
    // Floor tiles standing above the ground (a building's upper
    // storeys), and blocks standing above it (steps and landings).
    let above = |name: &str| models.iter().filter(|(m, t)| m.0 == name && t.translation.y > 0.5).count();
    let (floors_above, blocks_above) = (above("floor"), above("furniture"));
    let camera = cameras.iter().next().map(|c| {
        let (at, forward) = (c.translation(), c.forward());
        json!({"at": [at.x, at.y, at.z], "forward": [forward.x, forward.y, forward.z]})
    });
    let (mut standard, mut cel, mut seen, mut all) = (0, 0, 0, 0);
    for (is_standard, is_cel, visible) in &meshes {
        all += 1;
        standard += is_standard as u32;
        cel += is_cel as u32;
        seen += visible.get() as u32;
    }
    if let Ok(mut slot) = VIEW.lock() {
        *slot = json!({
            "camera": camera,
            "meshes": all,
            "standard": standard,
            "cel": cel,
            "seen": seen,
            "models": models.iter().count(),
            "floors_above": floors_above,
            "blocks_above": blocks_above,
            "map": map,
        });
    }
}

/// The layered items lying in the world, once a second, for the `items`
/// op.
static ITEMS: Mutex<Json> = Mutex::new(Json::Null);

/// Once a second: every item lying in the world with layers, its layers,
/// the numbers `numbers_of` works out from them, and the models it is
/// drawn with (its own and each child's).
pub fn publish_items(
    tick: Res<crate::SimTick>,
    registry: Res<crate::content::ItemRegistryRes>,
    layers: Res<crate::content::LayerRegistryRes>,
    items: Query<(&crate::interact::WorldItem, &Transform, Option<&crate::model::Model>, Option<&Children>)>,
    models: Query<&crate::model::Model>,
) {
    if !due(tick.0) {
        return;
    }
    let list: Vec<Json> = items
        .iter()
        .filter(|(item, ..)| !item.stack.layers.is_empty())
        .map(|(item, at, own, children)| {
            let combat = registry
                .0
                .def(&item.stack.item)
                .and_then(|def| modforge::item::numbers_of(def, &item.stack, &layers.0).combat);
            let drawn: Vec<&str> = own
                .into_iter()
                .chain(children.into_iter().flat_map(|c| c.iter()).filter_map(|c| models.get(c).ok()))
                .map(|m| m.0.as_str())
                .collect();
            json!({
                "item": item.stack.item,
                "layers": item.stack.layers,
                "at": [at.translation.x, at.translation.z],
                "reach": combat.as_ref().map(|c| c.reach),
                "spread_degrees": combat.as_ref().map(|c| c.spread_degrees),
                "delay": combat.as_ref().map(|c| c.delay),
                "models": drawn,
            })
        })
        .collect();
    if let Ok(mut slot) = ITEMS.lock() {
        *slot = Json::Array(list);
    }
}

/// The inventory screen, once a second, for the `hud` op.
static HUD: Mutex<Json> = Mutex::new(Json::Null);

/// Once a second: whether the inventory screen is open, and each panel of
/// slots (the equipment row, the inventory, the belt, the open box): its
/// holder, how many slots it shows, and where its middle stands on the
/// window, 0 the left or top edge and 1 the right or bottom.
pub fn publish_hud(
    tick: Res<crate::SimTick>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    screen: Query<&Visibility, With<crate::hud::InventoryScreen>>,
    panels: Query<(&crate::hud::Panel, &UiGlobalTransform)>,
    slots: Query<(&crate::hud::SlotUi, &crate::hud::SlotIcons, &UiGlobalTransform, &InheritedVisibility)>,
    deal: Query<(&UiGlobalTransform, &InheritedVisibility), With<crate::hud::DealButton>>,
    item_panel: Query<&crate::hud::ItemPanel>,
    talk_box: Query<&Visibility, With<crate::hud::TalkBox>>,
    choice_list: Query<&Node, With<crate::hud::ChoiceList>>,
    pointers: Query<(&bevy::picking::pointer::PointerId, &bevy::picking::pointer::PointerLocation)>,
    players: Query<&crate::interact::Hud, With<Player>>,
    hints: Query<(&crate::hud::HintsDone, &Transform), With<Player>>,
    conversation: Query<&crate::talk::Conversation, With<Player>>,
    real: Res<Time<Real>>,
) {
    if !due(tick.0) {
        return;
    }
    // The controls still shown (`hud::hints_shown`).
    let shown_hints = hints.single().map_or(Vec::new(), |(done, at)| crate::hud::hints_shown(done, crate::interact::feet_of(at).y));
    // The conversation as the HUD shows it (`HudState::talking`): whether
    // the player is in one, with whom, the last line, the choices (each
    // said by its number key), and whether the choices are drawn.
    let talking = players.single().ok().and_then(|h| h.0.talking.clone());
    let talk = json!({
        "open": talking.is_some(),
        "to": talking.as_ref().map(|t| t.with),
        "name": talking.as_ref().map(|t| t.name.clone()),
        "line": talking.as_ref().and_then(|t| t.line.clone()).map(|(by, words)| json!({"by": by, "words": words})),
        "choices": talking.as_ref().map_or(Vec::new(), |t| t.choices.iter().map(|c| c.words.clone()).collect()),
        "drawn": talk_box.single().is_ok_and(|v| *v != Visibility::Hidden) && choice_list.single().is_ok_and(|n| n.display != Display::None),
        // Real seconds the line shown still has to be read (talk.rs, modforge
        // `talk::reading_secs`).
        "reading_for": conversation.single().ok().and_then(|c| c.read).map(|(_, at)| (at - real.elapsed_secs_f64()).max(0.0)),
    });
    let Ok(window) = windows.single() else {
        return;
    };
    let (w, h) = (window.physical_width() as f32, window.physical_height() as f32);
    let list: Vec<Json> = panels
        .iter()
        .map(|(panel, at)| {
            json!({
                "holder": format!("{:?}", panel.0),
                "slots": slots.iter().filter(|(s, ..)| s.0.holder == panel.0).count(),
                "x": at.translation.x / w,
                "y": at.translation.y / h,
            })
        })
        .collect();
    // Every slot with something in it, the icons it shows, and, while it
    // is shown, where it is on screen in logical pixels (for the `pointer`
    // op).
    let scale = window.scale_factor();
    let on_screen = |at: &UiGlobalTransform, shown: &InheritedVisibility| shown.get().then(|| [at.translation.x / scale, at.translation.y / scale]);
    let filled: Vec<Json> = slots
        .iter()
        .filter_map(|(slot, icons, at, shown)| {
            let item = icons.item.as_ref()?;
            Some(json!({
                "holder": format!("{:?}", slot.0.holder),
                "index": slot.0.index,
                "item": item,
                "icons": icons.shown,
                "at": on_screen(at, shown),
            }))
        })
        .collect();
    let deal_at = deal.single().ok().and_then(|(at, shown)| on_screen(at, shown));
    // The item information panel: the selected slot and what it shows.
    let selected = item_panel.single().ok().and_then(|p| p.shown.as_ref()).map(|s| {
        json!({
            "holder": format!("{:?}", s.slot.holder),
            "index": s.slot.index,
            "name": s.name,
            "description": s.description,
            "information": s.information,
            "actions": s.actions.iter().map(|a| a.label()).collect::<Vec<_>>(),
            "count": s.count,
            "split": s.split,
        })
    });
    if let Ok(mut slot) = HUD.lock() {
        *slot = json!({
            "open": screen.single().is_ok_and(|v| *v != Visibility::Hidden),
            "panels": list,
            "filled": filled,
            "selected": selected,
            "talk": talk,
            // What the player can do with what they look at.
            "prompt": players.single().map_or(String::new(), |h| h.0.prompt.text.clone()),
            // Whether a note is open, being read.
            "reading": players.single().is_ok_and(|h| h.0.reading.is_some()),
            // The trade being made: with whom, the slots each side puts on
            // the table, and whether the last deal was refused.
            "trading": players.single().ok().and_then(|h| h.0.trading.as_ref()).map(|t| json!({
                "with": t.with, "name": t.name, "mine": t.mine, "theirs": t.theirs, "refused": t.refused, "no_room": t.no_room,
                "deal_at": deal_at,
            })),
            // The controls still shown, one a line.
            "hints": shown_hints,
            // Where Bevy picking has the `pointer` op's pointer now, in
            // logical pixels.
            "pointer": pointers
                .iter()
                .find(|(id, _)| **id == DRIVEN)
                .and_then(|(_, at)| at.location().map(|l| [l.position.x, l.position.y])),
        });
    }
}

/// What the window shows, saved to disk when the `screenshot` op asked.
pub fn take_screenshot(mut commands: Commands) {
    if let Some(path) = SCREENSHOT.lock().ok().and_then(|mut s| s.take()) {
        use bevy::render::view::screenshot::{Screenshot, save_to_disk};
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
}

/// The game speed the `speed` op asked for, applied to the clock the
/// fixed step runs on: the same game, more steps each frame. A frame may
/// catch up at most a second of game time, so a slow machine runs as fast
/// as it can, never skips steps.
pub fn apply_speed(mut time: ResMut<Time<Virtual>>) {
    let speed = f32::from_bits(SPEED_BITS.load(Ordering::Relaxed));
    if time.relative_speed() != speed {
        time.set_relative_speed(speed);
        time.set_max_delta(std::time::Duration::from_secs(1));
    }
    // Paused or not, as the `pause` op asked; `stand_still` publishes and
    // gives the op the tick it stopped at.
    let paused = PAUSED.load(Ordering::Relaxed);
    if paused != time.is_paused() {
        if paused {
            time.pause();
        } else {
            time.unpause();
        }
    }
}

/// The game paused by the `pause` op (Factorio's pause, for tests: the
/// world stands still while a test reads it), the tick it stands at
/// (`u64::MAX` while running), the ticks the `step` op asked for, and the
/// tick that stepping ends at.
static PAUSED: AtomicBool = AtomicBool::new(false);
static PAUSED_AT: AtomicU64 = AtomicU64::new(u64::MAX);
static STEP_ASK: AtomicU64 = AtomicU64::new(0);
static STEP_TO: Mutex<Option<u64>> = Mutex::new(None);
static STEP_ANSWER: Mutex<Option<Json>> = Mutex::new(None);

/// Before the fixed steps run, while paused: the steps the `step` op asked
/// for are handed to the fixed step's own clock, exactly that many steps'
/// time and nothing left over, so Bevy's fixed loop runs exactly that many
/// ticks the way it always runs them.
pub fn step_ticks(mut fixed: ResMut<Time<Fixed>>, time: Res<Time<Virtual>>, tick: Res<crate::SimTick>) {
    if !time.is_paused() {
        return;
    }
    let steps = STEP_ASK.swap(0, Ordering::Relaxed);
    if steps == 0 {
        return;
    }
    let left = fixed.overstep();
    fixed.discard_overstep(left);
    let timestep = fixed.timestep();
    fixed.accumulate_overstep(timestep * steps as u32);
    if let Ok(mut to) = STEP_TO.lock() {
        *to = Some(tick.0 + steps);
    }
}

/// After the fixed steps, once stepping has reached its tick: what the
/// ops read published (`publish_now`), then the `step` op answered with
/// the moment there (crate::save::Moment).
pub fn answer_step(world: &mut World) {
    let tick = world.resource::<crate::SimTick>().0;
    match STEP_TO.lock() {
        Ok(mut to) if *to == Some(tick) => *to = None,
        _ => return,
    }
    publish_now(world);
    let moment = world.run_system_cached(|moment: crate::save::Moment| moment.read()).unwrap_or(Json::Null);
    if let Ok(mut slot) = STEP_ANSWER.lock() {
        *slot = Some(moment);
    }
}

/// While set, every publisher writes its copy now, whatever the tick.
static PUBLISH_NOW: AtomicBool = AtomicBool::new(false);

/// Whether a publisher writes its copy this tick: once a game second in
/// play, and whenever the game comes to a stop for a test (`publish_now`).
fn due(tick: u64) -> bool {
    tick % 60 == 0 || PUBLISH_NOW.load(Ordering::Relaxed)
}

/// Every copy the ops read written now, by the same publishers that write
/// them once a game second: called when the game comes to a stop for a
/// test (a load done, a step done, a pause taking hold), so an op never
/// answers from before it (authority.md rule 5).
pub fn publish_now(world: &mut World) {
    PUBLISH_NOW.store(true, Ordering::Relaxed);
    let _ = world.run_system_cached(publish);
    let _ = world.run_system_cached(publish_world);
    let _ = world.run_system_cached(publish_ways);
    let _ = world.run_system_cached(publish_view);
    let _ = world.run_system_cached(publish_items);
    let _ = world.run_system_cached(publish_hud);
    PUBLISH_NOW.store(false, Ordering::Relaxed);
}

/// Every frame, after the speed and pause are applied: a pause that has
/// just taken hold publishes what the ops read before the `pause` op
/// answers with the tick it stands at.
pub fn stand_still(world: &mut World) {
    let paused = world.resource::<Time<Virtual>>().is_paused();
    if !paused {
        PAUSED_AT.store(u64::MAX, Ordering::Relaxed);
        return;
    }
    if PAUSED_AT.load(Ordering::Relaxed) == u64::MAX {
        publish_now(world);
        PAUSED_AT.store(world.resource::<crate::SimTick>().0, Ordering::Relaxed);
    }
}

/// Once a second of game time: the day, the storms, what the people have
/// done, and every bunker's store and people, for the `world` op.
pub fn publish_world(
    tick: Res<crate::SimTick>,
    day: Res<crate::survival::Day>,
    storms: Res<crate::storm::StormRes>,
    items: Res<crate::content::ItemRegistryRes>,
    episodes: Res<crate::content::EpisodeRegistryRes>,
    memories: Query<(&crate::actor::Memory, &Actor, Option<&Npc>)>,
    stores: Query<&crate::interact::Container>,
    people: Query<
        (
            (&crate::actor::Store, &Npc, Has<crate::actor::Dead>, &Survival),
            (&crate::actor::Home, &Transform, &crate::actor::ActorCondition, &crate::actor::Memory),
        ),
    >,
    chunks: Option<Res<crate::terrain::Chunks>>,
    rolled: Option<Res<crate::terrain::WorldRes>>,
    standing: Option<Res<crate::structure::MonumentsRes>>,
    reality: Res<crate::Reality>,
    episode: Res<crate::Episode>,
    below: Query<(&Transform, &crate::structure::Below)>,
    doors: Query<(&crate::interact::Door, &Transform)>,
) {
    if !due(tick.0) {
        return;
    }
    // Every building that goes below the ground: where it stands, its
    // lowest floor, and how many ground tiles it leaves open over the way
    // down.
    let below: Vec<Json> = below
        .iter()
        .map(|(at, b)| json!({"at": [at.translation.x, at.translation.z], "bottom": b.bottom, "open": b.open.len(), "tiles": b.rects.len()}))
        .collect();
    // The doors of the player's bunker: the middle of each closed, the
    // height of its floor, and whether it stands open.
    let bunker = rolled.as_ref().map_or(Vec2::ZERO, |w| w.bunker);
    let bunker_doors: Vec<Json> = doors
        .iter()
        .filter(|(d, _)| d.closed_center.distance(bunker) <= 30.0)
        .map(|(d, at)| json!({"at": [d.closed_center.x, d.closed_center.y], "floor": at.translation.y, "open": d.open}))
        .collect();
    let night = crate::survival::night(&tick, *day, &reality);
    // The reality the world is in now, and its rules (realities.md).
    let reality = json!({
        "name": reality.0.name,
        "danger": reality.0.danger.name(),
        "night": reality.0.night,
        "death_sticks": reality.0.death_sticks,
    });
    // The chunks made, and for a world with an edge, how many of the
    // chunks it is made whole with (the world and the ring past its
    // edge, terrain::make_chunks) are still not made.
    let made = chunks.as_ref().map_or(0, |c| c.made.len());
    let missing = rolled.as_ref().and_then(|w| w.world.ground.chunk_range()).map(|(lo, hi)| {
        (lo - 1..=hi)
            .flat_map(|y| (lo - 1..=hi).map(move |x| (x, y)))
            .filter(|key| !chunks.as_ref().is_some_and(|c| c.made.contains(key)))
            .count()
    });
    let mut bunkers: std::collections::BTreeMap<String, Json> = Default::default();
    for ((store, npc, dead, needs), (home, at, condition, memory)) in &people {
        let name = npc.faction.clone().unwrap_or_default();
        let entry = bunkers.entry(name).or_insert_with(|| {
            let (mut food, mut water) = (0, 0);
            if let Ok(held) = stores.get(store.0) {
                for stack in held.0.slots.iter().flatten() {
                    if let Some(f) = items.0.def(&stack.item).and_then(|d| d.food) {
                        if f.hunger > 0.0 {
                            food += stack.count;
                        }
                        if f.thirst > 0.0 {
                            water += stack.count;
                        }
                    }
                }
            }
            json!({"food": food, "water": water, "alive": 0, "starving": 0, "at_home": 0, "asleep": 0, "stocked": 0})
        });
        let add = |entry: &mut Json, field: &str, n: u64| {
            entry[field] = json!(entry[field].as_u64().unwrap_or(0) + n);
        };
        if !dead {
            add(entry, "alive", 1);
            // Home, as the brain counts it.
            if crate::interact::feet_of(at).distance(home.0) <= crate::ai::HOME_REACH {
                add(entry, "at_home", 1);
            }
            if condition.0.asleep {
                add(entry, "asleep", 1);
            }
        }
        if needs.0.starving() || needs.0.dehydrated() {
            add(entry, "starving", 1);
        }
        // Times they put something into the store, from their memory.
        let stocked = memory.0.done.iter().filter(|(_, d)| matches!(d, modforge::memory::Did::Stocked(..))).count();
        add(entry, "stocked", stocked as u64);
    }
    // What everyone did, alive and dead, each kind counted, read from
    // their memories (life.md: memory is the one record).
    let mut did = serde_json::Map::new();
    for (memory, ..) in &memories {
        for (_, d) in &memory.0.done {
            let n = did.get(d.kind()).and_then(Json::as_u64).unwrap_or(0);
            did.insert(d.kind().to_string(), json!(n + 1));
        }
    }
    // Each faction's standing toward each faction (life.md "How a person
    // feels about others"): its members' feelings toward that faction's
    // people taken together, worked out here, never stored.
    let faction_of: std::collections::HashMap<u64, String> = memories
        .iter()
        .filter_map(|(_, actor, npc)| {
            let faction = npc.map_or(Some(crate::content::PLAYER_FACTION.to_string()), |n| n.faction.clone());
            faction.map(|f| (actor.0.0, f))
        })
        .collect();
    // Each member's feeling toward a faction: their feelings toward its
    // people taken together; none when they feel nothing toward any.
    let names: std::collections::BTreeSet<&String> = faction_of.values().collect();
    let mut factions: Vec<Json> = Vec::new();
    for from in &names {
        let members: Vec<Vec<(modforge::actor::ActorId, f32)>> = memories
            .iter()
            .filter(|(_, actor, _)| faction_of.get(&actor.0.0) == Some(*from))
            .map(|(memory, ..)| modforge::relationship::feelings(&memory.0, |w| episodes.0.felt(w)))
            .collect();
        for to in &names {
            let toward = members.iter().map(|felt| {
                let theirs: Vec<Option<f32>> = felt.iter().filter(|(who, _)| faction_of.get(&who.0) == Some(*to)).map(|(_, f)| Some(*f)).collect();
                modforge::relationship::standing(theirs)
            });
            if let Some(standing) = modforge::relationship::standing(toward) {
                factions.push(json!({"faction": from, "toward": to, "standing": standing, "relation": format!("{:?}", modforge::relationship::relation_of(standing))}));
            }
        }
    }
    let world = json!({
        "tick": tick.0,
        "day": crate::survival::Day::secs(&tick) / day.0,
        "night": night,
        // Whether the day clock is in its night hours, night or not in
        // this reality.
        "night_hours": crate::survival::night_hours(&tick, *day),
        "storms": storms.shifts,
        "storm": format!("{:?}", crate::storm::phase(&tick, *day, &storms)).to_lowercase(),
        "reality": reality,
        "episode": episode.name,
        // Each part and the ActorId cast for it.
        "cast": episode.cast.iter().map(|(part, id)| json!({"part": part, "id": id.0})).collect::<Vec<_>>(),
        // What the episode brought into the world, and where.
        "things": episode.things.iter().map(|(name, at, key)| json!({"name": name, "at": [at.x, at.z], "height": at.y, "key": key})).collect::<Vec<_>>(),
        // Each pivot point reached: who and when.
        "reached": episode.reached.iter().map(|(pivot, who, at)| json!({"pivot": pivot, "by": who.0, "tick": at})).collect::<Vec<_>>(),
        "did": did,
        "factions": factions,
        "bunkers": bunkers,
        "below": below,
        "bunker_doors": bunker_doors,
        "chunks": {"made": made, "missing": missing},
        // The seed this world rolled from, and the road tiles in its chunks.
        "seed": rolled.as_ref().map(|w| w.seed),
        "roads": chunks.as_ref().map_or(0, |c| c.roads),
        // Every monument standing: its kind, where, and how many buildings.
        "monuments": standing.as_ref().map_or(Vec::new(), |m| m.0.iter().map(|(at, def)| json!({
            "kind": def.kind,
            "at": [at.x, at.y],
            "buildings": def.members.len(),
        })).collect::<Vec<_>>()),
    });
    if let Ok(mut slot) = WORLD.lock() {
        *slot = world;
    }
}

/// What the brain's tree has a person doing: the active state's name, or
/// "nothing" between one thing and the next.
fn doing_name(tree: &crate::content::brain::BrainTreeRes, record: &modforge::statetree::Record) -> &'static str {
    record.state.map_or("nothing", |s| tree.0.name(s))
}

pub fn publish(
    tick: Res<crate::SimTick>,
    real: Res<Time<Real>>,
    mut last_second: Local<Option<(u64, f64)>>,
    laps: Res<crate::profile::Laps>,
    actors: Res<ActorMapRes>,
    diagnostics: Option<Res<DiagnosticsStore>>,
    items: Res<crate::content::ItemRegistryRes>,
    layers: Res<crate::content::LayerRegistryRes>,
    tree: Res<crate::content::brain::BrainTreeRes>,
    players: Query<
        (&Transform, &Health, &Survival, &Actor, &crate::actor::Memory, Has<crate::actor::Dead>, Option<&crate::ai::Following>, &crate::actor::Inventory),
        With<Player>,
    >,
    player_walk: (Query<(Option<&crate::navmesh::Walker>, &ThisTick, Option<&crate::command::Commanded>), With<Player>>, crate::navmesh::Ways),
    npcs: Query<(
        (&Actor, &Npc, &Transform, &Health, &Survival, &Personality, &ThisTick, Option<&Name>),
        (&crate::actor::Memory, &crate::actor::ActorRecord, Has<crate::actor::Dead>, &crate::actor::Hotbar, &crate::actor::Facing, Option<&crate::people::Drawn>, &crate::actor::Map, &crate::combat::Equipment),
    )>,
) {
    TICK.store(tick.0, Ordering::Relaxed);
    PEOPLE.store(actors.0.live_count(), Ordering::Relaxed);
    // How fast the game runs against real time, measured once a second:
    // the game seconds its ticks covered over the real seconds that
    // passed (1 is keeping up).
    if tick.0 % 60 == 0 {
        let now = real.elapsed_secs_f64();
        // A load sets the tick back: measured afresh from there.
        if let Some((then_tick, then)) = *last_second
            && now > then
            && let Some(ticks) = tick.0.checked_sub(then_tick)
        {
            let speed = ticks as f64 / 60.0 / (now - then);
            RUNS_AT_BITS.store((speed as f32).to_bits(), Ordering::Relaxed);
        }
        *last_second = Some((tick.0, now));
    }
    if let Ok((at, health, needs, actor, memory, dead, following, inventory)) = players.single() {
        // Who the player is and what they know, once a second, for the
        // `player` op: every thing known (and who told them, if told), and
        // everything they heard said.
        if due(tick.0)
            && let Ok(mut slot) = PLAYER.lock()
        {
            *slot = json!({
                "id": actor.0.0,
                "at": [at.translation.x, at.translation.z],
                "dead": dead,
                "following": following.map(|f| f.0.0),
                // What they carry, as the npc op's "carrying".
                "carrying": inventory.0.slots.iter().flatten().map(|s| json!({"item": s.item, "count": s.count})).collect::<Vec<_>>(),
                // Their walk on the navigation mesh, as the npc op's "way",
                // their commands left, and what they do this tick.
                "way": player_walk.0.single().ok().and_then(|(walker, ..)| crate::navmesh::way_of(walker, &player_walk.1)).map(|(state, target)| json!({
                    "state": format!("{state:?}"),
                    "to": crate::navmesh::going_to(target).map(|p| [p.x, p.y, p.z]),
                })),
                "commands": player_walk.0.single().ok().and_then(|(.., c)| c).map(|c| c.left.iter().map(|c| format!("{c:?}")).collect::<Vec<_>>()),
                "this_tick": player_walk.0.single().ok().map(|(_, t, _)| t.0.iter().map(|s| format!("{:?}", s.action)).collect::<Vec<_>>()),
                // Who killed them last, and of what, from their memory.
                "killed_by": memory.0.killed_by().map(|(by, of)| json!({"by": by.map(|a| a.0), "of": of})),
                "knows": memory.0.known.iter().map(|k| json!({"key": k.key, "kind": k.kind, "told_by": k.told_by.map(|a| a.0)})).collect::<Vec<_>>(),
                "heard": memory.0.done.iter().filter_map(|(t, d)| match d {
                    modforge::memory::Did::Heard(by, said) => Some(json!({"tick": t, "by": by.0, "said": said.words, "line": said.name})),
                    _ => None,
                }).collect::<Vec<_>>(),
                // Who they heard knock on a door, and when.
                "heard_knocking": memory.0.done.iter().filter_map(|(t, d)| match d {
                    modforge::memory::Did::HeardKnocking(by) => Some(json!({"tick": t, "by": by.0})),
                    _ => None,
                }).collect::<Vec<_>>(),
                "said": memory.0.done.iter().filter_map(|(t, d)| match d {
                    modforge::memory::Did::Talked(to, said) => Some(json!({"tick": t, "to": to.0, "said": said.words, "line": said.name})),
                    _ => None,
                }).collect::<Vec<_>>(),
            });
        }
        HEALTH_BITS.store(health.0.current.to_bits(), Ordering::Relaxed);
        HUNGER_BITS.store(needs.0.hunger.to_bits(), Ordering::Relaxed);
        THIRST_BITS.store(needs.0.thirst.to_bits(), Ordering::Relaxed);
        // The ground point, and how high the player's feet are.
        X_BITS.store(at.translation.x.to_bits(), Ordering::Relaxed);
        Y_BITS.store(at.translation.z.to_bits(), Ordering::Relaxed);
        FEET_BITS.store((at.translation.y - crate::player::BODY_CENTRE).to_bits(), Ordering::Relaxed);
    }
    // Where the step's time goes, once a second: the whole step, every
    // part costliest first (a part named "steer: ..." is a piece inside
    // steer), and what was counted.
    if tick.0 % 60 == 0 {
        let parts: Vec<Json> = laps.sorted().into_iter().map(|(n, ms)| json!({"part": n, "ms": ms})).collect();
        let counts: Vec<Json> = laps.counts.iter().map(|(n, v)| json!({"what": n, "per_step": v})).collect();
        // Every span Bevy ran (each system, each schedule, the drawing's
        // work), ms per frame, the costliest first (profile.rs).
        let frame: Vec<Json> = crate::profile::per_frame().into_iter().map(|(n, ms)| json!({"span": n, "ms": ms})).collect();
        if let Ok(mut slot) = PERF.lock() {
            *slot = json!({
                "step_ms": laps.step_ms,
                "budget_ms": 1000.0 / 60.0,
                "people": actors.0.live_count(),
                "parts": parts,
                "counts": counts,
                "frame": frame,
            });
        }
    }
    // Every person, once a second.
    if due(tick.0) {
        let list: Vec<Json> = npcs
            .iter()
            .map(|((actor, npc, at, health, needs, personality, this_tick, name), (memory, record, dead, hotbar, facing, drawn, map, worn))| {
                // What is in their hands, its layers, and the damage its
                // hits land, worked out by the one place a hit reads it.
                let weapon = hotbar.0.held_stack().map(|stack| {
                    let damage = items
                        .0
                        .def(&stack.item)
                        .and_then(|def| modforge::item::numbers_of(def, stack, &layers.0).combat)
                        .map(|c| c.damage);
                    json!({"item": stack.item, "layers": stack.layers, "damage": damage})
                });
                // How many of each kind of thing they did (life.md "What a
                // person did"), and the last one.
                let mut did = serde_json::Map::new();
                // What they ate and drank, by what: a can, a well, water.
                let mut ate_what = serde_json::Map::new();
                for (_, d) in &memory.0.done {
                    let n = did.get(d.kind()).and_then(Json::as_u64).unwrap_or(0);
                    did.insert(d.kind().to_string(), json!(n + 1));
                    if let modforge::memory::Did::Ate(what) = d {
                        let n = ate_what.get(what).and_then(Json::as_u64).unwrap_or(0);
                        ate_what.insert(what.clone(), json!(n + 1));
                    }
                }
                json!({
                    "id": actor.0.0,
                    "name": name.map(|n| n.as_str()),
                    "def": npc.def,
                    "faction": npc.faction,
                    "dead": dead,
                    "at": [at.translation.x, at.translation.z],
                    // How high their feet stand (0 the ground).
                    "feet": crate::interact::feet_of(at).y,
                    "health": health.0.current,
                    "hunger": needs.0.hunger,
                    "thirst": needs.0.thirst,
                    "rest": needs.0.rest,
                    "doing": doing_name(&tree, &record.0),
                    "weapon": weapon,
                    // Where they face on the ground, in radians (0 along +x).
                    "facing": facing.0,
                    // What they wear, each in its equipment slot (one row of
                    // plain slots).
                    "worn": (0..modforge::item::EQUIPMENT_SLOTS)
                        .filter_map(|i| worn.0.get(i).map(|s| json!({"slot": i, "item": s.item})))
                        .collect::<Vec<_>>(),
                    // How many tiles are on the map they carry.
                    "map_tiles": map.map.tiles_seen(),
                    "map_tiles_told": map.told,
                    // What their model shows (only with a window).
                    "drawn": drawn.map(|d| json!({"layers": d.layers, "held": d.held, "walking": d.walking})),
                    "did": did,
                    "ate_what": ate_what,
                    // Every death in their memory: when, of what, by whom.
                    "deaths": memory.0.done.iter().filter_map(|(t, d)| match d {
                        modforge::memory::Did::Died(by, of) => Some(json!({"tick": t, "of": of, "by": by.map(|a| a.0)})),
                        _ => None,
                    }).collect::<Vec<_>>(),
                    "last_did": memory.0.done.last().map(|(t, d)| json!({"tick": t, "did": d.words()})),
                    "killed_by": memory.0.killed_by().map(|(by, of)| json!({"by": by.map(|a| a.0), "of": of})),
                    "alive_for": memory.0.learned.last.map(|(_, secs)| secs),
                    "lives": lives(&memory.0),
                    "learned": memory.0.learned.values.len(),
                    "axes": personality.0.axes,
                    "actions": this_tick.0.iter().map(|s| format!("{:?}", s.action)).collect::<Vec<_>>(),
                })
            })
            .collect();
        if let Ok(mut slot) = PEOPLE_LIST.lock() {
            *slot = Json::Array(list);
        }
    }
    let Some(diagnostics) = diagnostics else {
        return;
    };
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|f| f.smoothed())
        .unwrap_or(0.0) as f32;
    FPS_BITS.store(fps.to_bits(), Ordering::Relaxed);
    if tick.0 % 60 == 0 {
        let map: serde_json::Map<String, Json> = diagnostics
            .iter()
            .filter_map(|d| d.smoothed().map(|v| (d.path().as_str().to_string(), json!(v))))
            .collect();
        if let Ok(mut slot) = DIAGNOSTICS.lock() {
            *slot = Json::Object(map);
        }
    }
}

fn f(bits: &AtomicU32) -> f32 {
    f32::from_bits(bits.load(Ordering::Relaxed))
}

fn snapshot() -> Json {
    json!({
        "tick": TICK.load(Ordering::Relaxed),
        "people": PEOPLE.load(Ordering::Relaxed),
        "player": [f(&X_BITS), f(&Y_BITS)],
        "player_feet": f(&FEET_BITS),
        "health": f(&HEALTH_BITS),
        "hunger": f(&HUNGER_BITS),
        "thirst": f(&THIRST_BITS),
        "fps": f(&FPS_BITS),
        "speed": f(&RUNS_AT_BITS),
    })
}

pub fn start() {
    OP_REGISTRY.register(OpDef::new("status", "current simulation snapshot", "", |_| Ok(snapshot())));
    modforge::test_queue::register_ops();
    OP_REGISTRY.register(OpDef::new(
        "diagnostics",
        "every engine diagnostic: frame time, entity count",
        "",
        |_| Ok(DIAGNOSTICS.lock().map(|d| d.clone()).unwrap_or(Json::Null)),
    ));
    OP_REGISTRY.register(OpDef::new(
        "perf",
        "the whole fixed step's ms against its budget, every part's ms costliest first (pieces inside a system named 'system: piece'), counts per step, and `frame`: every span Bevy ran (each system, each schedule, the drawing's work) in ms per frame, the costliest first",
        "",
        |_| Ok(PERF.lock().map(|p| p.clone()).unwrap_or(Json::Null)),
    ));
    OP_REGISTRY.register(OpDef::new(
        "people",
        "every NPC: id, def, faction, position, health, needs, traits, actions this tick",
        "",
        |_| Ok(PEOPLE_LIST.lock().map(|p| p.clone()).unwrap_or(Json::Null)),
    ));
    OP_REGISTRY.register(OpDef::new(
        "speed",
        "run the game this many times real time (1 is normal); as fast as the machine allows",
        "{\"times\": 20}",
        |args| {
            let times = args.get("times").and_then(Json::as_f64).ok_or_else(|| "times: a number".to_string())? as f32;
            if !(times > 0.0) {
                return Err("times must be above zero".to_string());
            }
            SPEED_BITS.store(times.to_bits(), Ordering::Relaxed);
            Ok(json!({"times": times}))
        },
    ));
    OP_REGISTRY.register(OpDef::new(
        "npc",
        "everything about one NPC by id (see `people`): who, where, health, needs, what the brain has them doing, what they know and carry, their way, and their chunk drawn from the walk grid with how many doorways they can reach",
        "{\"id\": 42}",
        |args| {
            let id = args.get("id").and_then(Json::as_u64).ok_or_else(|| "id: a number".to_string())?;
            if let Ok(mut ask) = NPC_ASK.lock() {
                *ask = Some(id);
            }
            // The game answers on its next step.
            for _ in 0..100 {
                std::thread::sleep(std::time::Duration::from_millis(10));
                if let Ok(mut slot) = NPC_ANSWER.lock()
                    && slot.as_ref().is_some_and(|(asked, _)| *asked == id)
                {
                    return Ok(slot.take().map(|(_, a)| a).unwrap_or(Json::Null));
                }
            }
            Err("the game did not answer within a second".to_string())
        },
    ));
    OP_REGISTRY.register(OpDef::new(
        "walk",
        "the walk grid at a ground point: its chunk drawn (`#` blocked, `.` reachable from the point, `:` open and not reachable, `D` a reachable doorway, `x` one not) and every solid on the tiles within 3 m, named",
        "{\"at\": [412.0, -159.0]}",
        |args| {
            let at = args
                .get("at")
                .and_then(Json::as_array)
                .and_then(|a| Some(Vec2::new(a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32)))
                .ok_or_else(|| "at: [x, y]".to_string())?;
            if let Ok(mut ask) = WALK_ASK.lock() {
                *ask = Some(at);
            }
            // The game answers on its next step.
            for _ in 0..100 {
                std::thread::sleep(std::time::Duration::from_millis(10));
                if let Ok(mut slot) = WALK_ANSWER.lock()
                    && slot.is_some()
                {
                    return Ok(slot.take().unwrap_or(Json::Null));
                }
            }
            Err("the game did not answer within a second".to_string())
        },
    ));
    // Saving and loading (crate::save): one ask at a time, answered once
    // the game has done it.
    let save_or_load = |args: &Json, ask: fn(String) -> crate::save::Ask| -> Result<Json, String> {
        let name = args.get("name").and_then(Json::as_str).ok_or_else(|| "name: the save's name".to_string())?;
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return Err(format!("name {name:?}: letters, digits, _ and - only"));
        }
        if let Ok(mut slot) = crate::save::ANSWER.lock() {
            *slot = None;
        }
        if let Ok(mut slot) = crate::save::ASK.lock() {
            *slot = Some(ask(name.to_string()));
        }
        // Answered once the game has done it, however long that takes: no
        // clock here; the test runner's own limit stops a test that hangs.
        loop {
            std::thread::sleep(std::time::Duration::from_millis(10));
            if let Ok(mut slot) = crate::save::ANSWER.lock()
                && let Some(answer) = slot.take()
            {
                return answer;
            }
        }
    };
    OP_REGISTRY.register(OpDef::new(
        "save",
        "save the game as it is now into saves/<name>.save: every person, thing, door and bullet, the episode, the storm, the reality and the tick; answers with the tick it was saved at",
        "{\"name\": \"the_tap_start\"}",
        move |args| save_or_load(args, crate::save::Ask::Save),
    ));
    OP_REGISTRY.register(OpDef::new(
        "load",
        "load saves/<name>.save into the running game in place of what is there; answers once loaded, with the tick it is at (the saved one)",
        "{\"name\": \"the_tap_start\"}",
        move |args| save_or_load(args, crate::save::Ask::Load),
    ));
    OP_REGISTRY.register(OpDef::new(
        "pause",
        "pause the game (true) or let it run (false); answers once it stands still, with the tick it stands at. While paused, ops act on the next tick `step` runs",
        "{\"on\": true}",
        |args| {
            let on = args.get("on").and_then(Json::as_bool).ok_or_else(|| "on: true or false".to_string())?;
            PAUSED.store(on, Ordering::Relaxed);
            // Answered once the game stands still (or runs), however long a
            // load or a gather holds the frame: no clock here; the test
            // runner's own limit stops a test that hangs.
            loop {
                std::thread::sleep(std::time::Duration::from_millis(10));
                let at = PAUSED_AT.load(Ordering::Relaxed);
                if on && at != u64::MAX {
                    return Ok(json!({"paused": true, "tick": at}));
                }
                if !on && at == u64::MAX {
                    return Ok(json!({"paused": false}));
                }
            }
        },
    ));
    OP_REGISTRY.register(OpDef::new(
        "step",
        "while paused, run exactly this many ticks, as the game always runs them; answers with the moment after them (the tick, the episode, every person and thing), as `save` does",
        "{\"ticks\": 600}",
        |args| {
            let ticks = args.get("ticks").and_then(Json::as_u64).filter(|t| (1..=36_000).contains(t)).ok_or_else(|| "ticks: 1 to 36000".to_string())?;
            if PAUSED_AT.load(Ordering::Relaxed) == u64::MAX {
                return Err("the game is running: pause it first".to_string());
            }
            if let Ok(mut slot) = STEP_ANSWER.lock() {
                *slot = None;
            }
            STEP_ASK.store(ticks, Ordering::Relaxed);
            // Answered once the steps are done: no clock here; the test
            // runner's own limit stops a test that hangs.
            loop {
                std::thread::sleep(std::time::Duration::from_millis(10));
                if let Ok(mut slot) = STEP_ANSWER.lock()
                    && let Some(moment) = slot.take()
                {
                    return Ok(moment);
                }
            }
        },
    ));
    OP_REGISTRY.register(OpDef::new(
        "ways",
        "everyone sent somewhere the navigation mesh gives no way to: who, from, to ([x, height, z]), and why (no way, the place not on the mesh, or they off it)",
        "",
        |_| Ok(WAYS.lock().map(|w| w.clone()).unwrap_or(Json::Null)),
    ));
    OP_REGISTRY.register(OpDef::new(
        "world",
        "the day, the storms, what the people have done, every bunker's store and people",
        "",
        |_| Ok(WORLD.lock().map(|w| w.clone()).unwrap_or(Json::Null)),
    ));
    OP_REGISTRY.register(OpDef::new(
        "view",
        "where the camera is and looks, how many meshes there are by material, and how many the camera sees",
        "",
        |_| Ok(VIEW.lock().map(|v| v.clone()).unwrap_or(Json::Null)),
    ));
    OP_REGISTRY.register(OpDef::new(
        "items",
        "every item lying in the world with layers: its layers, its reach, spread and delay from them, and the models it is drawn with",
        "",
        |_| Ok(ITEMS.lock().map(|v| v.clone()).unwrap_or(Json::Null)),
    ));
    OP_REGISTRY.register(OpDef::new(
        "hud",
        "the inventory screen: whether it is open, each panel of slots with its holder, slot count, and where it stands on the window, and every filled slot with the icons it shows",
        "",
        |_| Ok(HUD.lock().map(|v| v.clone()).unwrap_or(Json::Null)),
    ));
    OP_REGISTRY.register(OpDef::new(
        "storm",
        "bring the next storm now: the sky warns on the game's next step, then the storm hits and the world shifts to the next reality, as any storm does; `reality` names the one it shifts to",
        "{\"reality\": \"Loop\"}",
        |args| {
            let reality = args.get("reality").and_then(Json::as_str).map(str::to_string);
            if let Some(name) = &reality
                && crate::content::RealityRegistryRes::default().0.def(name).is_none()
            {
                return Err(format!("no reality named '{name}'"));
            }
            if let Ok(mut next) = crate::storm::NEXT_REALITY.lock() {
                *next = reality.clone();
            }
            crate::storm::STORM_NOW.store(true, Ordering::Relaxed);
            Ok(json!({"storm": "coming", "reality": reality}))
        },
    ));
    OP_REGISTRY.register(OpDef::new(
        "kill",
        "kill one living person by id (see `people`) on the game's next step, through the one hit path, so a test can bring a death now",
        "{\"id\": 42}",
        |args| {
            let id = args.get("id").and_then(Json::as_u64).ok_or_else(|| "id: a number".to_string())?;
            if let Ok(mut ask) = KILL_ASK.lock() {
                *ask = Some(id);
            }
            Ok(json!({"kill": id}))
        },
    ));
    OP_REGISTRY.register(OpDef::new(
        "needs",
        "set the living person `id`'s `hunger` and `thirst` (0 to 100, 100 full; each only when given) on the game's next step, so a test sets up what someone needs",
        "{\"id\": 3, \"thirst\": 20}",
        |args| {
            let id = args.get("id").and_then(Json::as_u64).ok_or_else(|| "id: a number".to_string())?;
            let hunger = args.get("hunger").and_then(Json::as_f64).map(|v| v as f32);
            let thirst = args.get("thirst").and_then(Json::as_f64).map(|v| v as f32);
            if let Ok(mut ask) = NEEDS_ASK.lock() {
                ask.push((id, hunger, thirst));
            }
            Ok(json!({"needs": id, "hunger": hunger, "thirst": thirst}))
        },
    ));
    OP_REGISTRY.register(OpDef::new(
        "forget",
        "the living person `id` forgets every thing they know (their memory's things: boxes, sources, places) and all they learned (instinct decides again), on the game's next step, so a test sets up what someone knows",
        "{\"id\": 3}",
        |args| {
            let id = args.get("id").and_then(Json::as_u64).ok_or_else(|| "id: a number".to_string())?;
            if let Ok(mut ask) = FORGET_ASK.lock() {
                ask.push(id);
            }
            Ok(json!({"forget": id}))
        },
    ));
    OP_REGISTRY.register(OpDef::new(
        "give",
        "put `count` (1 when left out) of `item` into the inventory of the living person `id` (see `people`) on the game's next step, made as every item is made, so a test sets up what someone carries; `clear` empties their inventory first",
        "{\"id\": 1, \"item\": \"scrap\", \"count\": 20, \"clear\": true}",
        |args| {
            let id = args.get("id").and_then(Json::as_u64).ok_or_else(|| "id: a number".to_string())?;
            let item = args.get("item").and_then(Json::as_str).ok_or_else(|| "item: a name".to_string())?.to_string();
            let count = args.get("count").and_then(Json::as_u64).unwrap_or(1) as u32;
            let clear = args.get("clear").and_then(Json::as_bool).unwrap_or(false);
            if let Ok(mut ask) = GIVE_ASK.lock() {
                ask.push((id, item.clone(), count, clear));
            }
            Ok(json!({"give": id, "item": item, "count": count}))
        },
    ));
    OP_REGISTRY.register(OpDef::new(
        "use",
        "a living person by id (see `people`) stands a metre from the point (`at` on the ground, on the floor at `height`, 0 the ground when left out), or from a person (`person`, by id, where they stand when the game takes it: people keep walking), and uses what is there on the game's next step, through the one interact path, so a test brings a use now",
        "{\"id\": 1, \"person\": 3}",
        |args| {
            let id = args.get("id").and_then(Json::as_u64).ok_or_else(|| "id: a number".to_string())?;
            let at = args
                .get("at")
                .and_then(Json::as_array)
                .and_then(|a| Some(Vec2::new(a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32)));
            let height = args.get("height").and_then(Json::as_f64).unwrap_or(0.0) as f32;
            let to = match (at, args.get("person").and_then(Json::as_u64)) {
                (_, Some(person)) => UseAt::Person(person),
                (Some(at), None) => UseAt::Point(at, height),
                (None, None) => return Err("at: [x, y], or person: an id".to_string()),
            };
            if let Ok(mut ask) = USE_ASK.lock() {
                *ask = Some((id, to));
            }
            Ok(json!({"use": id, "at": at.map(|a| [a.x, a.y]), "person": args.get("person")}))
        },
    ));
    OP_REGISTRY.register(OpDef::new(
        "command",
        "give a person (`id`, the player when left out) a command on the game's next step, or a list of them (`commands`), carried out one after another with the brain's own actions (authority.md \"Commands and replays\"): {\"do\": \"move_to\", \"x\", \"y\", \"z\"} (y the height, as the world's), {\"do\": \"follow\", \"who\"}, {\"do\": \"lead\", \"who\", \"x\", \"y\", \"z\"} (to the point, until both are there),{\"do\": \"talk_to\", \"who\"}, {\"do\": \"knock\", \"who\"} (on the closed door between them), {\"do\": \"attack\", \"who\"}, {\"do\": \"attack_nearest_enemy\"}; a move of their own ends them",
        "{\"do\": \"talk_to\", \"who\": 3}",
        |args| {
            let id = args.get("id").and_then(Json::as_u64);
            let list: Vec<Json> = match args.get("commands").and_then(Json::as_array) {
                Some(list) => list.clone(),
                None => vec![args.clone()],
            };
            let given = list
                .into_iter()
                .map(|c| serde_json::from_value::<modforge::command::Command>(c).map_err(|e| format!("not a command: {e}")))
                .collect::<Result<Vec<_>, _>>()?;
            if let Ok(mut ask) = crate::command::COMMAND_ASK.lock() {
                ask.extend(given.iter().map(|c| (id, *c)));
            }
            Ok(json!({"id": id, "given": given}))
        },
    ));
    OP_REGISTRY.register(OpDef::new(
        "nav_path",
        "the way the navigation mesh gives (docs/pathing.md \"A navigation mesh\") between `from` and `to`, each [x, height, z]: asked, it is answered on the next frame; with no `from`, the last answer (its waypoints, or why there is none, and whether the mesh is built)",
        "{\"from\": [0, 0, 0], \"to\": [0, -6, 0]}",
        |args| {
            let point = |key: &str| {
                args.get(key)
                    .and_then(Json::as_array)
                    .and_then(|a| Some(Vec3::new(a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32)))
            };
            if let (Some(from), Some(to)) = (point("from"), point("to")) {
                if let Ok(mut ask) = crate::navmesh::PATH_ASK.lock() {
                    *ask = Some((from, to));
                }
                return Ok(json!({"asked": [[from.x, from.y, from.z], [to.x, to.y, to.z]]}));
            }
            Ok(crate::navmesh::PATH_ANSWER.lock().map(|a| a.clone()).unwrap_or(Json::Null))
        },
    ));
    OP_REGISTRY.register(OpDef::new(
        "replay",
        "`save`: write every command given this run, with its tick and the world seed, to replays/<name>.json; `play`: give a saved replay's commands again from the game's next step, the same ticks apart; `stop`: stop playing, and every command anyone has left is dropped, and all leading and following ended",
        "{\"save\": \"talk to dell\"}",
        |args| {
            let ask = match (args.get("save").and_then(Json::as_str), args.get("play").and_then(Json::as_str)) {
                (Some(name), _) => crate::command::ReplayAsk::Save(name.to_string()),
                (None, Some(name)) => crate::command::ReplayAsk::Play(name.to_string()),
                (None, None) if args.get("stop").is_some() => crate::command::ReplayAsk::Stop,
                (None, None) => return Err("save: a name, play: a name, or stop".to_string()),
            };
            let path = match &ask {
                crate::command::ReplayAsk::Save(name) | crate::command::ReplayAsk::Play(name) => Some(crate::command::replay_path(name)),
                crate::command::ReplayAsk::Stop => None,
            };
            if let Ok(mut slot) = crate::command::REPLAY_ASK.lock() {
                *slot = Some(ask);
            }
            Ok(json!({"replay": path}))
        },
    ));
    OP_REGISTRY.register(OpDef::new(
        "pointer",
        "drive the mouse as the player does, through Bevy picking on the real mouse pointer: `at` moves it to a window point in logical pixels (the `hud` op gives where things are), `click` presses the left button this frame and releases it the next",
        "{\"at\": [640.0, 360.0], \"click\": true}",
        |args| {
            let at = args.get("at").and_then(Json::as_array).and_then(|a| Some(Vec2::new(a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32)));
            let click = args.get("click").and_then(Json::as_bool).unwrap_or(false);
            if at.is_none() && !click {
                return Err("at: [x, y], click: true, or both".to_string());
            }
            if let Ok(mut queue) = DRIVE.lock() {
                if let Some(at) = at {
                    queue.extend([Drive::Move(at), Drive::Next]);
                }
                if click {
                    queue.extend([Drive::Press, Drive::Next, Drive::Release, Drive::Next]);
                }
            }
            Ok(json!({"at": at.map(|a| [a.x, a.y]), "click": click}))
        },
    ));
    OP_REGISTRY.register(OpDef::new(
        "key",
        "press a key as the player does, taken by the next tick the game runs with the real keys (input::input_to_actions): tapped (pressed for that tick) or, with `hold`, held down until a `key` op with `hold: false` (W, A, S, D, E, I, M, T, Tab, Escape, Enter, Space, 1 to 6)",
        "{\"key\": \"E\"}",
        |args| {
            let name = args.get("key").and_then(Json::as_str).ok_or_else(|| "key: a key's name".to_string())?;
            let code = key_code(name).ok_or_else(|| format!("no key named '{name}'"))?;
            if !crate::input::press_from_op(code, args.get("hold").and_then(Json::as_bool)) {
                return Err(format!("{name} is not a key the bindings know"));
            }
            Ok(json!({"key": name}))
        },
    ));
    OP_REGISTRY.register(OpDef::new(
        "player",
        "the player: their id, every thing they know (key, kind, and who told them, if told), and everything they heard said",
        "",
        |_| Ok(PLAYER.lock().map(|p| p.clone()).unwrap_or(Json::Null)),
    ));
    OP_REGISTRY.register(OpDef::new(
        "talk",
        "one living person by id says something to another (`to`) on the game's next step, through the one talk path: said is {\"Tell\": {\"key\": k}}, {\"Ask\": {\"key\": k}}, {\"Offer\": {\"slot\": n}}, or \"Threaten\"; a thing's key is the one in the `npc` op's knowledge",
        "{\"id\": 3, \"to\": 1, \"said\": {\"Tell\": {\"key\": 42}}}",
        |args| {
            let id = args.get("id").and_then(Json::as_u64).ok_or_else(|| "id: a number".to_string())?;
            let to = args.get("to").and_then(Json::as_u64).ok_or_else(|| "to: a number".to_string())?;
            let said: modforge::actions::Said =
                serde_json::from_value(args.get("said").cloned().unwrap_or(Json::Null)).map_err(|e| format!("said: {e}"))?;
            if let Ok(mut ask) = TALK_ASK.lock() {
                *ask = Some((id, to, said));
            }
            Ok(json!({"talk": id, "to": to, "said": format!("{said:?}")}))
        },
    ));
    OP_REGISTRY.register(OpDef::new(
        "screenshot",
        "save what the game window shows next frame to logs/<name>.png (logs/screenshot.png with no name)",
        "{\"name\": \"talk_open\"}",
        |args| {
            let name = args.get("name").and_then(Json::as_str).unwrap_or("screenshot");
            if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                return Err("name: letters, digits, _ and -".to_string());
            }
            let path = format!("{SCREENSHOT_DIR}/{name}.png");
            if let Ok(mut slot) = SCREENSHOT.lock() {
                *slot = Some(path.clone());
            }
            Ok(json!({"path": path}))
        },
    ));
    modforge::server::spawn(
        modforge::server::Config {
            port: PORT,
            endpoint: "/topside",
            thread_name: "topside-control-plane",
            auth_token: None,
        },
        |body| {
            let req: Json = serde_json::from_str(body).unwrap_or(Json::Null);
            let op = req.get("op").and_then(Json::as_str).unwrap_or_default().to_string();
            let args = req.get("args").cloned().unwrap_or(Json::Null);
            let resp = match OP_REGISTRY.dispatch(&op, &args) {
                Some(r) => OpResponse::from_result(&op, r, snapshot()),
                None => OpResponse::err(&op, format!("unknown op '{op}'"), snapshot()),
            };
            serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec())
        },
        |line| eprintln!("[topside control plane] {line}"),
    );
}
