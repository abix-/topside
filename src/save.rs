//! Saving and loading (todo 31a; Factorio's saves, which its replays and
//! multiplayer depend on): a save is a savestate, everything that changes
//! what happens next, so the same test from the same save does the same
//! thing every time. moonshine-save 0.7 writes the entities marked `Save`
//! (every person: `Actor`; every thing: `Interactable`; bullets in flight)
//! with the components in `saved!`, and the resources in it, as RON through
//! Bevy's reflection, into `saves/<name>.save`; a load despawns them and
//! spawns the saved into the running game. What is worked out from them is
//! made again the same way as the first time, never saved: bodies
//! (`Actor`'s required parts, `interact::item_body`, `interact::door_body`),
//! drawing (the `Added` systems), the walk grid, landmass's stand-ins, the
//! thing index; the actor map's entities are swapped for the new ones
//! (`loaded`), its ids kept.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;
use bevy::world_serialization::WorldFilter;
use moonshine_save::prelude::*;

use crate::SimTick;

/// A saved field written as its JSON text inside the save (`#[serde(with
/// = "crate::save::json_text")]`): for a type whose serde form RON cannot
/// read back, as modforge's `Command`, tagged by "do" for the `command`
/// op's JSON; one form for it everywhere, the one the op reads.
pub mod json_text {
    pub fn serialize<T: serde::Serialize, S: serde::Serializer>(value: &T, to: S) -> Result<S::Ok, S::Error> {
        let text = serde_json::to_string(value).map_err(serde::ser::Error::custom)?;
        to.serialize_str(&text)
    }

    pub fn deserialize<'de, T: serde::de::DeserializeOwned, D: serde::Deserializer<'de>>(from: D) -> Result<T, D::Error> {
        let text = <String as serde::Deserialize>::deserialize(from)?;
        serde_json::from_str(&text).map_err(serde::de::Error::custom)
    }
}

/// Where saves are kept: in the repo, so a run another day starts from
/// the same place (todo 31a).
pub const SAVES: &str = "saves";

/// The file a save of this name is kept in.
pub fn path_of(name: &str) -> std::path::PathBuf {
    std::path::Path::new(SAVES).join(format!("{name}.save"))
}

/// The one list of what is saved: every component that changes what
/// happens next, on the saved entities, and every such resource. Each is
/// registered for reflection and allowed into the save here, once.
macro_rules! saved {
    (components: [$($c:ty),* $(,)?], resources: [$($r:ty),* $(,)?]) => {
        /// Register every saved type.
        fn register(app: &mut App) {
            $(app.register_type::<$c>();)*
            $(app.register_type::<$r>();)*
        }
        /// The saved components.
        fn components() -> WorldFilter {
            WorldFilter::deny_all()$(.allow::<$c>())*
        }
        /// The saved resources.
        fn resources() -> WorldFilter {
            WorldFilter::deny_all()$(.allow::<$r>())*
        }
    };
}

saved! {
    components: [
        Transform,
        Name,
        crate::actor::Actor,
        crate::actor::Npc,
        crate::actor::Look,
        crate::actor::Facing,
        crate::actor::Dead,
        crate::actor::Gone,
        crate::actor::NextThink,
        crate::actor::Inventory,
        crate::actor::Hotbar,
        crate::actor::Personality,
        crate::actor::Traits,
        crate::actor::Memory,
        crate::actor::Map,
        crate::actor::Home,
        crate::actor::Asked,
        crate::actor::SpawnedAt,
        crate::actor::Store,
        crate::actor::Survival,
        crate::actor::ActorCondition,
        crate::actor::ActorRecord,
        crate::ai::Leading,
        crate::ai::Following,
        crate::ai::Holding,
        crate::navmesh::Walking,
        crate::edge::CameFor,
        crate::prisoner::Downed,
        crate::prisoner::LastDowned,
        crate::prisoner::Prisoner,
        crate::building::Planning,
        crate::turret::Turret,
        crate::exposure::Exposure,
        crate::power::Generator,
        crate::combat::Health,
        crate::combat::Protection,
        crate::combat::Equipment,
        crate::combat::Firing,
        crate::combat::Rolling,
        crate::combat::Knocked,
        crate::combat::Bullet,
        crate::command::Commanded,
        crate::input::Actions,
        crate::input::ThisTick,
        crate::interact::Interactable,
        crate::interact::ThingId,
        crate::interact::Door,
        crate::interact::WorldItem,
        crate::interact::Container,
        crate::interact::DrawsFrom,
        crate::interact::Drawn,
        crate::interact::OpenContainer,
        crate::interact::OpenStation,
        crate::interact::Hud,
        crate::model::Model,
        crate::player::Player,
        crate::player::Fall,
        crate::talk::Conversation,
        crate::talk::HeldToTalk,
        crate::hud::HintsDone,
        crate::terrain::WorldPiece,
    ],
    resources: [
        SimTick,
        crate::RunSeed,
        crate::Reality,
        crate::episode::Episode,
        crate::storm::StormRes,
        crate::actor::ActorMapRes,
        crate::sight::BunkerMaps,
        crate::survival::Day,
        crate::command::GivenThisRun,
        crate::command::Playing,
        crate::command::DoneThisRun,
        crate::command::PlayingDone,
        crate::terrain::ThingsMade,
        crate::navmesh::MeshesLanding,
        crate::interact::ThingIssuer,
        crate::edge::EdgeRes,
        crate::randy::RandyRes,
        crate::secrets::SecretsPlaced,
        crate::terrain::SavedChunks,
        crate::building::Builds,
        crate::raid::Raids,
        crate::research::Researched,
    ]
}

