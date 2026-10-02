//! Commands and replays (authority.md "Commands and replays"): plain orders
//! a person carries out with the same actions their brain pushes (modforge
//! `command::step`), given through the control plane's `command` op; every
//! command given this run kept in order, saved and played as a replay
//! (`replays/<name>.json`).

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use bevy::prelude::*;
use modforge::actions::{ActionQueue, Stamped};
use modforge::actor::ActorId;
use modforge::command::{Command, Given, Replay, Step};

use crate::actor::{Actor, Dead, Memory, Npc};
use crate::ai::Following;
use crate::content::{ActorRegistryRes, EpisodeRegistryRes, FactionRegistryRes, PLAYER_FACTION};
use crate::input::{Action, ThisTick};
use crate::player::{Eyes, Player};
use crate::{Ground, SimTick};

/// The commands a person has left, the first being carried out, and
/// whether the first has started (a follow handed over to `Following`).
#[derive(Component, Default)]
pub struct Commanded {
    pub left: VecDeque<Command>,
    pub started: bool,
}

impl Commanded {
    fn next(&mut self) {
        self.left.pop_front();
        self.started = false;
    }
}

/// Every command given this run, in order: what a saved replay holds.
#[derive(Resource, Default)]
pub struct GivenThisRun(pub Vec<Given>);

/// A replay being played: the commands still to give, each at its tick.
#[derive(Resource, Default)]
pub struct Playing(pub VecDeque<Given>);

/// The `command` op's commands (to whom, None the player), taken by
/// `give` on the game's next step.
pub static COMMAND_ASK: Mutex<Vec<(Option<u64>, Command)>> = Mutex::new(Vec::new());

/// The `replay` op's ask: save the commands given this run, or play a saved
/// replay, by name.
pub enum ReplayAsk {
    Save(String),
    Play(String),
}
pub static REPLAY_ASK: Mutex<Option<ReplayAsk>> = Mutex::new(None);

/// Where replays are saved: `replays/<name>.json` in the repo.
pub fn replay_path(name: &str) -> String {
    format!("replays/{name}.json")
}

/// Once a step: a replay saved or played if asked, then every command due
/// (asked through the op, or a playing replay's at its tick) given to its
/// person and kept in this run's list.
pub fn give(
    mut commands: Commands,
    tick: Res<SimTick>,
    seed: Res<crate::WorldSeed>,
    mut given: ResMut<GivenThisRun>,
    mut playing: ResMut<Playing>,
    player: Query<&Actor, With<Player>>,
    mut people: Query<(Entity, &Actor, Option<&mut Commanded>)>,
) {
    if let Some(ask) = REPLAY_ASK.lock().ok().and_then(|mut ask| ask.take()) {
        match ask {
            ReplayAsk::Save(name) => {
                let replay = Replay { seed: seed.0, given: given.0.clone() };
                let saved = std::fs::create_dir_all("replays")
                    .map_err(|e| e.to_string())
                    .and_then(|_| serde_json::to_string_pretty(&replay).map_err(|e| e.to_string()))
                    .and_then(|text| std::fs::write(replay_path(&name), text).map_err(|e| e.to_string()));
                match saved {
                    Ok(()) => info!("replay {name} saved: {} commands", replay.given.len()),
                    Err(e) => warn!("replay {name} not saved: {e}"),
                }
            }
            ReplayAsk::Play(name) => {
                let replay = std::fs::read_to_string(replay_path(&name)).ok().and_then(|text| serde_json::from_str::<Replay>(&text).ok());
                match replay {
                    Some(replay) => {
                        if replay.seed != seed.0 {
                            warn!("replay {name} was saved on world {}, this is world {}: it may not do the same", replay.seed, seed.0);
                        }
                        playing.0 = replay.from_tick(tick.0).into();
                        info!("replay {name} playing: {} commands", playing.0.len());
                    }
                    None => warn!("no replay {name} at {}", replay_path(&name)),
                }
            }
        }
    }
    let the_player = player.single().ok().map(|a| a.0.0);
    let mut due: Vec<(u64, Command)> = COMMAND_ASK
        .lock()
        .map(|mut ask| ask.drain(..).filter_map(|(who, c)| Some((who.or(the_player)?, c))).collect())
        .unwrap_or_default();
    while playing.0.front().is_some_and(|g| g.tick <= tick.0) {
        if let Some(g) = playing.0.pop_front() {
            due.push((g.who, g.command));
        }
    }
    let mut new: HashMap<Entity, VecDeque<Command>> = HashMap::new();
    for (who, command) in due {
        given.0.push(Given { tick: tick.0, who, command });
        match people.iter_mut().find(|(_, a, _)| a.0.0 == who) {
            Some((_, _, Some(mut has))) => has.left.push_back(command),
            Some((entity, _, None)) => new.entry(entity).or_default().push_back(command),
            None => warn!("command {command:?} for {who}: nobody by that id"),
        }
    }
    for (entity, left) in new {
        commands.entity(entity).insert(Commanded { left, started: false });
    }
}

