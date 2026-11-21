//! Topside's brain: its one StateTree (modforge docs/statetree.md, life.md
//! "The brain"). modforge runs any tree and holds the rules; which states
//! topside's people have, in what order, is here.
//!
//! The root picks by what the person learned, all of it, the storm and
//! fighting included (life.md "Learning to stay alive"); with nothing
//! learned, instinct's order below decides:
//!   hide from the storm, flee, break off, fight, eat what they carry,
//!   go to a need (going, then using it), trade for a need (going, then
//!   trading), do what was asked (going, then waiting there), supply the
//!   bunker (in order:
//!   haul, fetch, look in a box, head out), look (in order: the nearest
//!   unchecked thing, head out), wander
//! The storm or a threat takes a person out of any errand to choose again.

use bevy::prelude::Resource;
use modforge::brain::{self, Brain, choice, errand, state, when};
use modforge::learn::Choice;
use modforge::statetree::{ROOT, StateId, StateTree};

pub const HIDE: StateId = 1;
pub const FLEE: StateId = 2;
pub const BREAK_OFF: StateId = 3;
pub const FIGHT: StateId = 4;
pub const EAT_CARRIED: StateId = 5;
pub const NEED: StateId = 6;
pub const NEED_GOING: StateId = 7;
pub const NEED_USING: StateId = 8;
pub const TRADE: StateId = 9;
pub const TRADE_GOING: StateId = 10;
pub const TRADE_TRADING: StateId = 11;
pub const ASKED: StateId = 12;
pub const ASKED_GOING: StateId = 13;
pub const ASKED_WAITING: StateId = 14;
pub const SUPPLY: StateId = 15;
pub const HAUL: StateId = 16;
pub const HAUL_GOING: StateId = 17;
pub const HAUL_STOCKING: StateId = 18;
pub const FETCH: StateId = 19;
pub const FETCH_GOING: StateId = 20;
pub const FETCH_TAKING: StateId = 21;
pub const LOOK_IN: StateId = 22;
pub const LOOK_IN_GOING: StateId = 23;
pub const LOOK_IN_CHECKING: StateId = 24;
pub const SUPPLY_HEAD_OUT: StateId = 25;
pub const LOOK: StateId = 26;
pub const LOOK_AT: StateId = 27;
pub const LOOK_AT_GOING: StateId = 28;
pub const LOOK_AT_CHECKING: StateId = 29;
pub const LOOK_HEAD_OUT: StateId = 30;
pub const WANDER: StateId = 31;

/// Topside's tree: modforge's states every game shares (modforge brain
/// "The states every game shares"), with topside's own: hiding from the
/// storm first, and supplying the bunker after what was asked.
pub fn brain_tree() -> StateTree<Brain> {
    let mut s = vec![brain::root()];

    let mut hide = choice(state("hide", Some(ROOT), Some(brain::enter_hide), Some(brain::hide)), Choice::Hide);
    hide.transitions.push(when(brain::storm_over, ROOT));
    s.push(hide);

    brain::fighting_states(&mut s);
    brain::eat_carried_state(&mut s);
    brain::need_states(&mut s);
    // Trade for a need with someone carrying extra of it (design.md
    // "Trading"), after a source they know.
    brain::trade_states(&mut s);
    // Do what they were asked (an episode's errand): go there and wait,
    // after their own needs, before their bunker's.
    brain::asked_states(&mut s);

    let mut supply = choice(state("supply the bunker", Some(ROOT), Some(brain::enter_supply), None), Choice::Supply);
    supply.transitions.push(when(brain::danger, ROOT));
    supply.transitions.push(when(brain::asked_now, ROOT));
    s.push(supply);
    errand(&mut s, "haul home", SUPPLY, brain::enter_haul, "stocking", brain::stock);
    errand(&mut s, "fetch", SUPPLY, brain::enter_fetch, "taking", brain::take);
    errand(&mut s, "look in a box", SUPPLY, brain::enter_look_in, "checking", brain::check);
    let mut head_out = state("head out", Some(SUPPLY), Some(brain::enter_head_out_short), Some(brain::going));
    head_out.transitions.push(when(brain::knows_better, ROOT));
    s.push(head_out);

    brain::look_states(&mut s);
    brain::wander_state(&mut s);

    debug_assert_eq!(s.len(), WANDER + 1, "the ids above name these states");
    StateTree { states: s }
}

/// Topside's tree as an app resource: every NPC thinks through it.
#[derive(Resource)]
pub struct BrainTreeRes(pub StateTree<Brain>);