/// Saving and loading: the saved types registered, moonshine-save's
/// observers, and ours.
pub fn plugin(app: &mut App) {
    register(app);
    app.add_observer(save_on_default_event);
    app.add_observer(load_on_default_event);
    app.add_observer(saved);
    app.add_observer(loaded);
}

/// What the `save` and `load` ops ask, taken on the game's next step.
pub enum Ask {
    Save(String),
    Load(String),
}

pub static ASK: Mutex<Option<Ask>> = Mutex::new(None);

/// The answer to the last ask: saved, or loaded and at which tick (a load
/// answers only once it has finished, so no test starts on a half-loaded
/// world), or why it failed.
pub static ANSWER: Mutex<Option<Result<serde_json::Value, String>>> = Mutex::new(None);

pub fn answer(result: Result<serde_json::Value, String>) {
    if let Ok(mut slot) = ANSWER.lock() {
        *slot = Some(result);
    }
}

/// What a save says first, on its own line, before the saved world: the
/// world it was made in (the seed, the world's size, how many times the
/// storm has shifted it), and its tick. A RON comment, so the save is still
/// plain RON. Every save carries over (operator, 2026-10-03): there is no
/// version and no content check; content comes from the game, never from
/// the save.
#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
pub struct Header {
    pub seed: u64,
    pub world: String,
    pub shifts: u32,
    pub tick: u64,
}

/// How a save's first line begins.
const HEADER: &str = "// topside save ";

impl Header {
    /// The running game's.
    fn of(world: &World) -> Self {
        Self {
            seed: world.resource::<crate::WorldSeed>().0,
            world: format!("{:?}", world.resource::<crate::content::WorldKind>()).to_lowercase(),
            shifts: world.resource::<crate::storm::StormRes>().shifts,
            tick: world.resource::<SimTick>().0,
        }
    }

    /// Why a save with this header cannot load into the running game, if it
    /// cannot: a world size this game does not know. Another world never
    /// refuses: the load rolls the save's (`roll_saved_world`).
    fn refuses(&self) -> Option<String> {
        if crate::content::WorldKind::from_name(&self.world).is_none() {
            return Some(format!("the save's world is {:?}, not one this game makes (small or big)", self.world));
        }
        None
    }
}

/// The world the save was made in, rolled again whatever world the game
/// is in (a new launch rolls from the clock, a storm shifts it): the seed
/// and size set, this world's pieces gone, and the world rolled through
/// the one roll, as a storm rolls the next (`storm::shift_world`). Its
/// chunks are made again round the loaded people from the next step, their
/// own things only from the save (`terrain::ThingsMade`, loaded after).
/// Saved things are not despawned here: the load unloads them.
fn roll_saved_world(world: &mut World, header: &Header) {
    world.resource_mut::<crate::WorldSeed>().0 = header.seed;
    if let Some(kind) = crate::content::WorldKind::from_name(&header.world) {
        world.insert_resource(kind);
    }
    let pieces: Vec<Entity> = world
        .query_filtered::<Entity, (With<crate::terrain::WorldPiece>, Without<Save>)>()
        .iter(world)
        .collect();
    for piece in pieces {
        if let Ok(piece) = world.get_entity_mut(piece) {
            piece.despawn();
        }
    }
    if let Err(e) = world.run_system_cached(crate::terrain::roll) {
        error!("rolling the saved world: {e}");
    }
}

