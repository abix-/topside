//! Commands and replays (authority.md "Commands and replays"): plain orders
//! a person carries out with the same actions their brain pushes (modforge
//! `command::step`), given through the control plane's `command` op; every
//! command given this run kept in order, saved and played as a replay
//! (`replays/<name>.json`).

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use bevy::prelude::*;
use modforge::actions::{ActionQueue, Stamped};
use modforge::actor::ActorId;
use modforge::command::{Act, Command, Done, Given, Replay, Step};
use modforge::memory::Did;

use crate::actor::{Actor, Dead, Memory, Npc};
use crate::ai::{Following, Leading};
use crate::content::{ActorRegistryRes, EpisodeRegistryRes, FactionRegistryRes, PLAYER_FACTION};
use crate::input::{Action, ThisTick};
use crate::player::{Eyes, Player};
use crate::{Ground, SimTick};

/// The commands a person has left, the first being carried out, and
/// whether the first has started (a follow handed over to `Following`).
#[derive(Component, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Commanded {
    #[serde(with = "crate::save::json_text")]
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
#[derive(Resource, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct GivenThisRun(#[serde(with = "crate::save::json_text")] pub Vec<Given>);

/// A replay being played: the commands still to give, each at its tick.
#[derive(Resource, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct Playing(#[serde(with = "crate::save::json_text")] pub VecDeque<Given>);

/// Every act done this run (the `use` and `talk` ops'), in order: kept in
/// a saved replay with the commands.
#[derive(Resource, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct DoneThisRun(#[serde(with = "crate::save::json_text")] pub Vec<Done>);

/// A replay being played: the acts still to do, each at its tick.
#[derive(Resource, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct PlayingDone(#[serde(with = "crate::save::json_text")] pub VecDeque<Done>);

/// The `command` op's commands (to whom, None the player), taken by
/// `give` on the game's next step.
pub static COMMAND_ASK: Mutex<Vec<(Option<u64>, Command)>> = Mutex::new(Vec::new());

/// The `use` and `talk` ops' acts (by whom), done by
/// control_plane::answer_acts on the game's next step.
pub static ACT_ASK: Mutex<Vec<(u64, Act)>> = Mutex::new(Vec::new());