impl Default for BrainTreeRes {
    fn default() -> Self {
        Self(brain_tree())
    }
}

/// What topside tells the brain that not every game has (modforge
/// `brain::GameFacts`): the storm (design.md "The storm"), their bunker's
/// store and what it is short of, and the water they carry (design.md
/// "Taking loot"). Built by `ai::think` each think.
#[derive(Clone, Debug, Default)]
pub struct TopsideFacts {
    /// The storm is coming or here: hide or die.
    pub storm_coming: bool,
    /// Their bunker's store, if they have one: its key and where.
    pub store: Option<(u64, bevy::math::Vec3)>,
    /// What their bunker is short of, the shortest first.
    pub bunker_short: Vec<modforge::survival::Need>,
    /// They carry food that belongs in the store.
    pub carries_for_bunker: bool,
    /// The water in the containers they carry, and the room left in them,
    /// in litres.
    pub carried_litres: f32,
    pub water_room: f32,
}

impl brain::GameFacts for TopsideFacts {
    fn storm_coming(&self) -> bool {
        self.storm_coming
    }
    fn store(&self) -> Option<(u64, bevy::math::Vec3)> {
        self.store
    }
    fn bunker_short(&self) -> &[modforge::survival::Need] {
        &self.bunker_short
    }
    fn carries_for_bunker(&self) -> bool {
        self.carries_for_bunker
    }
    fn carried_litres(&self) -> f32 {
        self.carried_litres
    }
    fn water_room(&self) -> f32 {
        self.water_room
    }
}

/// Whether they are fleeing, breaking off, or fighting: they react at the
/// combat rate.
pub fn fighting(record: &modforge::statetree::Record) -> bool {
    matches!(record.state, Some(FLEE | BREAK_OFF | FIGHT))
}

#[cfg(test)]
mod tests {
    use super::*;
    use modforge::actions::Action;
    use modforge::actor::{ActorId, Axis, Behaviour, Personality};
    use modforge::brain::{Do, FLEE_FAR, LEASH, Perception, THREAT_RECENT, Think};
    use modforge::memory::{Known, Memory};
    use modforge::monument::Roll;
    use modforge::statetree::{Record, Target};
    use modforge::survival::{Need, SurvivalStats};
    use bevy::math::Vec3;

    /// A storage box is worth 50 for hunger, home 100 for rest, a well 50
    /// for thirst, a wreck nothing.
    fn worth(known: &Known, need: Need) -> f32 {
        match (known.kind.as_str(), need) {
            ("storage box", Need::Hunger) => 50.0,
            ("home", Need::Rest) => 100.0,
            ("well", Need::Thirst) => 50.0,
            _ => 0.0,
        }
    }

    fn perception<'a>(memory: &'a Memory, personality: &'a Personality) -> Perception<'a> {
        Perception {
            now: 100,
            position: Vec3::ZERO,
            yaw: 0.0,
            health_fraction: 1.0,
            needs: SurvivalStats::default(),
            home: Some(Vec3::ZERO),
            behaviour: Behaviour::Hunter,
            at_home: true,
            asleep: false,
            hostile: None,
            game: &(),
            night: false,
            carries_food: false,
            carries_drink: false,
            bags_full: false,
            memory,
            personality,
            worth: &worth,
            standable: &|_| true,
            reachable: &Vec::new,
            unknown: &|_| true,
            asked: None,
            trader: None,
        }
    }

    #[test]
    fn thirsty_with_no_water_known_they_trade_with_someone_who_carries_it() {
        let memory = Memory::default();
        let personality = Personality::default();
        let mut p = perception(&memory, &personality);
        p.needs.thirst = 20.0;
        p.trader = Some((ActorId(7), Vec3::new(4.0, 0.0, 0.0), Need::Thirst));
        let mut record = Record::default();
        let (actions, _) = think(&p, &mut record, &mut Roll::new(1));
        assert_eq!(record.state, Some(TRADE_GOING), "to the one carrying water");
        assert!(has_move(&actions));
        p.position = Vec3::new(3.5, 0.0, 0.0);
        let (_, do_now) = think(&p, &mut record, &mut Roll::new(1));
        assert_eq!(do_now, Some(Do::Trade { with: ActorId(7), need: Need::Thirst }), "there: the trade");
    }

    /// One think on topside's tree: what they do this tick.
    fn think(p: &Perception, record: &mut Record, roll: &mut Roll) -> (Vec<Action>, Option<Do>) {
        let tree = brain_tree();
        let mut t = Think::new(p, roll);
        tree.think(&mut t, record);
        (t.actions, t.do_now)
    }