/// The game's own saves along a run (todo "First: playing the game through
/// replays"): one at the start of the game and one at each step of an
/// episode, so a replay can be played from any point. Asked by `own_save`,
/// made at the next frame's start by `answer_save`.
static OWN_SAVE: AtomicBool = AtomicBool::new(false);

/// While the game makes one of its own saves: `saved` answers no op.
static OWN_SAVING: AtomicBool = AtomicBool::new(false);

/// Ask for one of the game's own saves at the next frame's start.
pub fn own_save() {
    OWN_SAVE.store(true, Ordering::Relaxed);
}

/// What the commands given and the acts done up to a moment hash to: names
/// a run's own save, so a replay finds the saves made on its own line of
/// play.
pub fn commands_hash(given: &[modforge::command::Given], done: &[modforge::command::Done]) -> u32 {
    // FNV-1a over the commands' JSON, the same text a replay file holds,
    // then the acts', when there are any (so a line with none hashes as it
    // did before acts were kept).
    let mut text = serde_json::to_string(given).unwrap_or_default();
    if !done.is_empty() {
        text.push_str(&serde_json::to_string(done).unwrap_or_default());
    }
    text.bytes().fold(0x811c_9dc5u32, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193))
}

/// The name of a run's own save: the run, the tick, and the commands given
/// up to it.
pub fn own_save_name(run: u64, tick: u64, hash: u32) -> String {
    format!("run_{run}_{tick}_{hash:08x}")
}

/// A run's own save's tick and hash, read back from its name.
pub fn own_save_of(name: &str, run: u64) -> Option<(u64, u32)> {
    let rest = name.strip_prefix(&format!("run_{run}_"))?;
    let (tick, hash) = rest.split_once('_')?;
    Some((tick.parse().ok()?, u32::from_str_radix(hash, 16).ok()?))
}

/// Every own save of a run kept in saves/: (tick, hash, name).
pub fn own_saves(run: u64) -> Vec<(u64, u32, String)> {
    let mut found: Vec<_> = std::fs::read_dir(SAVES)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().strip_suffix(".save")?.to_string();
            let (tick, hash) = own_save_of(&name, run)?;
            Some((tick, hash, name))
        })
        .collect();
    found.sort();
    found
}

/// One of the game's own saves made now, unless one of that name is kept
/// already (the same run, tick and commands: the same moment).
fn make_own_save(world: &mut World) {
    let run = world.resource::<crate::RunSeed>().0;
    let tick = world.resource::<SimTick>().0;
    let hash = commands_hash(&world.resource::<crate::command::GivenThisRun>().0, &world.resource::<crate::command::DoneThisRun>().0);
    let name = own_save_name(run, tick, hash);
    if path_of(&name).exists() {
        return;
    }
    OWN_SAVING.store(true, Ordering::Relaxed);
    match start_save(world, &name) {
        Ok(()) => info!("the game saved {name} on its own"),
        Err(e) => warn!("the game's own save {name} not made: {e}"),
    }
    // moonshine-save writes through a queued command: written now, while
    // `saved` still knows the save is the game's own.
    world.flush();
    OWN_SAVING.store(false, Ordering::Relaxed);
}

/// Once a tick: a step of the episode reached is one of the game's own
/// saves, made at the next frame's start. A load that changes the count
/// makes none.
pub fn save_at_steps(episode: Res<crate::episode::Episode>, mut last: Local<Option<(String, usize)>>) {
    let now = (episode.name.clone().unwrap_or_default(), episode.reached.len());
    if let Some((name, count)) = last.as_ref()
        && *name == now.0
        && now.1 > *count
    {
        own_save();
    }
    *last = Some(now);
}

/// Every frame, first, before any other system has queued a command on
/// the world a load replaces (a command left for an entity the load
/// despawned stopped the game): a save or a load asked for is done, and
/// answers from `saved` or `loaded`, or at once with why it cannot be.
pub fn answer_save(world: &mut World) {
    if OWN_SAVE.swap(false, Ordering::Relaxed) {
        make_own_save(world);
    }
    let Some(ask) = ASK.lock().ok().and_then(|mut ask| ask.take()) else {
        return;
    };
    match ask {
        Ask::Save(name) => {
            if let Err(e) = start_save(world, &name) {
                answer(Err(e));
            }
        }
        Ask::Load(name) => match read_save(world, &name) {
            Ok((header, saved)) => {
                // The world is rolled once (authority.md "Running tests"):
                // a save of the world running keeps its ground, buildings
                // and mesh; only a save of another world rolls that one.
                let now = Header::of(world);
                let rolled = (header.seed, &header.world, header.shifts) != (now.seed, &now.world, now.shifts);
                if rolled {
                    roll_saved_world(world, &header);
                }
                world.insert_resource(LoadRolled(rolled, std::time::Instant::now()));
                world.trigger_load(LoadWorld::<moonshine_save::load::DefaultUnloadFilter>::new(LoadInput::World(saved), Default::default()));
            }
            Err(e) => answer(Err(e)),
        },
    }
}

