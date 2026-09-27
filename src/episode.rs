//! The episode running now (episodes.md): started by the storm after the
//! reality, its parts cast from the bunkers' people once they exist.

use bevy::prelude::*;
use modforge::actor::ActorId;
use modforge::storyteller::{CastBunker, cast};

use modforge::item;
use modforge::memory::Did;

use crate::actor::{Actor, Dead, Home, Memory, Npc, Store};
use crate::content::{BUNKER_AT, EpisodeRegistryRes, ItemRegistryRes, PLAYER_FACTION};
use crate::interact::{DrawsFrom, spawn_item};

/// The episode running now: one that fits the reality, started by the
/// storm that handed the reality over; its cast and the pivot points
/// reached so far.
#[derive(Resource)]
pub struct Episode {
    /// None when no episode fits the reality.
    pub name: Option<String>,
    /// Each part and the person cast for it.
    pub cast: Vec<(String, ActorId)>,
    /// Whether the cast has been filled for this episode.
    pub cast_done: bool,
    /// Each pivot point reached: its name, who, and when.
    pub reached: Vec<(String, ActorId, u64)>,
    /// What the episode brought into the world, and where.
    pub things: Vec<(String, Vec2)>,
}

impl Episode {
    /// A new episode starts: nobody cast, nothing reached.
    pub fn start(&mut self, name: Option<String>) {
        *self = Self::new(name);
    }

    fn new(name: Option<String>) -> Self {
        Self {
            name,
            cast: Vec::new(),
            cast_done: false,
            reached: Vec::new(),
            things: Vec::new(),
        }
    }

    /// The person cast for a part.
    pub fn cast_as(&self, part: &str) -> Option<ActorId> {
        self.cast.iter().find(|(p, _)| p == part).map(|(_, id)| *id)
    }
}

impl FromWorld for Episode {
    fn from_world(world: &mut World) -> Self {
        let reality = world.get_resource_or_init::<crate::Reality>().0.name.clone();
        let seed = world.resource::<crate::WorldSeed>().0;
        let episodes = world.get_resource_or_init::<EpisodeRegistryRes>();
        Self::new(episodes.0.pick(&reality, seed, 0).map(|e| e.name.clone()))
    }
}

/// Once the bunker people exist, cast the episode's parts from them
/// (modforge `cast`): each bunker's living people, the player's bunker
/// marked, each by its distance from the player's.
pub fn cast_episode(
    mut episode: ResMut<Episode>,
    episodes: Res<EpisodeRegistryRes>,
    seed: Res<crate::WorldSeed>,
    people: Query<(&Actor, &Npc, &Home), Without<Dead>>,
) {
    if episode.cast_done {
        return;
    }
    let Some(def) = episode.name.as_deref().and_then(|n| episodes.0.def(n)) else {
        episode.cast_done = true;
        return;
    };
    if people.is_empty() {
        return;
    }
    let mut bunkers: Vec<(Option<String>, CastBunker<ActorId>)> = Vec::new();
    for (actor, npc, home) in &people {
        let at = bunkers.iter().position(|(f, _)| *f == npc.faction);
        let at = at.unwrap_or_else(|| {
            let player = npc.faction.as_deref() == Some(PLAYER_FACTION);
            bunkers.push((
                npc.faction.clone(),
                CastBunker {
                    player,
                    distance: home.0.distance(BUNKER_AT),
                    people: Vec::new(),
                },
            ));
            bunkers.len() - 1
        });
        bunkers[at].1.people.push(actor.0);
    }
    // The same people in the same order each time, so the seed casts
    // the same way.
    bunkers.sort_by(|a, b| a.0.cmp(&b.0));
    for (_, bunker) in &mut bunkers {
        bunker.people.sort_by_key(|id| id.0);
    }
    let bunkers: Vec<CastBunker<ActorId>> = bunkers.into_iter().map(|(_, b)| b).collect();
    let parts = def.parts.clone();
    episode.cast = cast(&parts, &bunkers, seed.0);
    episode.cast_done = true;
    info!("episode {:?} cast: {:?}", episode.name, episode.cast);
}

/// The Tap, once cast: the tap comes into the world through
/// `interact::spawn_item`, midway between the player's bunker and Mara's,
/// its water drawn from Mara's bunker's store (docs/episodes/the-tap.md).
pub fn place_the_tap(
    mut commands: Commands,
    mut episode: ResMut<Episode>,
    items: Res<ItemRegistryRes>,
    people: Query<(&Actor, &Home, &Store)>,
) {
    if episode.name.as_deref() != Some("The Tap") || !episode.cast_done || !episode.things.is_empty() {
        return;
    }
    let (Some(mara), Some(def)) = (episode.cast_as("Mara"), items.0.def("tap")) else {
        return;
    };
    let Some((_, home, store)) = people.iter().find(|(a, ..)| a.0 == mara) else {
        return;
    };
    let at = (BUNKER_AT + home.0) / 2.0;
    let tap = spawn_item(&mut commands, def, item::create(def, 1, &[], 0.0, 0), at);
    commands.entity(tap).insert(DrawsFrom(store.0));
    episode.things.push(("tap".to_string(), at));
    info!("the tap stands at {at}, drawing from Mara's bunker's store");
}

/// The Tap's first pivot point after it opens: the first drink at the
/// tap, read from the memory of whoever drank (memory is the one record).
pub fn the_first_drink(tick: Res<crate::SimTick>, mut episode: ResMut<Episode>, people: Query<(&Actor, &Memory)>) {
    const FIRST_DRINK: &str = "the first drink";
    // Once a game second is soon enough to notice it.
    if tick.0 % 60 != 0
        || episode.name.as_deref() != Some("The Tap")
        || episode.things.is_empty()
        || episode.reached.iter().any(|(p, ..)| p == FIRST_DRINK)
    {
        return;
    }
    let first = people
        .iter()
        .filter_map(|(actor, memory)| {
            memory
                .0
                .done
                .iter()
                .find(|(_, did)| matches!(did, Did::Ate(item) if item == "tap"))
                .map(|(tick, _)| (actor.0, *tick))
        })
        .min_by_key(|(_, tick)| *tick);
    if let Some((who, tick)) = first {
        episode.reached.push((FIRST_DRINK.to_string(), who, tick));
        info!("The Tap: the first drink, by {who:?} at tick {tick}");
    }
}