    /// One think with topside's own facts (the storm, the bunker, water).
    fn think_in(p: &Perception, facts: &TopsideFacts, record: &mut Record, roll: &mut Roll) -> (Vec<Action>, Option<Do>) {
        think(&Perception { game: facts, ..p.clone() }, record, roll)
    }

    /// Sent somewhere: the brain's way of walking, by the way there.
    fn has_move(actions: &[Action]) -> bool {
        actions.iter().any(|a| matches!(a, Action::Go { .. }))
    }

    #[test]
    fn every_state_id_names_its_state() {
        let tree = brain_tree();
        assert_eq!(tree.name(HIDE), "hide");
        assert_eq!(tree.name(FIGHT), "fight");
        assert_eq!(tree.name(NEED_USING), "using it");
        assert_eq!(tree.name(HAUL_STOCKING), "stocking");
        assert_eq!(tree.name(FETCH_TAKING), "taking");
        assert_eq!(tree.name(LOOK_IN_CHECKING), "checking");
        assert_eq!(tree.name(SUPPLY_HEAD_OUT), "head out");
        assert_eq!(tree.name(LOOK_AT_CHECKING), "checking");
        assert_eq!(tree.name(LOOK_HEAD_OUT), "head out");
        assert_eq!(tree.name(WANDER), "wander");
    }

    /// Nothing known to answer a need (theirs or their bunker's): they
    /// head out to find out, as far as something worth stopping for
    /// (150 m), a different way for different people.
    #[test]
    fn short_and_knowing_nothing_they_head_out_to_look() {
        let memory = Memory::default();
        let personality = Personality::default();
        let mut p = perception(&memory, &personality);
        let mut facts = TopsideFacts { store: Some((1, Vec3::ZERO)), bunker_short: vec![Need::Hunger], ..TopsideFacts::default() };
        let to = |p: &Perception, facts: &TopsideFacts, seed: u64| {
            let mut record = Record::default();
            think_in(p, facts, &mut record, &mut Roll::new(seed));
            match record.target {
                Target::Point(to) => to,
                other => panic!("expected to head out, got {:?} {other:?}", record.state),
            }
        };
        let a = to(&p, &facts, 1);
        assert!(a.distance(Vec3::ZERO) >= 100.0, "out past the home ground: {a}");
        assert_ne!(a, to(&p, &facts, 2), "different ways for different rolls");
        // Hungry themselves with nothing known: the same.
        facts = TopsideFacts::default();
        p.needs.hunger = 20.0;
        assert!(to(&p, &facts, 3).distance(Vec3::ZERO) >= 100.0);
    }

    /// The bunker's needs (design.md "Taking loot"): short of food, a
    /// bunker person goes to the box it knows holds food, not the store;
    /// carrying food with the bunker covered, it hauls home; bags full,
    /// it hauls home even while the bunker is short.
    #[test]
    fn a_short_bunker_sends_its_people_for_food_and_they_haul_it_home() {
        let mut memory = Memory::default();
        memory.see(1, "storage box", Vec3::new(2.0, 0.0, 0.0), 1);
        memory.checked(1, vec![], 0.0, 1);
        memory.see(7, "storage box", Vec3::new(40.0, 0.0, 0.0), 1);
        memory.checked(7, vec![("canned food".to_string(), 6)], 0.0, 1);
        let personality = Personality::default();
        let mut p = perception(&memory, &personality);
        // Short of water too, with no water known: the food still sends it.
        let mut facts = TopsideFacts {
            store: Some((1, Vec3::new(2.0, 0.0, 0.0))),
            bunker_short: vec![Need::Thirst, Need::Hunger],
            ..TopsideFacts::default()
        };
        let mut roll = Roll::new(1);
        let mut record = Record::default();
        think_in(&p, &facts, &mut record, &mut roll);
        assert_eq!(record.state, Some(FETCH_GOING));
        assert!(matches!(record.target, Target::Thing { key: 7, need: Some(Need::Hunger), .. }), "{record:?}");
        // There: take.
        p.position = Vec3::new(40.0, 0.0, 0.0);
        let (_, do_now) = think_in(&p, &facts, &mut record, &mut roll);
        assert_eq!(do_now, Some(Do::Take { key: 7 }));
        // Carrying it, the bunker covered: home to the store.
        facts.carries_for_bunker = true;
        facts.bunker_short = vec![];
        let mut record = Record::default();
        think_in(&p, &facts, &mut record, &mut roll);
        assert_eq!(record.state, Some(HAUL_GOING));
        assert!(matches!(record.target, Target::Thing { key: 1, .. }));
        // Still short but the bags are full: home too.
        facts.bunker_short = vec![Need::Hunger];
        p.bags_full = true;
        let mut record = Record::default();
        think_in(&p, &facts, &mut record, &mut roll);
        assert_eq!(record.state, Some(HAUL_GOING));
        // At the store: stock it until nothing is carried.
        p.position = Vec3::new(2.0, 0.0, 0.0);
        let (_, do_now) = think_in(&p, &facts, &mut record, &mut roll);
        assert_eq!(do_now, Some(Do::Stock { key: 1 }));
        facts.carries_for_bunker = false;
        think_in(&p, &facts, &mut record, &mut roll);
        assert_eq!(record.state, None, "stocked: they choose again at the next think");
    }