/// The `replay` op's ask: save the commands given this run, or play a saved
/// replay, by name.
pub enum ReplayAsk {
    Save(String),
    Play(String),
    /// Stop playing, and drop every command anyone has left.
    Stop,
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
    seed: Res<crate::RunSeed>,
    mut given: ResMut<GivenThisRun>,
    mut playing: ResMut<Playing>,
    done: Res<DoneThisRun>,
    mut playing_done: ResMut<PlayingDone>,
    player: Query<&Actor, With<Player>>,
    mut people: Query<(Entity, &Actor, Option<&mut Commanded>)>,
    going_with: Query<Entity, Or<(With<Leading>, With<Following>, With<crate::ai::Holding>)>>,
) {
    if let Some(ask) = REPLAY_ASK.lock().ok().and_then(|mut ask| ask.take()) {
        match ask {
            ReplayAsk::Save(name) => {
                let replay = Replay { seed: seed.0, given: given.0.clone(), done: done.0.clone() };
                let saved = std::fs::create_dir_all("replays")
                    .map_err(|e| e.to_string())
                    .and_then(|_| serde_json::to_string_pretty(&replay).map_err(|e| e.to_string()))
                    .and_then(|text| std::fs::write(replay_path(&name), text).map_err(|e| e.to_string()));
                match saved {
                    Ok(()) => info!("replay {name} saved: {} commands, {} acts", replay.given.len(), replay.done.len()),
                    Err(e) => warn!("replay {name} not saved: {e}"),
                }
            }
            ReplayAsk::Play(name) => {
                let replay = std::fs::read_to_string(replay_path(&name)).ok().and_then(|text| serde_json::from_str::<Replay>(&text).ok());
                match replay {
                    Some(replay) => {
                        if replay.seed != seed.0 {
                            warn!("replay {name} was saved in run {}, this is run {}: it may not do the same", replay.seed, seed.0);
                        }
                        // Each command at its own tick, from this step on: the
                        // ones before were given before the save loaded, which
                        // holds them.
                        playing.0 = replay.from_tick(tick.0).into();
                        playing_done.0 = replay.done_from(tick.0).into();
                        info!(
                            "replay {name} playing from tick {}: {} commands to give and {} acts to do, {} and {} before it in the save",
                            tick.0,
                            playing.0.len(),
                            playing_done.0.len(),
                            replay.given.len() - playing.0.len(),
                            replay.done.len() - playing_done.0.len()
                        );
                    }
                    None => warn!("no replay {name} at {}", replay_path(&name)),
                }
            }
            ReplayAsk::Stop => {
                playing.0.clear();
                playing_done.0.clear();
                // Emptied now, not removed at the step's end: a command given
                // in this same step goes on after the stop, not with it.
                for (_, _, has) in &mut people {
                    if let Some(mut has) = has {
                        has.left.clear();
                        has.started = false;
                    }
                }
                // And the leading, following and holding commands started.
                for entity in &going_with {
                    commands.entity(entity).remove::<(Leading, Following, crate::ai::Holding)>();
                }
                info!("replay stopped: every command left dropped");
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
        order(&mut people, &mut new, who, command);
    }
    for (entity, left) in new {
        commands.entity(entity).insert(Commanded { left, started: false });
    }
    PLAYING_LEFT.store(playing.0.len() as u64, Ordering::Relaxed);
    ACTS_LEFT.store(playing_done.0.len() as u64, Ordering::Relaxed);
    DONE_SO_FAR.store(done.0.len() as u64, Ordering::Relaxed);
    PLAYING_NEXT.store(playing.0.front().map_or(u64::MAX, |g| g.tick), Ordering::Relaxed);
    GIVEN_SO_FAR.store(given.0.len() as u64, Ordering::Relaxed);
    RUN.store(seed.0, Ordering::Relaxed);
}

/// What the `replay` op reports, written each step by `give`: the commands
/// a playing replay has left, the tick of the next (`u64::MAX` for none),
/// the commands given this run so far, and the run.
pub static PLAYING_LEFT: AtomicU64 = AtomicU64::new(0);
pub static PLAYING_NEXT: AtomicU64 = AtomicU64::new(u64::MAX);
pub static GIVEN_SO_FAR: AtomicU64 = AtomicU64::new(0);
/// And the acts: a playing replay's left to do, and those done so far.
pub static ACTS_LEFT: AtomicU64 = AtomicU64::new(0);
pub static DONE_SO_FAR: AtomicU64 = AtomicU64::new(0);
pub static RUN: AtomicU64 = AtomicU64::new(0);

/// The one way an order reaches a person: onto the end of their commands
/// left, or, with none, into `new`, the commands to give them this step
/// (the caller inserts each as their `Commanded`). A command given
/// (`give`) and an episode's cue (episode::cues) alike.
pub fn order(people: &mut Query<(Entity, &Actor, Option<&mut Commanded>)>, new: &mut HashMap<Entity, VecDeque<Command>>, who: u64, command: Command) {
    match people.iter_mut().find(|(_, a, _)| a.0.0 == who) {
        Some((_, _, Some(mut has))) => has.left.push_back(command),
        Some((entity, _, None)) => new.entry(entity).or_default().push_back(command),
        None => warn!("command {command:?} for {who}: nobody by that id"),
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
    ways: crate::navmesh::Ways,
    mut people: Query<(
        (Entity, &Actor, &Transform, &mut ThisTick, &mut Commanded, Option<&crate::navmesh::Walker>),
        (
            Has<Following>,
            Has<Dead>,
            Option<&crate::interact::Hud>,
            Option<&crate::talk::LookingAt>,
            Option<&Children>,
            Has<crate::ai::Leading>,
            Has<crate::ai::Holding>,
            Has<crate::player::Player>,
        ),
    )>,
    everyone: Query<(Entity, &Actor, &Transform, Option<&Npc>, &Memory, Has<Dead>, Has<Player>)>,
    eyes: Query<&Eyes>,
    doors: Query<(&crate::interact::Door, &Transform)>,
    nav: Res<crate::navmesh::Nav>,
    archipelagos: Query<&bevy_landmass::prelude::Archipelago3d>,
    talks: Query<(&Actor, &crate::talk::Conversation)>,
) {
    let archipelago = archipelagos.get(nav.archipelago).ok();
    let on_mesh = |at: Vec3| archipelago.and_then(|a| crate::navmesh::on_mesh(a, at));
    let heard = |words: &str| episodes.felt(words);
    for ((entity, actor, at, mut this_tick, mut commanded, walker), (following, dead, hud, looking, children, leading, holding, is_player)) in &mut people {
        // A move of their own is the player's keys (a move with no aim);
        // anyone else's moves come from the walk, never their own.
        let (x, y) = ActionQueue::movement(&this_tick.0);
        let own_move = is_player && (x != 0.0 || y != 0.0) && ActionQueue::aim(&this_tick.0).is_none();
        if dead || own_move {
            if own_move && let Some(command) = commanded.left.front() {
                remember(&mut commands, entity, Did::Failed(words_of(command), "stopped by a move of their own".to_string()), tick.0);
            }
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
                // Leading does it, until both are there (ai::lead); then the
                // next.
                Command::Lead { who, to } => {
                    if !commanded.started {
                        commands.entity(entity).insert(Leading { who: ActorId(who), to: to.into(), waiting_at: None });
                        commanded.started = true;
                    } else if !leading {
                        commanded.next();
                        continue;
                    }
                    break;
                }
                // Holding does it (the brain, ai::Holding), until something
                // takes it off them; then the next.
                Command::Hold { at } => {
                    if !commanded.started {
                        commands.entity(entity).insert(crate::ai::Holding { at: at.into(), warned: Vec::new() });
                        // Whatever way they were on, they stop where they
                        // stand (no Go, and no move a walk left held), and
                        // their brain drops what it was doing: it takes the
                        // hold at its next decision.
                        this_tick.0.retain(|s| !matches!(s.action, Action::Go { .. } | Action::Move { .. } | Action::Aim { .. }));
                        this_tick.0.push(Stamped { tick: tick.0, action: Action::Move { x: 0.0, y: 0.0 } });
                        commands.entity(entity).insert(crate::actor::ActorRecord(Default::default()));
                        commanded.started = true;
                    } else if !holding {
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
                // as a move of their own does, and holding.
                Command::MoveTo { .. } | Command::TalkTo { .. } | Command::Knock { .. } | Command::Attack { .. } => {
                    if following {
                        commands.entity(entity).remove::<Following>();
                    }
                    if holding {
                        commands.entity(entity).remove::<crate::ai::Holding>();
                    }
                }
            }
            let (target, who) = match command {
                Command::MoveTo { to } => (Some(to.into()), None),
                Command::TalkTo { who } | Command::Attack { who } => (alive_at(who), Some(who)),
                // The spot beside the closed door between them.
                // Knocked, they wait for an answer: at the knock spot while
                // the door is shut, coming up to them once it stands open
                // (the step below).
                Command::Knock { who } if commanded.started => (alive_at(who), Some(who)),
                Command::Knock { who } => (alive_at(who).and_then(|them| crate::interact::knock_spot(doors.iter(), them, on_mesh)), Some(who)),
                Command::Follow { .. } | Command::Lead { .. } | Command::Hold { .. } | Command::AttackNearestEnemy => (None, None),
            };
            let talking_with = hud.and_then(|h| h.0.talking.as_ref()).map(|t| t.with);
            // Talking with them; or (a knock) answered: a talk between the
            // two has started (orchestrator, 2026-10-03: someone who knocks
            // waits to be answered).
            let in_a_talk = |a: u64, b: u64| talks.iter().any(|(actor, talk)| (actor.0.0 == a && talk.with == b) || (actor.0.0 == b && talk.with == a));
            let ended = match command {
                Command::TalkTo { .. } => talking_with.is_some() && talking_with == who,
                Command::Knock { who } => commanded.started && in_a_talk(actor.0.0, who),
                _ => false,
            };
            let looking_at_them = who.is_some() && looking.and_then(|l| l.0).map(|(_, id)| id) == who;
            // A move to a point the navigation mesh gives no way to stops,
            // and why is remembered: landmass, walking them to that very
            // point, found no way (todo: write every act's result into
            // memory, 2026-11-19).
            if let Command::MoveTo { to } = command
                && let Some((state, going)) = crate::navmesh::way_of(walker, &ways)
                && crate::navmesh::going_to(going).is_some_and(|p| p.distance(to.into()) < 1.0)
                && let Some(why) = match state {
                    bevy_landmass::AgentState::NoPath => Some("no way there"),
                    bevy_landmass::AgentState::TargetNotOnNavMesh => Some("the place is not on the navigation mesh"),
                    bevy_landmass::AgentState::AgentNotOnNavMesh => Some("they are not on the navigation mesh"),
                    _ => None,
                }
            {
                remember(&mut commands, entity, Did::Failed(words_of(&command), why.to_string()), tick.0);
                this_tick.0.retain(|s| !matches!(s.action, Action::Go { .. } | Action::Aim { .. }));
                commanded.next();
                continue;
            }
            let actions = match modforge::command::step(&command, here, target, ended, looking_at_them, crate::talk::TALK_REACH) {
                Step::Done => {
                    if let Command::MoveTo { .. } = command {
                        remember(&mut commands, entity, Did::Done(format!("{}: there", words_of(&command))), tick.0);
                    }
                    // Answered, the knocker stays for the talk that began
                    // (talk::hold, as the talk holds anyone it is with), not
                    // off on their own the moment their knock is done.
                    if matches!(command, Command::Knock { .. }) && ended {
                        crate::talk::hold(&mut commands, entity, here);
                    }
                    commanded.next();
                    continue;
                }
                // Knocked once, they stand at the knock spot while the door
                // between stays shut; once it is open (no shut door left to
                // knock at), they come within talking reach of them.
                Step::Act(_) if matches!(command, Command::Knock { .. }) && commanded.started => {
                    let door_open = target.is_some_and(|them| crate::interact::knock_spot(doors.iter(), them, on_mesh).is_none());
                    match target {
                        Some(them) if door_open && here.distance(them) > crate::talk::TALK_REACH * 0.8 => modforge::brain::walk_toward(here, them),
                        _ => Vec::new(),
                    }
                }
                Step::Act(actions) => actions,
            };
            this_tick.0.retain(|s| !matches!(s.action, Action::Go { .. } | Action::Aim { .. }));
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
                if matches!(action, Action::Knock) {
                    commanded.started = true;
                }
                this_tick.0.push(Stamped { tick: tick.0, action });
            }
            break;
        }
    }
}

/// Note what a command came to in that person's memory, the one record of
/// what anyone did, at the end of this step (the carrying out reads every
/// memory while it runs).
fn remember(commands: &mut Commands, who: Entity, did: Did, tick: u64) {
    commands.entity(who).queue(move |mut person: EntityWorldMut| {
        if let Some(mut memory) = person.get_mut::<Memory>() {
            memory.0.did(did, tick);
        }
    });
}

/// A command in words, as memory keeps it: "move to 3.0, 0.0, 5.0".
fn words_of(command: &Command) -> String {
    match command {
        Command::MoveTo { to } => format!("move to {:.1}, {:.1}, {:.1}", to.x, to.y, to.z),
        Command::Follow { who } => format!("follow {who}"),
        Command::Lead { who, .. } => format!("lead {who}"),
        Command::Hold { .. } => "hold".to_string(),
        Command::TalkTo { who } => format!("talk to {who}"),
        Command::Knock { who } => format!("knock for {who}"),
        Command::Attack { who } => format!("attack {who}"),
        Command::AttackNearestEnemy => "attack the nearest enemy".to_string(),
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
            let members = everyone.iter().filter(|m| !m.5 && faction_of(m.3, m.6).is_some() && faction_of(m.3, m.6) == mine).map(|m| m.4);
            crate::actor::stands_toward((memory, mine.as_deref()), members, (a.0, faction_of(*them, *p).as_deref()), &factions.0, heard)
                == modforge::faction::Relation::Hostile
        })
        .min_by(|a, b| crate::interact::feet_of(a.2).distance(here).total_cmp(&crate::interact::feet_of(b.2).distance(here)))
        .map(|(_, a, ..)| a.0.0)
}