/// Every step, once the queues have drained (as following): each person's
/// first command carried out with the brain's own actions, through modforge
/// `command::step` (a `Go` walked on the navigation mesh, navmesh::walk). A
/// move of their own (the keys) or their death ends their commands.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn carry_out(
    mut commands: Commands,
    tick: Res<SimTick>,
    registry: Res<ActorRegistryRes>,
    factions: Res<FactionRegistryRes>,
    episodes: Res<EpisodeRegistryRes>,
    mut people: Query<(
        (Entity, &Actor, &Transform, &mut ThisTick, &mut Commanded),
        (Has<Following>, Has<Dead>, Option<&crate::interact::Hud>, Option<&crate::talk::LookingAt>, Option<&Children>),
    )>,
    everyone: Query<(Entity, &Actor, &Transform, Option<&Npc>, &Memory, Has<Dead>, Has<Player>)>,
    eyes: Query<&Eyes>,
) {
    let heard = |words: &str| episodes.0.felt(words);
    for ((entity, actor, at, mut this_tick, mut commanded), (following, dead, hud, looking, children)) in &mut people {
        let (x, y) = ActionQueue::movement(&this_tick.0);
        let own_move = (x != 0.0 || y != 0.0) && ActionQueue::aim(&this_tick.0).is_none();
        if dead || own_move {
            commands.entity(entity).remove::<Commanded>();
            continue;
        }
        let here = crate::interact::feet_of(at);
        let alive_at = |who: u64| everyone.iter().find(|(_, a, _, _, _, d, _)| a.0.0 == who && !d).map(|(_, _, t, ..)| crate::interact::feet_of(t));
        loop {
            let Some(command) = commanded.left.front().copied() else {
                commands.entity(entity).remove::<Commanded>();
                break;
            };
            match command {
                // Following does it, until a move of their own or the one
                // followed is gone; then the next.
                Command::Follow { who } => {
                    if !commanded.started {
                        commands.entity(entity).insert(Following(ActorId(who)));
                        commanded.started = true;
                    } else if !following {
                        commanded.next();
                        continue;
                    }
                    break;
                }
                Command::AttackNearestEnemy => {
                    match nearest_enemy(entity, actor.0, here, &everyone, &registry, &factions, heard) {
                        Some(who) => {
                            if let Some(first) = commanded.left.front_mut() {
                                *first = Command::Attack { who };
                            }
                        }
                        None => commanded.next(),
                    }
                    continue;
                }
                // Any other command is their own order: it ends following,
                // as a move of their own does.
                Command::MoveTo { .. } | Command::TalkTo { .. } | Command::Attack { .. } => {
                    if following {
                        commands.entity(entity).remove::<Following>();
                    }
                }
            }
            let (target, who) = match command {
                Command::MoveTo { x, y, z } => (Some(Vec3::new(x, y, z)), None),
                Command::TalkTo { who } | Command::Attack { who } => (alive_at(who), Some(who)),
                Command::Follow { .. } | Command::AttackNearestEnemy => (None, None),
            };
            let talking_with = hud.and_then(|h| h.0.talking.as_ref()).map(|t| t.with);
            let ended = matches!(command, Command::TalkTo { .. }) && talking_with.is_some() && talking_with == who;
            let actions = match modforge::command::step(&command, here, target, ended, crate::talk::TALK_REACH) {
                Step::Done => {
                    commanded.next();
                    continue;
                }
                Step::Act(actions) => actions,
            };
            this_tick.0.retain(|s| !matches!(s.action, Action::Go { .. } | Action::Aim { .. }));
            let looking_at_them = who.is_some() && looking.and_then(|l| l.0).map(|(_, id)| id) == who;
            for action in actions {
                // Use only on them, as E on the one looked at; until then,
                // the eyes turn onto them.
                if matches!(action, Action::Use) && !looking_at_them {
                    let pitch = children.into_iter().flat_map(|c| c.iter()).find_map(|c| eyes.get(c).ok()).map(|e| e.pitch);
                    if let (Some(pitch), Some(there)) = (pitch, target) {
                        // From their eyes to the middle of the one they look at.
                        let up = (there.y + crate::player::BODY_CENTRE) - (here.y + crate::EYE_HEIGHT);
                        let want = up.atan2(there.ground().distance(here.ground()).max(0.1));
                        this_tick.0.push(Stamped { tick: tick.0, action: Action::Look { yaw: 0.0, pitch: want - pitch } });
                    }
                    continue;
                }
                this_tick.0.push(Stamped { tick: tick.0, action });
            }
            break;
        }
    }
}