    /// Short of water and knowing only a well, with nothing to carry water
    /// in: nobody is sent to fetch from it (with room in a container they
    /// are: modforge brain's short_of_water_they_fill_at_a_source_and_pour_it_home).
    #[test]
    fn a_short_bunker_does_not_fetch_from_a_well() {
        let mut memory = Memory::default();
        memory.see(1, "storage box", Vec3::new(2.0, 0.0, 0.0), 1);
        memory.checked(1, vec![], 0.0, 1);
        memory.see(9, "well", Vec3::new(30.0, 0.0, 0.0), 1);
        memory.visited(9, 1);
        let personality = Personality::default();
        let p = perception(&memory, &personality);
        let facts = TopsideFacts { store: Some((1, Vec3::new(2.0, 0.0, 0.0))), bunker_short: vec![Need::Thirst], ..TopsideFacts::default() };
        let mut record = Record::default();
        think_in(&p, &facts, &mut record, &mut Roll::new(1));
        assert_ne!(record.state, Some(FETCH_GOING), "not to the well: {record:?}");
    }

    /// Hungry with a can in the bag: eat it where they stand rather than
    /// walk to the box they know; not hungry, carry it.
    #[test]
    fn a_hungry_person_eats_what_it_carries_first() {
        let mut memory = Memory::default();
        memory.see(7, "storage box", Vec3::new(10.0, 0.0, 0.0), 1);
        memory.checked(7, vec![("canned food".to_string(), 1)], 0.0, 1);
        let personality = Personality::default();
        let mut p = perception(&memory, &personality);
        p.needs.hunger = 20.0;
        p.carries_food = true;
        let (_, do_now) = think(&p, &mut Record::default(), &mut Roll::new(1));
        assert_eq!(do_now, Some(Do::EatCarried { need: Need::Hunger }));
        p.needs.hunger = 80.0;
        let (_, do_now) = think(&p, &mut Record::default(), &mut Roll::new(1));
        assert_ne!(do_now, Some(Do::EatCarried { need: Need::Hunger }), "not hungry, it keeps it");
        p.needs.hunger = 20.0;
        p.carries_food = false;
        let mut record = Record::default();
        think(&p, &mut record, &mut Roll::new(1));
        assert_eq!(record.state, Some(NEED_GOING), "nothing carried: to the box");
        assert!(matches!(record.target, Target::Thing { key: 7, .. }));
    }

    #[test]
    fn a_hunter_goes_after_a_threat_it_remembers_and_forgets_it_later() {
        let mut memory = Memory::default();
        memory.threat(ActorId(7), Vec3::new(0.0, 0.0, 10.0), 90);
        let personality = Personality::default();
        let p = perception(&memory, &personality);
        let mut record = Record::default();
        let (actions, _) = think(&p, &mut record, &mut Roll::new(1));
        assert_eq!(record.state, Some(FIGHT));
        assert!(matches!(record.target, Target::Threat { who: ActorId(7), .. }));
        assert!(actions.contains(&Action::Go { x: 0.0, y: 0.0, z: 10.0 }), "to where it was");
        let mut later = perception(&memory, &personality);
        later.now = 90 + THREAT_RECENT + 1;
        think(&later, &mut record, &mut Roll::new(1));
        assert!(!fighting(&record), "long gone: {record:?}");
    }

    #[test]
    fn a_coming_storm_sends_a_person_home_over_a_fight() {
        let memory = Memory::default();
        let personality = Personality::default();
        let mut p = perception(&memory, &personality);
        p.home = Some(Vec3::new(0.0, 0.0, 30.0));
        p.at_home = false;
        p.hostile = Some((ActorId(4), Vec3::new(2.0, 0.0, 0.0)));
        let storm = TopsideFacts { storm_coming: true, ..TopsideFacts::default() };
        let mut record = Record::default();
        let (actions, _) = think_in(&p, &storm, &mut record, &mut Roll::new(1));
        assert_eq!(record.state, Some(HIDE), "no fighting in a storm");
        assert!(actions.contains(&Action::Go { x: 0.0, y: 0.0, z: 30.0 }), "heading home");
    }