/// The save's file opened with its header line written, and the saved
/// world written after it.
fn start_save(world: &mut World, name: &str) -> Result<(), String> {
    use std::io::Write;
    let path = path_of(name);
    std::fs::create_dir_all(SAVES).map_err(|e| format!("making {SAVES}: {e}"))?;
    let header = serde_json::to_string(&Header::of(world)).map_err(|e| e.to_string())?;
    let mut file = std::fs::File::create(&path).map_err(|e| format!("writing {}: {e}", path.display()))?;
    writeln!(file, "{HEADER}{header}").map_err(|e| format!("writing {}: {e}", path.display()))?;
    // The chunk meshes still building now, and the chunks made, saved with
    // the rest.
    let landing = crate::navmesh::landing_now(world);
    world.insert_resource(landing);
    let made = crate::terrain::chunks_now(world);
    world.insert_resource(made);
    let mut save = SaveWorld::default_into_stream(file);
    save.components = components();
    save.resources = resources();
    world.trigger_save(save);
    Ok(())
}

/// A save's header and saved world, read only if its header says it loads
/// into this game; every way it cannot, said plainly.
fn read_save(world: &mut World, name: &str) -> Result<(Header, bevy::world_serialization::DynamicWorld), String> {
    use serde::de::DeserializeSeed;
    let path = path_of(name);
    let bytes = std::fs::read(&path).map_err(|e| format!("no save at {}: {e}", path.display()))?;
    let first = bytes.split(|b| *b == b'\n').next().unwrap_or_default();
    let header: Header = std::str::from_utf8(first)
        .ok()
        .and_then(|line| line.trim_end().strip_prefix(HEADER))
        .and_then(|json| serde_json::from_str(json).ok())
        .ok_or_else(|| format!("{} has no topside save header: make the save again by playing", path.display()))?;
    if let Some(why) = header.refuses() {
        return Err(format!("{} not loaded: {why}", path.display()));
    }
    let mut assets = world.resource::<AssetServer>().clone();
    let registry = world.resource::<AppTypeRegistry>().clone();
    let registry = registry.read();
    let mut ron = ron::Deserializer::from_bytes(&bytes).map_err(|e| format!("{} not loaded: {e}", path.display()))?;
    let saved = bevy::world_serialization::serde::WorldDeserializer { type_registry: &registry, load_from_path: &mut assets }
        .deserialize(&mut ron)
        .map_err(|e| format!("{} not loaded, it does not read (a saved type changed its fields?): {e}", path.display()))?;
    Ok((header, saved))
}

/// What the game holds at one moment, read the same way when saved and
/// when loaded, so a test compares the two exactly: the tick, the world's
/// seed, the episode,
/// every person by ActorId (where their feet are, health, needs, what they
/// are doing) and every thing by its ThingId (where, what, a door open).
#[derive(bevy::ecs::system::SystemParam)]
pub struct Moment<'w, 's> {
    tick: Res<'w, SimTick>,
    seed: Res<'w, crate::WorldSeed>,
    chunks: Option<Res<'w, crate::terrain::Chunks>>,
    nav: Option<Res<'w, crate::navmesh::Nav>>,
    issuer: Res<'w, crate::interact::ThingIssuer>,
    episode: Res<'w, crate::episode::Episode>,
    people: Query<
        'w,
        's,
        (
            &'static crate::actor::Actor,
            &'static Transform,
            &'static crate::combat::Health,
            &'static crate::actor::Survival,
            Option<&'static crate::actor::ActorRecord>,
        ),
    >,
    things: Query<'w, 's, (&'static crate::interact::ThingId, &'static Transform, Option<&'static crate::interact::WorldItem>, Option<&'static crate::interact::Door>)>,
}