/// The nearest living person `me` stands hostile to within their sight,
/// by the one rule the brain uses (modforge `relationship::stands`: their
/// own feeling, else their faction's standing, else the faction table).
fn nearest_enemy(
    me: Entity,
    my_id: ActorId,
    here: Vec3,
    everyone: &Query<(Entity, &Actor, &Transform, Option<&Npc>, &Memory, Has<Dead>, Has<Player>)>,
    registry: &ActorRegistryRes,
    factions: &FactionRegistryRes,
    heard: impl Fn(&str) -> f32 + Copy,
) -> Option<u64> {
    let (_, _, _, npc, memory, _, is_player) = everyone.get(me).ok()?;
    let faction_of = |npc: Option<&Npc>, player: bool| if player { Some(PLAYER_FACTION.to_string()) } else { npc.and_then(|n| n.faction.clone()) };
    let mine = faction_of(npc, is_player);
    let def = if is_player { registry.0.def("player") } else { npc.and_then(|n| registry.0.def(&n.def)) }?;
    everyone
        .iter()
        .filter(|(e, a, t, them, _, d, p)| {
            if *e == me || *d || a.0 == my_id || crate::interact::feet_of(t).distance(here) > def.sight {
                return false;
            }
            let own = modforge::relationship::feeling(&memory.0, a.0, heard);
            let feelings = everyone
                .iter()
                .filter(|m| !m.5 && faction_of(m.3, m.6).is_some() && faction_of(m.3, m.6) == mine)
                .map(|m| modforge::relationship::feeling(&m.4.0, a.0, heard));
            let standing = modforge::relationship::standing(feelings);
            let table = factions.0.relation(mine.as_deref(), faction_of(*them, *p).as_deref());
            modforge::relationship::stands(own, standing, table) == modforge::faction::Relation::Hostile
        })
        .min_by(|a, b| crate::interact::feet_of(a.2).distance(here).total_cmp(&crate::interact::feet_of(b.2).distance(here)))
        .map(|(_, a, ..)| a.0.0)
}