    /// Everything is learnable, the storm too (life.md "Learning to stay
    /// alive"): with nothing learned they hide; someone who learned that
    /// staying out went better than hiding by more than instinct's lead
    /// stays out.
    #[test]
    fn what_they_learned_can_overturn_even_the_storm() {
        let personality = Personality::default();
        let coming = TopsideFacts { storm_coming: true, ..TopsideFacts::default() };
        let storm = |memory: &Memory| {
            let mut p = perception(memory, &personality);
            p.game = &coming;
            p.home = Some(Vec3::new(0.0, 0.0, 30.0));
            p.at_home = false;
            let mut record = Record::default();
            think(&p, &mut record, &mut Roll::new(1));
            (record.state, modforge::brain::situation(&p))
        };
        let fresh = Memory::default();
        let (chose, situation) = storm(&fresh);
        assert_eq!(chose, Some(HIDE), "nothing learned: instinct hides");
        let mut learned = Memory::default();
        learned.learned.values.insert((situation, Choice::Wander), 2.0 * modforge::learn::INSTINCT_STEP);
        let (chose, _) = storm(&learned);
        assert_eq!(chose, Some(WANDER), "learned to stay out");
    }

    #[test]
    fn the_storm_takes_a_person_out_of_an_errand() {
        let mut memory = Memory::default();
        memory.see(9, "well", Vec3::new(0.0, 0.0, -40.0), 1);
        let personality = Personality::default();
        let mut p = perception(&memory, &personality);
        p.needs.thirst = 30.0;
        let mut record = Record::default();
        think(&p, &mut record, &mut Roll::new(1));
        assert_eq!(record.state, Some(NEED_GOING), "to the well");
        let storm = TopsideFacts { storm_coming: true, ..TopsideFacts::default() };
        p.home = Some(Vec3::new(0.0, 0.0, 30.0));
        p.at_home = false;
        let (actions, _) = think_in(&p, &storm, &mut record, &mut Roll::new(1));
        assert_eq!(record.state, Some(HIDE));
        assert!(actions.contains(&Action::Go { x: 0.0, y: 0.0, z: 30.0 }), "home, in the same think");
    }

    /// On the way to the well nothing is chosen again: no stroll spot is
    /// rolled, no other errand weighed, whatever else they come to want.
    #[test]
    fn a_person_walking_to_a_well_keeps_walking() {
        let mut memory = Memory::default();
        memory.see(9, "well", Vec3::new(0.0, 0.0, -40.0), 1);
        memory.see(7, "storage box", Vec3::new(0.0, 0.0, 5.0), 1);
        let personality = Personality::default();
        let mut p = perception(&memory, &personality);
        p.needs.thirst = 30.0;
        let mut record = Record::default();
        think(&p, &mut record, &mut Roll::new(1));
        let going_to_the_well = record;
        assert!(matches!(going_to_the_well.target, Target::Thing { key: 9, .. }));
        // Hungrier now than thirsty, with a box close by, but not
        // critical: still the well.
        p.needs.hunger = 20.0;
        let rolled = std::cell::Cell::new(0);
        let counting = |_: Vec3| {
            rolled.set(rolled.get() + 1);
            true
        };
        p.standable = &counting;
        for step in 1..20 {
            p.position = Vec3::new(0.0, 0.0, -(step as f32));
            let (actions, _) = think(&p, &mut record, &mut Roll::new(step));
            assert_eq!(record, going_to_the_well, "step {step}");
            assert!(has_move(&actions));
        }
        assert_eq!(rolled.get(), 0, "no stroll or trip out was rolled on the way");
    }

    /// On the way to the well, hunger going critical (below
    /// `NEED_CRITICAL`) with a box close by: they leave for the box.
    #[test]
    fn a_person_walking_to_a_well_leaves_for_a_critical_need() {
        let mut memory = Memory::default();
        memory.see(9, "well", Vec3::new(0.0, 0.0, -40.0), 1);
        memory.see(7, "storage box", Vec3::new(0.0, 0.0, 5.0), 1);
        let personality = Personality::default();
        let mut p = perception(&memory, &personality);
        p.needs.thirst = 30.0;
        let mut record = Record::default();
        think(&p, &mut record, &mut Roll::new(1));
        assert!(matches!(record.target, Target::Thing { key: 9, .. }), "to the well");
        p.needs.hunger = modforge::brain::NEED_CRITICAL - 5.0;
        p.position = Vec3::new(0.0, 0.0, -1.0);
        think(&p, &mut record, &mut Roll::new(2));
        assert!(matches!(record.target, Target::Thing { key: 7, need: Some(Need::Hunger), .. }), "to the box: {record:?}");
    }

