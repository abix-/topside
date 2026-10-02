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

use bevy::prelude::*;
use bevy::world_serialization::WorldFilter;
use moonshine_save::prelude::*;

use crate::SimTick;

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
        crate::combat::Health,
        crate::combat::Protection,
        crate::combat::Equipment,
        crate::combat::Firing,
        crate::combat::Rolling,
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
        crate::Reality,
        crate::episode::Episode,
        crate::storm::StormRes,
        crate::actor::ActorMapRes,
        crate::sight::BunkerMaps,
        crate::survival::Day,
        crate::command::GivenThisRun,
        crate::command::Playing,
        crate::terrain::ThingsMade,
        crate::navmesh::MeshesLanding,
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

fn answer(result: Result<serde_json::Value, String>) {
    if let Ok(mut slot) = ANSWER.lock() {
        *slot = Some(result);
    }
}

/// The save format's version: raised whenever a saved component or
/// resource changes its fields, so an older save is refused plainly
/// instead of failing halfway (todo 31a). Test saves are then made again
/// by playing (9k).
pub const VERSION: u32 = 3;

/// What a save says first, on its own line, before the saved world: the
/// format's version and the world it was made in (the seed, the world's
/// size, how many times the storm has shifted it), and its tick. A RON
/// comment, so the save is still plain RON.
#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
pub struct Header {
    pub version: u32,
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
            version: VERSION,
            seed: world.resource::<crate::WorldSeed>().0,
            world: format!("{:?}", world.resource::<crate::content::WorldKind>()).to_lowercase(),
            shifts: world.resource::<crate::storm::StormRes>().shifts,
            tick: world.resource::<SimTick>().0,
        }
    }

    /// Why a save with this header cannot load into the running game
    /// (`now`), if it cannot: another version, or a world size this game
    /// does not know. Another world never refuses: the load rolls the
    /// save's (`roll_saved_world`).
    fn refuses(&self, now: &Header) -> Option<String> {
        if self.version != now.version {
            return Some(format!("the save is version {}, this game reads version {}: make the save again by playing", self.version, now.version));
        }
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

/// Every frame, first, before any other system has queued a command on
/// the world a load replaces (a command left for an entity the load
/// despawned stopped the game): a save or a load asked for is done, and
/// answers from `saved` or `loaded`, or at once with why it cannot be.
pub fn answer_save(world: &mut World) {
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
                roll_saved_world(world, &header);
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
    // The chunk meshes still building now, saved with the rest.
    let landing = crate::navmesh::landing_now(world);
    world.insert_resource(landing);
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
    if let Some(why) = header.refuses(&Header::of(world)) {
        return Err(format!("{} not loaded: {why}", path.display()));
    }
    let mut assets = world.resource::<AssetServer>().clone();
    let registry = world.resource::<AppTypeRegistry>().clone();
    let registry = registry.read();
    let mut ron = ron::Deserializer::from_bytes(&bytes).map_err(|e| format!("{} not loaded: {e}", path.display()))?;
    let saved = bevy::world_serialization::serde::WorldDeserializer { type_registry: &registry, load_from_path: &mut assets }
        .deserialize(&mut ron)
        .map_err(|e| format!("{} not loaded, it does not read as this version's save (a saved type changed without raising VERSION?): {e}", path.display()))?;
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
        serde_json::json!({
            "tick": self.tick.0,
            "seed": self.seed.0,
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
    answer(Ok(moment.read()));
}

/// A load has spawned the save: every person's entity in the actor map
/// swapped for their new one (their ActorId kept); then, at the load's
/// tick and without a tick passing, the chunks round the loaded people made
/// by the chunk systems that make them in play (their own things only from
/// the save, `terrain::ThingsMade`) and every chunk's mesh landed
/// (`navmesh::mesh_the_loaded_world`); only then the `load` op answered
/// with what the game holds (`Moment`), the saved moment.
fn loaded(loaded: On<Loaded>, mut actors: ResMut<crate::actor::ActorMapRes>, mut commands: Commands) {
    actors.0.map_handles(|e| loaded.entity_map.get(&e).copied());
    commands.queue(|world: &mut World| {
        let _ = world.run_system_cached(crate::terrain::make_chunks);
        let _ = world.run_system_cached(crate::structure::spawn_sites);
        let _ = world.run_system_cached(crate::terrain::open_the_ground);
        let _ = world.run_system_cached(crate::actor::populate_monuments);
        let _ = world.run_system_cached(crate::actor::populate_topside_people);
        crate::navmesh::mesh_the_loaded_world(world);
        answer(world.run_system_cached(|moment: Moment| moment.read()).map_err(|e| format!("reading the loaded game: {e}")));
    });
}