impl Moment<'_, '_> {
    pub fn read(&self) -> serde_json::Value {
        let mut people: Vec<_> = self
            .people
            .iter()
            .map(|(a, t, h, s, r)| {
                let feet = crate::interact::feet_of(t);
                serde_json::json!({
                    "id": a.0.0,
                    "feet": [feet.x, feet.y, feet.z],
                    "health": h.0.current,
                    "needs": [s.0.hunger, s.0.thirst, s.0.rest, s.0.safety],
                    "doing": r.map(|r| format!("{:?}", r.0)),
                })
            })
            .collect();
        people.sort_by_key(|p| p["id"].as_u64());
        let mut things: Vec<_> = self
            .things
            .iter()
            .map(|(id, t, item, door)| {
                serde_json::json!({
                    "id": id.0,
                    "at": [t.translation.x, t.translation.y, t.translation.z],
                    "item": item.map(|i| format!("{} x{}", i.stack.item, i.stack.count)),
                    "open": door.map(|d| d.open),
                })
            })
            .collect();
        things.sort_by_key(|t| t["id"].as_u64());
        // What the chunks and the mesh are at this moment, so two moments
        // that should be one show where the world under people differs.
        let made: std::collections::BTreeSet<_> = self.chunks.as_ref().map(|c| c.made.iter().copied().collect()).unwrap_or_default();
        let made_hash = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            made.hash(&mut h);
            h.finish()
        };
        serde_json::json!({
            "tick": self.tick.0,
            "seed": self.seed.0,
            "chunks_made": [made.len(), made_hash],
            "mesh": self.nav.as_ref().map(|n| n.moment()),
            "next_thing_id": self.issuer.next(),
            "episode": self.episode.name,
            "cast": self.episode.cast.iter().map(|(part, id)| (part.clone(), id.0)).collect::<Vec<_>>(),
            "reached": self.episode.reached.len(),
            "people": people,
            "things": things,
        })
    }
}

/// A save has been written: the `save` op answered with what was saved
/// (`Moment`). A failed save answers nothing (moonshine-save logs why) and
/// the op says the game did not answer.
fn saved(_: On<Saved>, moment: Moment) {
    if OWN_SAVING.load(Ordering::Relaxed) {
        return;
    }
    answer(Ok(moment.read()));
}

/// A load has spawned the save: every person's entity in the actor map
/// swapped for their new one (their ActorId kept); the loading screen
/// started, behind which, without a tick passing, the chunks round the
/// loaded people are made by the chunk systems that make them in play
/// (their own things only from the save, `terrain::ThingsMade`) and every
/// chunk's mesh lands (`loading::ready_the_world`). The `load` op answers
/// with what the game holds (`Moment`), the saved moment, maybe before
/// loading is done (authority.md "Running tests": loading).
fn loaded(loaded: On<Loaded>, mut actors: ResMut<crate::actor::ActorMapRes>, mut commands: Commands) {
    actors.0.map_handles(|e| loaded.entity_map.get(&e).copied());
    commands.queue(move |world: &mut World| {
        let (rolled, start) = world.remove_resource::<LoadRolled>().map_or((false, std::time::Instant::now()), |r| (r.0, r.1));
        // Every person a new entity: the chunks asked round them forgotten,
        // so the first step asks for everything (terrain::Asked).
        crate::terrain::forget_asked(world);
        if !rolled {
            // The same world: the chunks made since the save taken back.
            let saved = world.get_resource::<crate::terrain::SavedChunks>().map(|s| s.0.clone()).unwrap_or_default();
            let since: std::collections::BTreeSet<_> = crate::terrain::chunks_now(world).0.difference(&saved).copied().collect();
            crate::terrain::unmake(world, &since);
            crate::navmesh::mesh_as_saved(world);
        }
        // The rest of the world, its chunks and meshes, made ready behind
        // the loading screen (`loading::ready_the_world`).
        world.insert_resource(crate::loading::Loading::start());
        info!("load done in {:.0} ms ({})", start.elapsed().as_secs_f64() * 1000.0, if rolled { "its world rolled" } else { "the same world" });
        crate::control_plane::LOADED_AT.store(world.resource::<crate::SimTick>().0, std::sync::atomic::Ordering::Relaxed);
        // A stop: what the ops read published and the load answered at
        // this frame's end, once the HUD has drawn the loaded game.
        crate::control_plane::stopped(crate::control_plane::Stop::Load);
    });
}

/// Whether the load under way rolled its world (another world than the
/// running one), and when it began, for `loaded`.
#[derive(Resource)]
struct LoadRolled(bool, std::time::Instant);