    #[test]
    fn a_hunter_aims_at_what_it_hunts_and_goes_to_it() {
        let memory = Memory::default();
        let personality = Personality::default();
        let mut p = perception(&memory, &personality);
        p.hostile = Some((ActorId(3), Vec3::new(6.0, 1.5, 8.0)));
        let (actions, _) = think(&p, &mut Record::default(), &mut Roll::new(1));
        assert!(actions.contains(&Action::Aim { x: 6.0, y: 8.0 }), "{actions:?}");
        // To where it stands, its height included.
        assert!(actions.contains(&Action::Go { x: 6.0, y: 1.5, z: 8.0 }), "closes in: {actions:?}");
    }

    #[test]
    fn a_wounded_coward_flees_home_and_a_brave_one_stands() {
        let memory = Memory::default();
        let mut coward = Personality::default();
        coward.axes[Axis::Courage as usize] = -1.0;
        let mut p = perception(&memory, &coward);
        p.home = Some(Vec3::new(0.0, 0.0, 30.0));
        p.at_home = false;
        p.health_fraction = 0.4;
        p.hostile = Some((ActorId(9), Vec3::new(0.0, 0.0, -3.0)));
        let mut record = Record::default();
        let (actions, _) = think(&p, &mut record, &mut Roll::new(1));
        assert_eq!(record.state, Some(FLEE));
        assert!(actions.contains(&Action::Go { x: 0.0, y: 0.0, z: 30.0 }), "runs home");
        assert!(!actions.contains(&Action::Attack));

        let mut brave = Personality::default();
        brave.axes[Axis::Courage as usize] = 1.0;
        let p2 = Perception {
            personality: &brave,
            ..p.clone()
        };
        let mut record = Record::default();
        think(&p2, &mut record, &mut Roll::new(1));
        assert_eq!(record.state, Some(FIGHT), "the brave stand");
    }

    /// Someone with no home never goes home, which never ends (a raider
    /// that fled once stood in a bunker for good, killing whoever woke
    /// there): wounded they run away from the threat, leashed they walk
    /// back to where the fight began.
    #[test]
    fn someone_with_no_home_runs_away_and_never_goes_home() {
        let memory = Memory::default();
        let mut coward = Personality::default();
        coward.axes[Axis::Courage as usize] = -1.0;
        let mut p = perception(&memory, &coward);
        p.home = None;
        p.at_home = false;
        p.health_fraction = 0.4;
        p.hostile = Some((ActorId(9), Vec3::new(0.0, 0.0, -3.0)));
        let mut record = Record::default();
        let (actions, _) = think(&p, &mut record, &mut Roll::new(1));
        assert_eq!(record.state, Some(FLEE));
        assert!(actions.contains(&Action::Go { x: 0.0, y: 0.0, z: FLEE_FAR }), "runs away from the threat: {actions:?}");

        let mut leashed = perception(&memory, &coward);
        leashed.home = None;
        leashed.position = Vec3::new(0.0, 0.0, -(LEASH + 5.0));
        leashed.hostile = Some((ActorId(9), Vec3::new(0.0, 0.0, -(LEASH + 8.0))));
        let mut record = Record {
            state: Some(FIGHT),
            target: Target::Threat { who: ActorId(9), began_at: Vec3::ZERO },
        };
        let (actions, _) = think(&leashed, &mut record, &mut Roll::new(1));
        assert_eq!(record.state, Some(BREAK_OFF));
        assert!(actions.contains(&Action::Go { x: 0.0, y: 0.0, z: 0.0 }), "back to where the fight began");
    }

