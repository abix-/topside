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

/// Every step: a save or a load asked for is started, and answers from
/// `saved` or `loaded` once done.
pub fn answer_save(mut commands: Commands) {
    let Some(ask) = ASK.lock().ok().and_then(|mut ask| ask.take()) else {
        return;
    };
    match ask {
        Ask::Save(name) => {
            let mut save = SaveWorld::default_into_file(path_of(&name));
            save.components = components();
            save.resources = resources();
            commands.trigger_save(save);
        }
        Ask::Load(name) => {
            let path = path_of(&name);
            if !path.exists() {
                answer(Err(format!("no save at {}", path.display())));
                return;
            }
            commands.trigger_load(LoadWorld::default_from_file(path));
        }
    }
}

/// What the game holds at one moment, read the same way when saved and
/// when loaded, so a test compares the two exactly: the tick, the episode,
/// every person by ActorId (where their feet are, health, needs, what they
/// are doing) and every thing by its ThingId (where, what, a door open).
#[derive(bevy::ecs::system::SystemParam)]
pub struct Moment<'w, 's> {
    tick: Res<'w, SimTick>,
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
    fn read(&self) -> serde_json::Value {
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

/// A load has finished: every person's entity in the actor map swapped for
/// their new one (their ActorId kept), and the `load` op answered with what
/// the game holds now (`Moment`), the saved moment.
fn loaded(loaded: On<Loaded>, mut actors: ResMut<crate::actor::ActorMapRes>, moment: Moment) {
    actors.0.map_handles(|e| loaded.entity_map.get(&e).copied());
    answer(Ok(moment.read()));
}