    #[test]
    fn a_hunter_chased_past_its_leash_goes_home() {
        let memory = Memory::default();
        let personality = Personality::default();
        let mut p = perception(&memory, &personality);
        p.home = Some(Vec3::new(0.0, 0.0, 5.0));
        p.position = Vec3::new(0.0, 0.0, -(LEASH + 5.0));
        p.at_home = false;
        p.hostile = Some((ActorId(9), Vec3::new(0.0, 0.0, -(LEASH + 8.0))));
        let mut record = Record {
            state: Some(FIGHT),
            target: Target::Threat { who: ActorId(9), began_at: Vec3::ZERO },
        };
        let (actions, _) = think(&p, &mut record, &mut Roll::new(1));
        assert_eq!(record.state, Some(BREAK_OFF));
        assert!(actions.contains(&Action::Go { x: 0.0, y: 0.0, z: 5.0 }), "home");
        // Breaking off, the raider still in sight does not start the fight
        // again: they keep walking home.
        let (actions, _) = think(&p, &mut record, &mut Roll::new(1));
        assert_eq!(record.state, Some(BREAK_OFF));
        assert!(!actions.contains(&Action::Attack));
    }

    /// A fight interrupts an errand; after it they choose again (life.md
    /// "The person"): nothing is held underneath to resume.
    #[test]
    fn a_hostile_in_sight_is_fought_and_after_the_fight_they_choose_again() {
        let mut memory = Memory::default();
        memory.see(9, "well", Vec3::new(0.0, 0.0, -40.0), 1);
        let personality = Personality::default();
        let mut p = perception(&memory, &personality);
        let mut record = Record {
            state: Some(WANDER),
            target: Target::Point(Vec3::new(10.0, 0.0, 0.0)),
        };
        p.hostile = Some((ActorId(9), Vec3::new(0.0, 0.0, -1.0)));
        let (actions, _) = think(&p, &mut record, &mut Roll::new(1));
        assert!(actions.contains(&Action::Attack), "in reach: swing");
        assert_eq!(
            record,
            Record {
                state: Some(FIGHT),
                target: Target::Threat { who: ActorId(9), began_at: Vec3::ZERO }
            }
        );
        // A guard never chases, a hunter does.
        p.hostile = Some((ActorId(9), Vec3::new(0.0, 0.0, -10.0)));
        p.behaviour = Behaviour::Guard;
        let (actions, _) = think(&p, &mut record, &mut Roll::new(1));
        assert!(!has_move(&actions));
        p.behaviour = Behaviour::Hunter;
        let (actions, _) = think(&p, &mut record, &mut Roll::new(1));
        assert!(has_move(&actions));
        // Gone and long forgotten: the fight is over, and at the next think
        // they choose again from the root, now for their thirst, not the
        // stroll they were on.
        p.hostile = None;
        p.now = 100_000;
        think(&p, &mut record, &mut Roll::new(1));
        assert_eq!(record.state, None, "the fight ended");
        p.needs.thirst = 30.0;
        think(&p, &mut record, &mut Roll::new(1));
        assert_eq!(record.state, Some(NEED_GOING), "chose again: {record:?}");
    }

    #[test]
    fn a_hungry_person_goes_to_the_remembered_box_and_eats_until_fed() {
        let mut memory = Memory::default();
        memory.see(7, "storage box", Vec3::new(0.0, 0.0, -12.0), 1);
        let personality = Personality::default();
        let mut p = perception(&memory, &personality);
        p.needs.hunger = 30.0;
        let mut roll = Roll::new(1);
        let mut record = Record::default();
        let (actions, _) = think(&p, &mut record, &mut roll);
        assert_eq!(
            record,
            Record {
                state: Some(NEED_GOING),
                target: Target::Thing { key: 7, at: Vec3::new(0.0, 0.0, -12.0), need: Some(Need::Hunger) }
            }
        );
        assert!(has_move(&actions));
        // Arrived: eat, in the same think.
        p.position = Vec3::new(0.0, 0.0, -11.0);
        let (_, do_now) = think(&p, &mut record, &mut roll);
        assert_eq!(do_now, Some(Do::Eat { key: 7, need: Need::Hunger }));
        assert_eq!(record.state, Some(NEED_USING));
        // Still hungry: eat again. Fed: done.
        let (_, do_now) = think(&p, &mut record, &mut roll);
        assert_eq!(do_now, Some(Do::Eat { key: 7, need: Need::Hunger }));
        p.needs.hunger = 95.0;
        let (_, do_now) = think(&p, &mut record, &mut roll);
        assert_eq!(do_now, None);
        assert_eq!(record.state, None);
    }

    /// Drinking at a well for thirst stops when the thirst is met, however
    /// hungry they still are: everything is for a need.
    #[test]
    fn drinking_stops_when_the_thirst_is_met() {
        let mut memory = Memory::default();
        memory.see(9, "well", Vec3::ZERO, 1);
        let personality = Personality::default();
        let mut p = perception(&memory, &personality);
        p.needs.hunger = 20.0;
        p.needs.thirst = 40.0;
        let drinking = Record {
            state: Some(NEED_USING),
            target: Target::Thing { key: 9, at: Vec3::ZERO, need: Some(Need::Thirst) },
        };
        let mut record = drinking;
        let (_, do_now) = think(&p, &mut record, &mut Roll::new(1));
        assert_eq!(do_now, Some(Do::Eat { key: 9, need: Need::Thirst }), "still thirsty: another mouthful");
        p.needs.thirst = 95.0;
        let (_, do_now) = think(&p, &mut record, &mut Roll::new(1));
        assert_eq!(do_now, None, "thirst met: no more drinking, hungry or not");
        assert_ne!(record, drinking);
    }

    #[test]
    fn an_empty_box_sends_a_hungry_person_looking_at_the_nearest_unchecked_thing() {
        let mut memory = Memory::default();
        memory.see(7, "storage box", Vec3::new(0.0, 0.0, -2.0), 1);
        memory.checked(7, vec![], 0.0, 2);
        memory.see(8, "wreck", Vec3::new(40.0, 0.0, 0.0), 1);
        memory.see(9, "wreck", Vec3::new(-20.0, 0.0, 0.0), 1);
        let personality = Personality::default();
        let mut p = perception(&memory, &personality);
        p.needs.hunger = 20.0;
        let mut record = Record::default();
        think(&p, &mut record, &mut Roll::new(1));
        assert_eq!(record.state, Some(LOOK_AT_GOING));
        assert!(matches!(record.target, Target::Thing { key: 9, .. }), "the nearer wreck: {record:?}");
        // Arrived at it: check it.
        p.position = Vec3::new(-20.0, 0.0, 0.0);
        let (_, do_now) = think(&p, &mut record, &mut Roll::new(1));
        assert_eq!(do_now, Some(Do::Check { key: 9 }));
    }

    #[test]
    fn a_tired_person_at_home_sleeps_and_wakes_rested() {
        let mut memory = Memory::default();
        memory.see(1, "home", Vec3::ZERO, 1);
        let personality = Personality::default();
        let mut p = perception(&memory, &personality);
        p.needs.rest = 20.0;
        let mut record = Record::default();
        let (_, do_now) = think(&p, &mut record, &mut Roll::new(1));
        assert_eq!(do_now, Some(Do::Sleep));
        let sleeping = record;
        p.asleep = true;
        p.needs.rest = 60.0;
        let (_, do_now) = think(&p, &mut record, &mut Roll::new(1));
        assert_eq!(record, sleeping, "still asleep");
        assert_eq!(do_now, None);
        p.needs.rest = 96.0;
        let (_, do_now) = think(&p, &mut record, &mut Roll::new(1));
        assert_eq!(do_now, Some(Do::Wake));
        assert_eq!(record.state, None);
    }

    #[test]
    fn idle_people_wander_within_their_home_radius_and_guards_stay_close() {
        let memory = Memory::default();
        let personality = Personality::default();
        let mut p = perception(&memory, &personality);
        let home = Vec3::new(100.0, 0.0, 100.0);
        p.home = Some(home);
        // Spots they can reach every metre around home, out past the
        // widest home radius, so the radius is what keeps them in.
        let spots = || (-60..=60).flat_map(|x| (-60..=60).map(move |z| home + Vec3::new(x as f32, 0.0, z as f32))).collect();
        p.reachable = &spots;
        for behaviour in [Behaviour::Guard, Behaviour::Hunter] {
            p.behaviour = behaviour;
            let mut roll = Roll::new(3);
            let mut farthest: f32 = 0.0;
            let mut strolls = 0;
            for _ in 0..400 {
                let mut record = Record::default();
                let (actions, _) = think(&p, &mut record, &mut roll);
                if let Target::Point(to) = record.target {
                    strolls += 1;
                    farthest = farthest.max(to.distance(p.home.unwrap()));
                    assert!(has_move(&actions));
                }
            }
            assert!(strolls > 10 && strolls < 120, "{behaviour:?}: {strolls} strolls in 400");
            assert!(farthest <= behaviour.home_radius() + 1e-3, "{behaviour:?}: {farthest}");
            assert!(farthest > behaviour.home_radius() * 0.5);
        }
    }

    #[test]
    fn the_same_roll_decides_the_same() {
        let memory = Memory::default();
        let personality = Personality::default();
        let p = perception(&memory, &personality);
        let run = || {
            let mut roll = Roll::new(9);
            let mut record = Record::default();
            (0..50)
                .map(|_| {
                    let out = think(&p, &mut record, &mut roll);
                    (record, out)
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }
}
