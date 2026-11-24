//! The one interact path: a person's Use this tick reaches the nearest
//! interactable thing in reach, modforge's `hud::interact` decides
//! what happens, and topside carries out the engine side.

use std::f32::consts::FRAC_PI_2;

use avian3d::prelude::*;
use bevy::prelude::*;
use modforge::crafting::StationKind;
use modforge::hud::{HudState, InteractKind, InteractResult, OpenPanel, interact};
use modforge::item::{self, ItemDef, ItemKind, ItemStack};
use modforge::memory::Did;

use modforge::walk::centre;

use crate::actor::{Dead, Facing, Inventory, Persona};
use crate::model::Model;
use crate::{Ground, on_ground};
use crate::combat::Health;
use crate::content::ItemRegistryRes;
use crate::input::{Action, ActionQueue, ThisTick};

/// How far a person reaches to use something, in metres.
pub const REACH: f32 = 1.5;

/// Something a person can use, and what kind of thing it is: an item, a
/// station, a door. Saved.
#[derive(Component, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
#[require(moonshine_save::prelude::Save)]
pub struct Interactable {
    pub kind: InteractKind,
}

/// A thing's own id, lasting across save and load as a person's ActorId
/// does (an entity's id changes on every load): what memory knows a thing
/// by. Given once, when the thing comes into being (`ThingIssuer::issue`,
/// at the three places usable things are made: `spawn_item`,
/// `spawn_station`, a building's doors).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct ThingId(pub u64);

/// The one issuer of thing ids, as `ActorMapRes` issues ActorIds: the next
/// id to give. Saved, so a load gives on from exactly where the saved game
/// would have.
#[derive(Resource, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct ThingIssuer(u64);

impl Default for ThingIssuer {
    fn default() -> Self {
        Self(1)
    }
}

impl ThingIssuer {
    /// The id it gives next.
    pub fn next(&self) -> u64 {
        self.0
    }

    /// The one place a thing id is given.
    pub fn issue(&mut self) -> ThingId {
        let id = ThingId(self.0);
        self.0 += 1;
        id
    }
}

/// Every thing by its id: worked out from the things, never saved.
#[derive(Resource, Default)]
pub struct ThingIds {
    by_id: std::collections::HashMap<u64, Entity>,
}

impl ThingIds {
    /// The thing with this id, if it is in the world.
    pub fn get(&self, id: u64) -> Option<Entity> {
        self.by_id.get(&id).copied()
    }
}

/// The index follows the ids: a thing with an id is found by it, one gone
/// is not.
pub fn index_thing_ids(add: On<Add, ThingId>, things: Query<&ThingId>, mut ids: ResMut<ThingIds>) {
    if let Ok(id) = things.get(add.entity) {
        ids.by_id.insert(id.0, add.entity);
    }
}

pub fn unindex_thing_ids(remove: On<Remove, ThingId>, things: Query<&ThingId>, mut ids: ResMut<ThingIds>) {
    if let Ok(id) = things.get(remove.entity) {
        ids.by_id.remove(&id.0);
    }
}

/// A door, its own place its hinge (structure::spawn_structure): open, it
/// turns a quarter turn about the hinge and stops nothing. Where it stands
/// closed, its middle in the doorway, which way it faces closed, how wide
/// its doorway is, and its panel's model; its body is made from these
/// (`door_body`).
#[derive(Component, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Door {
    pub open: bool,
    pub closed_center: Vec2,
    pub hinge: Vec2,
    pub closed: Quat,
    pub width: f32,
    /// How far its panel reaches along its hinge: a doorway's height, or
    /// a hatch's length.
    pub height: f32,
    /// A hatch: lying in a floor over a way down (modforge `Hatch`), its
    /// two sides above and below it.
    pub lying: bool,
    pub model: String,
}

/// How thick a door is, in metres (scripts/models.py's panel, made 1 m
/// wide and a wall tall).
const DOOR_THICK: f32 = 0.12;

/// A door's body, made whenever a door comes into the world, built or
/// loaded: solid from the hinge across the doorway (no wall to the
/// navigation mesh: people open doors), and its panel, a child stretched
/// to the doorway so the collider is not stretched with it; open, it
/// stops nothing.
pub fn door_body(add: On<Add, Door>, doors: Query<&Door>, mut commands: Commands) {
    let Ok(door) = doors.get(add.entity) else {
        return;
    };
    let width = door.width;
    let mut body = commands.entity(add.entity);
    body.insert((
        RigidBody::Static,
        avian_rerecast::prelude::ExcludeColliderFromNavmesh,
        Collider::compound(vec![(
            Vec3::new(width / 2.0, door.height / 2.0, 0.0),
            Rotation::IDENTITY,
            Collider::cuboid(width, door.height, DOOR_THICK),
        )]),
        Visibility::default(),
    ));
    body.with_child((
        Model(door.model.clone()),
        Transform::from_xyz(width / 2.0, 0.0, 0.0).with_scale(Vec3::new(width, door.height / crate::structure::WALL_HEIGHT, 1.0)),
    ));
    if door.open {
        body.insert(ColliderDisabled);
    }
}

impl Door {
    /// The turn that lays a door from `hinge` across to `centre`: its panel
    /// reaches along its own +x.
    pub fn facing(hinge: Vec2, centre: Vec2) -> Quat {
        let d = centre - hinge;
        Quat::from_rotation_y((-d.y).atan2(d.x))
    }

    /// A hatch over the hole of modforge `hatch`, its building at `at`: its
    /// panel lies flat, reaching across the hole from the hinge edge along
    /// its own +x and along the edge along its own +y, its own +z down, so
    /// the same quarter turn about its own +y that opens a door stands it
    /// up on its hinge edge (`open_turn`).
    pub fn hatch(hatch: &modforge::structure::Hatch, at: Vec2, model: &str) -> Self {
        let (ends, across, width) = hatch.hinge_edge();
        let x = Vec3::new(across.x, 0.0, across.y);
        let y = x.cross(Vec3::Y);
        // The hinge: the end of the edge its +y runs on from.
        let hinge = if (ends[1] - ends[0]).dot(Vec2::new(y.x, y.z)) > 0.0 { ends[0] } else { ends[1] };
        let length = ends[0].distance(ends[1]);
        let centre = hinge + across * width / 2.0 + Vec2::new(y.x, y.z) * length / 2.0;
        Self {
            open: false,
            closed_center: at + centre,
            hinge: at + hinge,
            closed: Quat::from_mat3(&Mat3::from_cols(x, y, x.cross(y))),
            width,
            height: length,
            lying: true,
            model: model.to_string(),
        }
    }

    /// The turn of an open door: a quarter turn about its hinge (its own
    /// +y), a door swinging aside, a hatch standing up.
    pub fn open_turn(&self) -> Quat {
        let turn = if self.open { FRAC_PI_2 } else { 0.0 };
        self.closed * Quat::from_rotation_y(turn)
    }

    /// The way through the doorway on the ground, across the closed door;
    /// for a hatch, across its hole from the hinge, the way off it.
    pub fn across(&self) -> Vec2 {
        let along = (self.closed * Vec3::X).normalize_or_zero();
        let along = Vec2::new(along.x, along.z);
        if self.lying { along } else { along.perp() }
    }

    /// Which side of the closed door, standing on its floor at `floor`, a
    /// point is on: 1 or -1; a hatch's sides are above and below it.
    pub fn side_of(&self, p: Vec3, floor: f32) -> f32 {
        let ahead = if self.lying { p.y - (floor - 0.5) } else { (p.ground() - self.closed_center).dot(self.across()) };
        if ahead >= 0.0 { 1.0 } else { -1.0 }
    }

    /// Whether a knock from `knocker` on this door, standing on its floor
    /// at `floor`, is heard by someone at `ear` (feet): on the door's other
    /// side within `KNOCK_HEARD`, on its floor (a door) or on the floor
    /// below (a hatch).
    pub fn heard_through(&self, floor: f32, knocker: Vec3, ear: Vec3) -> bool {
        self.side_of(ear, floor) != self.side_of(knocker, floor) && self.within_hearing(floor, ear)
    }

    /// Whether someone at `ear` (feet) is within a knock's hearing of this
    /// door on its floor at `floor`: on that floor within `KNOCK_HEARD` of
    /// its doorway (a door), or on the floor below within it (a hatch).
    pub fn within_hearing(&self, floor: f32, ear: Vec3) -> bool {
        if self.lying {
            let below = floor - ear.y;
            below > 0.5 && below < crate::content::BUNKER_LEVEL + 1.5 && on_ground(self.closed_center, floor).distance(ear) <= KNOCK_HEARD
        } else {
            (ear.y - floor).abs() < 1.5 && ear.ground().distance(self.closed_center) <= KNOCK_HEARD
        }
    }

    /// Along its hinge on the ground: a door's doorway, a hatch's edge.
    pub fn along(&self) -> Vec2 {
        let along = (self.closed * if self.lying { Vec3::Y } else { Vec3::X }).normalize_or_zero();
        Vec2::new(along.x, along.z)
    }

    /// How far its edge stands out from its middle across it: none for a
    /// door in its doorway, half the hole for a hatch.
    pub fn edge_out(&self) -> f32 {
        if self.lying { self.width / 2.0 } else { 0.0 }
    }
}

/// Where a knocker stands, from the doorway's middle: this far out from the
/// door, and this far along the wall beside the doorway, so they stand
/// clear of it (a 1.2 m doorway holds one body, and someone knocking in it
/// once blocked the way out), metres.
pub const KNOCK_OUT: f32 = 0.7;
pub const KNOCK_ASIDE: f32 = 1.2;
/// With no room beside the doorway: straight out from it this far, leaving
/// room for someone to come out past them, metres.
pub const KNOCK_OUT_FAR: f32 = 1.8;
/// How far from a door's middle a knock reaches it: the farthest spot and
/// as near as standing there counts (modforge `KNOCK_ARRIVE`), metres.
pub const KNOCK_REACH: f32 = 2.4;
/// How far a knock is heard on the door's other side, on its level, metres
/// (operator, 2026-10-02).
pub const KNOCK_HEARD: f32 = 6.0;

/// Where to stand to knock on the door between `them` and a knocker: the
/// closed door nearest them whose knock they would hear (`within_hearing`:
/// a door on their level, a hatch over them), and on its side away from
/// them the first spot on the mesh there (`on_mesh` gives the mesh's point
/// for a spot, which must be the spot itself, not somewhere near it):
/// beside the doorway away from the hinge (where the door does not swing),
/// beside it on the hinge's side, straight out from it far enough to pass
/// (a hatch: out past its free edge); at its floor.
pub fn knock_spot<'a>(doors: impl Iterator<Item = (&'a Door, &'a Transform)>, them: Vec3, on_mesh: impl Fn(Vec3) -> Option<Vec3>) -> Option<Vec3> {
    let (door, floor) = doors
        .filter(|(d, at)| !d.open && d.within_hearing(at.translation.y, them))
        .map(|(d, at)| (d, at.translation.y))
        .min_by(|a, b| a.0.closed_center.distance(them.ground()).total_cmp(&b.0.closed_center.distance(them.ground())))?;
    let away = door.across() * -door.side_of(them, floor);
    let along = door.along() * KNOCK_ASIDE;
    let out = door.edge_out();
    [away * (out + KNOCK_OUT) + along, away * (out + KNOCK_OUT) - along, away * (out + KNOCK_OUT_FAR)]
        .into_iter()
        .map(|off| on_ground(door.closed_center + off, floor))
        .find_map(|spot| on_mesh(spot).filter(|p| p.ground().distance(spot.ground()) <= modforge::command::KNOCK_ARRIVE / 2.0))
}

/// Every person's Knock this tick: on the closed door in their reach, heard
/// by everyone on its other side within `KNOCK_HEARD` on its level, each
/// remembering who knocked, and the one line the knock calls through the
/// door to the player, if one fits (talk::called_line), heard as the knock
/// is. No conversation opens through a shut door: it starts once the two
/// are within talking reach (authority.md "Talking").
pub fn knock(
    tick: Res<crate::SimTick>,
    called: (Res<crate::episode::Episode>, Res<crate::talk::Lines>, Res<crate::terrain::WorldRes>, Res<crate::WorldSeed>, Res<crate::content::ItemRegistryRes>),
    doors: Query<(&Door, &Transform)>,
    knockers: Query<(Entity, &crate::actor::Actor, &Transform, &ThisTick), Without<Dead>>,
    mut people: Query<(Entity, &crate::actor::Actor, &Transform, &mut Persona, Has<crate::player::Player>), Without<Dead>>,
) {
    // In ActorId order (authority.md rule 5): of two knocking in one tick,
    // whom the player ends up talking to is the same after a load.
    let order = crate::actor::in_actor_order(knockers.iter().map(|(entity, actor, ..)| (Some(actor.0), entity)));
    for (knocker, id, at, this_tick) in order.into_iter().filter_map(|e| knockers.get(e).ok()) {
        if !ActionQueue::has(&this_tick.0, Action::Knock) {
            continue;
        }
        let feet = feet_of(at);
        let Some((door, floor)) = doors
            .iter()
            .filter(|(d, door_at)| !d.open && (door_at.translation.y - feet.y).abs() < 1.5 && d.closed_center.distance(feet.ground()) <= KNOCK_REACH)
            .map(|(d, door_at)| (d, door_at.translation.y))
            .min_by(|a, b| a.0.closed_center.distance(feet.ground()).total_cmp(&b.0.closed_center.distance(feet.ground())))
        else {
            continue;
        };
        // Who hears it: within hearing on the door's other side (`Door::
        // heard_through`: a door's floor, a hatch's floor below).
        let hears = |there: &Transform| door.heard_through(floor, feet, feet_of(there));
        // The one called line the knock carries to the player behind the
        // door, if one fits (authority.md "Talking"); no conversation opens
        // through a shut door, it starts once the two are within talking
        // reach.
        let player_behind = people.iter().find(|(e, _, there, _, is_player)| *e != knocker && *is_player && hears(there)).map(|(_, a, ..)| a.0);        let line = player_behind.and_then(|player| {
            let (_, _, _, person, _) = people.get(knocker).ok()?;
            crate::talk::called_line((&called.0, &called.1, called.2.bunker, called.3.0, &called.4.0), (id.0, person, feet), (player, true), crate::talk::Call::Knocking, tick.0)
        });
        for (hearer, _, there, mut memory, _) in &mut people {
            if hearer == knocker {
                memory.0.memory.did(modforge::memory::Did::Knocked, tick.0);
                if let (Some(line), Some(player)) = (&line, player_behind) {
                    memory.0.memory.did(modforge::memory::Did::Talked(player, line.clone()), tick.0);
                }
                continue;
            }
            if !hears(there) {
                continue;
            }
            memory.0.memory.did(modforge::memory::Did::HeardKnocking(id.0), tick.0);
            if let Some(line) = &line {
                memory.0.memory.did(modforge::memory::Did::Heard(id.0, line.clone()), tick.0);
            }
        }
    }
}

/// Anything from the item system lying in the world: a dropped can, a
/// storage box, the note. Items inside a box are stacks in its
/// `Container`, not WorldItems.
#[derive(Component, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct WorldItem {
    pub stack: ItemStack,
}

/// The inventory a deployed WorldItem holds (a storage box).
#[derive(Component, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Container(pub item::Inventory);

/// A source whose water comes out of a store (an episode's thing drawn
/// from a part's bunker's store): each drink takes one water from that box.
#[derive(Component, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct DrawsFrom(#[entities] pub Entity);

/// The town's water (topside-town "Everything works": "faucets, toilets,
/// showers, from the water tower through the pump house"; Project
/// Zomboid's plumbing): the water tower, a store of full water barrels,
/// and every house's sinks, shower and toilet drawing from it, each
/// through the one way a thing comes into the world. Once a world, as its
/// people; after a load they are in the save.
pub fn populate_town_water(
    mut commands: Commands,
    world: Res<crate::terrain::WorldRes>,
    items: Res<ItemRegistryRes>,
    mut ids: ResMut<ThingIssuer>,
    mut things_made: ResMut<crate::terrain::ThingsMade>,
) {
    if things_made.water {
        return;
    }
    things_made.water = true;
    let Some(town) = world.world.town.as_ref() else {
        return;
    };
    let (Some(tower_def), Some(barrel)) = (items.0.def(crate::content::WATER_TOWER), items.0.def("water barrel")) else {
        return;
    };
    let (tower, _) = spawn_item(&mut commands, &mut ids, tower_def, item::create(tower_def, 1, &[], 0.0, 0), on_ground(town.water_tower(), 0.0));
    let mut held = item::Inventory::new(crate::content::WATER_TOWER_BARRELS);
    for _ in 0..crate::content::WATER_TOWER_BARRELS {
        let mut full = item::create(barrel, 1, &[], 0.0, 0);
        full.litres = barrel.holds_litres.unwrap_or(0.0);
        let _ = held.add(full, barrel.max_stack);
    }
    let litres = modforge::fluid::litres_in(&held);
    commands.entity(tower).insert((Container(held), Drawn { held: litres }));
    for (kind, at) in town.fixtures() {
        let Some(def) = items.0.def(kind) else {
            continue;
        };
        let (made, _) = spawn_item(&mut commands, &mut ids, def, item::create(def, 1, &[], 0.0, 0), on_ground(at, 0.0));
        // The medicine cabinet is a box, found with what treats wounds.
        if kind == crate::content::MEDICINE_CABINET {
            let mut held = item::Inventory::new(def.storage.unwrap_or(0));
            for &(name, count) in &crate::content::CABINET_HOLDS {
                if let Some(medicine) = items.0.def(name) {
                    let _ = held.add(item::create(medicine, count, &[], 0.0, 0), medicine.max_stack);
                }
            }
            commands.entity(made).insert((Container(held), Interactable { kind: InteractKind::Container }));
            continue;
        }
        commands.entity(made).insert(DrawsFrom(tower));
    }
}

/// A store another place draws from (`DrawsFrom`), and the water it held
/// when that began (the episode's start: its thing placed). It says how
/// much it holds when looked at, and reads dry under content's `DRY_BELOW`
/// of what it held (todo 11ad).
#[derive(Component, Clone, Copy, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Drawn {
    /// The litres it held (modforge fluid).
    pub held: f32,
}

impl Drawn {
    /// Whether it reads dry holding `litres` now.
    pub fn dry(&self, litres: f32) -> bool {
        litres < self.held * crate::content::DRY_BELOW
    }
}

/// The thing a person at `feet` uses with E: within reach, the one nearest
/// the point they aim at (`pointed`). Also what the prompt names.
pub fn pointed_thing<'a>(feet: Vec3, pointed: Vec2, things: impl Iterator<Item = (Entity, Option<&'a Door>, &'a Transform)>) -> Option<Entity> {
    things
        .map(|(e, door, t)| (e, use_point(door, t)))
        .filter(|(_, p)| in_reach(feet, *p))
        .min_by(|a, b| a.1.ground().distance(pointed).total_cmp(&b.1.ground().distance(pointed)))
        .map(|(e, _)| e)
}

/// How many days what a store holds for `need` lasts a bunker of `people`
/// living people, as the brain counts it (modforge memory::days_of, each
/// eating or drinking a day's worth; orchestrator 2026-10-03: the bunker's
/// drinking is its people's): water for thirst, food for hunger.
pub fn days_of(held: &item::Inventory, need: modforge::survival::Need, items: &ItemRegistryRes, day: crate::survival::Day, people: u32) -> f32 {
    let supplies = held.slots.iter().flatten().map(|s| (s.item.as_str(), s.count));
    let litres = modforge::fluid::litres_in(held);
    let (hunger, thirst) = modforge::memory::days_of(supplies, litres, people.max(1), day.per_day(), &items.0);
    match need {
        modforge::survival::Need::Thirst => thirst,
        _ => hunger,
    }
}

/// Whether a melee weapon is in hand: what is held is a weapon with no
/// ammo (orchestrator 2026-10-03). The one answer the break rule
/// (`interact_on_use`) and the brain (ai::think, putting it away before
/// using a source) read.
pub fn melee_in_hand(hotbar: &modforge::hud::Hotbar, items: &modforge::item::ItemRegistry) -> bool {
    hotbar.held_stack().and_then(|s| items.def(&s.item)).and_then(|d| d.combat.as_ref()).is_some_and(|c| c.ammo.is_none())
}

/// The box this person has open, if any.
#[derive(Component, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct OpenContainer(#[entities] pub Option<Entity>);

/// The crafting station this person has open, if any.
#[derive(Component, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct OpenStation(pub Option<StationKind>);

/// The def a river's water is (content: kind `Source`), named by what it
/// is.
pub const RIVER: &str = "river";

/// Whether a stack answers `need` eaten or drunk: food that fills it, or,
/// for thirst, a container holding water (modforge fluid). The one answer
/// the brain, trading and stocking read.
pub fn answers(items: &modforge::item::ItemRegistry, stack: &item::ItemStack, need: modforge::survival::Need) -> bool {
    use modforge::survival::Need;
    let def = items.def(&stack.item);
    let food = def.and_then(|d| d.food.as_ref()).is_some_and(|f| match need {
        Need::Hunger => f.hunger > 0.0,
        Need::Thirst => f.thirst > 0.0,
        Need::Rest | Need::Safety => false,
    });
    food || (need == Need::Thirst && stack.litres > 0.0 && def.is_some_and(|d| d.holds_litres.is_some()))
}

/// The stack in a person's bags or belt a slot names, if it is there.
pub fn slot_stack<'a>(inventory: &'a mut Inventory, hotbar: Option<&'a mut crate::actor::Hotbar>, slot: modforge::hud::SlotRef) -> Option<&'a mut item::ItemStack> {
    let held = match slot.holder {
        modforge::hud::Holder::Inventory => &mut inventory.0,
        modforge::hud::Holder::Hotbar => &mut hotbar?.0.slots,
        _ => return None,
    };
    held.slots.get_mut(slot.index).and_then(Option::as_mut)
}

/// One mouthful from a source where it stands (a well, a bush, a river's
/// water): modforge's `SurvivalStats::eat` fills the needs, then the one
/// `hud::used` heals and remembers it. Nothing is used up.
fn eat_from_source(
    name: &str,
    registry: &ItemRegistryRes,
    person: &mut Persona,
    health: &mut Health,
    now: u64,
) {
    if let Some(def) = registry.0.def(name)
        && let Ok(heal) = person.0.needs.eat(def)
    {
        crate::hud::used(name, heal, registry, health, Some(person), now);
    }
}

/// Where a station stands for E, on its floor (x, height, y): nothing to
/// draw (the furniture is drawn), only the spot.
pub fn spawn_station(commands: &mut Commands, ids: &mut ThingIssuer, kind: StationKind, at: Vec3) -> Entity {
    commands
        .spawn((
            Interactable {
                kind: InteractKind::Station(kind),
            },
            ids.issue(),
            Transform::from_translation(at),
        ))
        .id()
}

/// A small item's size and a deployable's, in metres; scripts/models.py
/// makes item models to these sizes (ITEM, DEPLOYABLE), change both
/// together.
const ITEM_SIZE: f32 = 0.35;
const DEPLOYABLE_SIZE: f32 = 0.7;

/// The ONE path from an item stack to a WorldItem in the world, resting
/// at `at` (x, height, y: on whatever floor it lies on). A small item is a
/// sensor people walk over and E picks up; a deployable is solid, and a
/// box among them holds a `Container`. Its body is made from its item
/// (`item_body`). Answers the thing and the id it was given (what memory
/// knows it by).
pub fn spawn_item(commands: &mut Commands, ids: &mut ThingIssuer, def: &ItemDef, stack: ItemStack, at: Vec3) -> (Entity, ThingId) {
    debug_assert_eq!(def.name, stack.item, "the stack must be of this def");
    let kind = if def.storage.is_some() {
        InteractKind::Container
    } else if def.kind == ItemKind::Note {
        InteractKind::Read
    } else if def.kind == ItemKind::Source {
        // A well, a bush: used where it stands, never picked up.
        InteractKind::Source
    } else {
        InteractKind::Pickup
    };
    let id = ids.issue();
    let mut entity = commands.spawn((
        Interactable { kind },
        id,
        WorldItem { stack },
        Transform::from_translation(at),
    ));
    // Its model, made to this size by scripts/models.py; never a box
    // (design.md: no coloured boxes).
    match &def.picture {
        Some(name) => {
            entity.insert(Model(name.clone()));
        }
        None => warn!("item {} has no model", def.name),
    }
    if let Some(slots) = def.storage {
        entity.insert(Container(item::Inventory::new(slots)));
    }
    (entity.id(), id)
}

/// An item's body, made whenever an item comes into the world, spawned or
/// loaded: resting on its floor, the box raised by half its height; a
/// small item a sensor people walk over, a deployable solid.
pub fn item_body(add: On<Add, WorldItem>, things: Query<&WorldItem>, items: Res<crate::content::ItemRegistryRes>, mut commands: Commands) {
    let Some(def) = things.get(add.entity).ok().and_then(|w| items.0.def(&w.stack.item)) else {
        return;
    };
    let deployable = def.kind == ItemKind::Deployable;
    let size = if deployable { DEPLOYABLE_SIZE } else { ITEM_SIZE };
    let mut body = commands.entity(add.entity);
    body.insert((
        RigidBody::Static,
        Collider::compound(vec![(Vec3::Y * size / 2.0, Rotation::IDENTITY, Collider::cuboid(size, size, size))]),
        crate::player::makes_no_contacts(),
    ));
    if !deployable {
        body.insert(Sensor);
    }
}

/// A person's panels, modforge's HUD state: what they have open (the
/// inventory, a box, a station, a note). Everyone has one; the screen
/// draws the player's.
#[derive(Component, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Hud(pub HudState);

impl Default for Hud {
    fn default() -> Self {
        Self(HudState::new())
    }
}

/// Where a thing is used from: a door from where it stands closed, at its
/// floor's height.
pub(crate) fn use_point(door: Option<&Door>, transform: &Transform) -> Vec3 {
    match door {
        Some(d) => on_ground(d.closed_center, transform.translation.y),
        None => transform.translation,
    }
}

/// How far below a person's feet and above them a thing is in reach: on
/// their own level, up to a high shelf (authority.md "Presentation":
/// things stand at every height).
const BELOW_FEET: f32 = 0.5;
const ABOVE_FEET: f32 = 2.0;

/// Whether a thing resting at `at` is in reach of a person whose feet are
/// at `feet`: within `REACH` on the ground, and on their level.
pub fn in_reach(feet: Vec3, at: Vec3) -> bool {
    let up = at.y - feet.y;
    at.ground().distance(feet.ground()) <= REACH && (-BELOW_FEET..=ABOVE_FEET).contains(&up)
}

/// Where a person's feet are: their body's centre less its height.
pub fn feet_of(person: &Transform) -> Vec3 {
    person.translation - Vec3::Y * crate::player::BODY_CENTRE
}

/// Every person's Use this tick, the player's E and the brain's alike,
/// reaches the nearest interactable within `REACH`; modforge says what
/// happens and topside does it.
pub fn interact_on_use(
    mut commands: Commands,
    registry: Res<ItemRegistryRes>,
    (tick, day): (Res<crate::SimTick>, Res<crate::survival::Day>),
    chunks: Option<Res<crate::terrain::Chunks>>,
    mut people: Query<
        (
            (Entity, &Transform, &ThisTick, &mut Inventory, &mut Hud, &mut OpenContainer, &mut OpenStation),
            (&mut Persona, &mut Health, &Facing, Option<&crate::talk::LookingAt>, Option<&crate::actor::Actor>, Option<&mut crate::actor::Hotbar>),
            (Option<&mut crate::actor::ActorCondition>, Option<&mut crate::survival::Clean>),
        ),
        Without<Dead>,
    >,
    mut things: Query<
        (Entity, &mut Interactable, Option<&mut Door>, Option<&WorldItem>, &mut Transform, Option<&DrawsFrom>),
        Without<ThisTick>,
    >,
    mut stores: Query<&mut Container>,
    names: Query<&Name>,
) {
    // In ActorId order (authority.md rule 5): of two people using one thing
    // in one tick, who gets it is the same after a load.
    let order = crate::actor::in_actor_order(people.iter().map(|((me, ..), (.., actor, _), _)| (actor.map(|a| a.0), me)));
    for entity in order {
        let Ok(((me, person, this_tick, mut inventory, mut hud, mut open, mut station), (mut mind, mut health, facing, looking, _, mut hotbar), (mut condition, mut clean))) =
            people.get_mut(entity)
        else {
            continue;
        };
        // Use (E), or Fill: a container in their bags filled at the source
        // pointed at, the same reach and aim.
        let fills: Vec<modforge::hud::SlotRef> = this_tick
            .0
            .iter()
            .filter_map(|s| match s.action {
                Action::Fill { slot } => Some(slot),
                _ => None,
            })
            .collect();
        if fills.is_empty() && !this_tick.0.iter().any(|s| matches!(s.action, Action::Use)) {
            continue;
        }
        // Looking at a person: E talks to exactly them (episodes.md
        // "Conversations"), a conversation that ends by itself.
        if let Some((them, id)) = looking.and_then(|l| l.0).filter(|_| fills.is_empty()) {
            if hud.0.talking.as_ref().is_none_or(|t| t.with != id) {
                let name = names.get(them).map_or("", |n| n.as_str());
                hud.0.talking = Some(crate::talk::talking_with(id, name));
                commands.entity(me).insert(crate::talk::Conversation::new(id, tick.0, true));
            }
            continue;
        }
        // The thing pointed at: within reach of the person, the one
        // nearest the point they aim at (what the brain means to use);
        // with no aim (the player, in first person), the point a step
        // ahead of where they face.
        let feet = feet_of(person);
        let at = feet.ground();
        let pointed = ActionQueue::aim(&this_tick.0).map_or(at + facing.direction(), |(x, y)| Vec2::new(x, y));
        let nearest = pointed_thing(feet, pointed, things.iter().map(|(e, _, door, _, t, _)| (e, door, t)));
        let Some(Ok((entity, thing, door, item, transform, draws_from))) = nearest.map(|e| things.get_mut(e)) else {
            // Nothing to use in reach: a river's water pointed at is drunk
            // (design.md: rivers are drinkable), from the ground's level,
            // or a container filled from it.
            if in_reach(feet, on_ground(at, 0.0)) && chunks.as_ref().is_some_and(|c| nearest_water(c, at, pointed).is_some()) {
                if fills.is_empty() {
                    eat_from_source(RIVER, &registry, &mut mind, &mut health, tick.0);
                }
                for &slot in &fills {
                    if let Some(to) = slot_stack(&mut *inventory, hotbar.as_deref_mut(), slot) {
                        let mut river = f32::INFINITY;
                        modforge::fluid::fill(to, &mut river, &registry.0);
                    }
                }
            } else {
                // Every act's result is remembered, a failed one with why
                // (todo: write every act's result into memory, 2026-11-19).
                mind.0.memory.did(Did::Failed("use".to_string(), "nothing in reach".to_string()), tick.0);
            }
            continue;
        };
        // A source item (a well, a bush), decided by modforge's interact.
        if thing.kind == InteractKind::Source {
            if let Some(item) = item {
                // A thing that breaks when struck (its def's
                // `breaks_when_struck`), used with a melee weapon in hand,
                // is broken, and gone: by anyone, the player as any person
                // (orchestrator 2026-10-03: in hand, any melee weapon, a
                // weapon with no ammo). A person's brain puts the weapon
                // out of hand before it uses a source (ai::think).
                let breaks = registry.0.def(&item.stack.item).is_some_and(|d| d.breaks_when_struck);
                if breaks && fills.is_empty() && hotbar.as_ref().is_some_and(|h| melee_in_hand(&h.0, &registry.0)) {
                    commands.entity(entity).despawn();
                    mind.0.memory.did(Did::Done(format!("broke the {}", item.stack.item)), tick.0);
                    continue;
                }
                // A bed or a couch, used, is slept on; used again, the
                // sleeper wakes (topside-town "Everything works"). How
                // well it rests is `survival::survival_tick`'s.
                if fills.is_empty() && crate::content::COMFORT.iter().any(|(name, _)| *name == item.stack.item) {
                    if let Some(condition) = condition.as_mut() {
                        condition.0.asleep = !condition.0.asleep;
                        let did = if condition.0.asleep { Did::Slept } else { Did::Done(format!("got up from the {}", item.stack.item)) };
                        mind.0.memory.did(did, tick.0);
                    }
                    continue;
                }
                // Water drawn from a store comes out of the store's water
                // (modforge fluid::take_from); with none left the source
                // gives nothing.
                let mut store = draws_from.and_then(|from| stores.get_mut(from.0).ok());
                let gives_water = registry.0.def(&item.stack.item).and_then(|d| d.food).is_some_and(|f| f.thirst > 0.0);
                // Each container filled: up to its size, from the store if
                // drawn from one, else from a source that never runs dry.
                for &slot in fills.iter().filter(|_| gives_water) {
                    if let Some(to) = slot_stack(&mut *inventory, hotbar.as_deref_mut(), slot) {
                        let room = modforge::fluid::room(to, &registry.0);
                        to.litres += match store.as_mut() {
                            Some(store) => modforge::fluid::take_from(&mut store.0, room),
                            None => room,
                        };
                    }
                }
                // Used with E: a mouthful where it stands; from a store, a
                // mouthful's water out of it (drinking at a source stays as
                // it was, undecided, operator 2026-10-03).
                if fills.is_empty() {
                    if let Some(store) = store.as_mut()
                        && modforge::fluid::take_from(&mut store.0, modforge::fluid::MOUTHFUL) <= 0.0
                    {
                        mind.0.memory.did(Did::Failed(format!("drink at the {}", item.stack.item), "its store is empty".to_string()), tick.0);
                        continue;
                    }
                    eat_from_source(&item.stack.item, &registry, &mut mind, &mut health, tick.0);
                    // A shower washes all of one, a sink hands and face,
                    // soap carried doubling it (modforge hygiene).
                    let wash = match item.stack.item.as_str() {
                        crate::content::SHOWER => Some(modforge::hygiene::Wash::Whole),
                        crate::content::SINK => Some(modforge::hygiene::Wash::Hands),
                        _ => None,
                    };
                    if let (Some(wash), Some(clean)) = (wash, clean.as_mut()) {
                        let soap = inventory.0.count_of(crate::content::SOAP) > 0;
                        clean.0.wash(wash, soap);
                        mind.0.memory.did(Did::Done(format!("washed at the {}", item.stack.item)), tick.0);
                    }
                }
            }
            continue;
        }
        if !fills.is_empty() {
            continue;
        }
        let hour = crate::survival::today(&tick, *day).fract() * 24.0;
        let did = use_thing(
            &mut commands,
            &registry,
            (entity, thing, door, item, transform),
            &mut inventory,
            &mut hud,
            &mut open,
            &mut station,
            hour,
        );
        // What the use came to is remembered, as anything done is (an
        // episode's cue may wait on a note read).
        if let Some(did) = did {
            mind.0.memory.did(did, tick.0);
        }
    }
}

/// The water tile within reach of `at` nearest `pointed`, if any, from
/// the made chunks' shores (the one answer for what a tile is). Someone
/// standing at a shore reaches its water: the shore's land tile within
/// reach, the tile a person drinking is sent to stand on (a river's
/// remembered spot).
pub fn nearest_water(chunks: &crate::terrain::Chunks, at: Vec2, pointed: Vec2) -> Option<Vec2> {
    let c = crate::terrain::chunk_of(at);
    (c.1 - 1..=c.1 + 1)
        .flat_map(|y| (c.0 - 1..=c.0 + 1).map(move |x| (x, y)))
        .filter_map(|k| chunks.shores.get(&k))
        .flatten()
        // The edge of the land tile nearest the person is what they reach.
        .filter(|s| (centre(s.land) - at).abs().max_element() <= REACH + 0.5)
        .map(|s| centre(s.water))
        .min_by(|a, b| a.distance(pointed).total_cmp(&b.distance(pointed)))
}

/// One person uses one thing, at `hour` of the day.
#[allow(clippy::too_many_arguments)]
fn use_thing(
    commands: &mut Commands,
    registry: &ItemRegistryRes,
    (entity, mut thing, door, item, mut transform): (
        Entity,
        Mut<Interactable>,
        Option<Mut<Door>>,
        Option<&WorldItem>,
        Mut<Transform>,
    ),
    inventory: &mut Inventory,
    hud: &mut Hud,
    open: &mut OpenContainer,
    station: &mut OpenStation,
    hour: f32,
) -> Option<Did> {
    let stack = item.map(|w| w.stack.clone());
    let max_stack = match &stack {
        Some(stack) => match registry.0.def(&stack.item) {
            Some(def) => def.max_stack,
            // Not a registered item: it never enters the inventory.
            None => return Some(Did::Failed(format!("pick up {}", stack.item), "not a known item".to_string())),
        },
        None => 0,
    };
    let what = stack.as_ref().map_or_else(String::new, |s| s.item.clone());
    // modforge decides what happens; this carries out the engine side, and
    // answers what it came to, for the user's memory.
    match interact(thing.kind, &mut inventory.0, stack, max_stack, &mut hud.0) {
        InteractResult::ToggleDoor => {
            let Some(mut door) = door else {
                return None;
            };
            door.open = !door.open;
            thing.kind = InteractKind::Door { open: door.open };
            // A quarter turn about the hinge, where the door stands.
            transform.rotation = door.open_turn();
            if door.open {
                commands.entity(entity).insert(ColliderDisabled);
            } else {
                commands.entity(entity).remove::<ColliderDisabled>();
            }
            Some(Did::Done(if door.open { "opened a door" } else { "closed a door" }.to_string()))
        }
        InteractResult::PickedUp => {
            commands.entity(entity).despawn();
            Some(Did::Done(format!("picked up {what}")))
        }
        // modforge toggled the panel: E opens the box, E again closes it.
        InteractResult::OpenContainer => {
            open.0 = (hud.0.open_panel == OpenPanel::Inventory).then_some(entity);
            Some(Did::Done(if open.0.is_some() { "opened a box" } else { "closed a box" }.to_string()))
        }
        // modforge toggled the station panel: remember which station.
        InteractResult::OpenStation => {
            station.0 = match thing.kind {
                InteractKind::Station(kind) if hud.0.open_panel == OpenPanel::Station => Some(kind),
                _ => None,
            };
            Some(Did::Done(match station.0 {
                Some(kind) => format!("opened the {kind:?}"),
                None => "closed a station".to_string(),
            }))
        }
        // The note's words are in the HUD state now; nothing moves.
        InteractResult::Read => Some(Did::Read(what)),
        // A gate stays shut. A full inventory leaves the item lying.
        InteractResult::Locked(level) => Some(Did::Failed("open the gate".to_string(), format!("it is locked (level {level})"))),
        InteractResult::InventoryFull => Some(Did::Failed(format!("pick up {what}"), "the bags are full".to_string())),
        // A source is eaten from before this (`eat_from_source`), never here.
        InteractResult::UseSource => None,
        // A switch flips (a light switch, a breaker box); what it powers
        // follows (`power::lights`).
        InteractResult::ToggleSwitch => {
            let InteractKind::Switch { on } = thing.kind else {
                return None;
            };
            thing.kind = InteractKind::Switch { on: !on };
            // A TV, a radio or a phone turned on says what is on at this
            // hour (Works 16, modforge broadcast).
            let playing = crate::content::channel_of(&what).filter(|_| !on).and_then(|c| modforge::broadcast::on_air(c, hour));
            match playing {
                Some(words) => Some(Did::Done(format!("turned the {what} on: {words}"))),
                None => Some(Did::Done(format!("turned the {what} {}", if on { "off" } else { "on" }))),
            }
        }
        // The key was carried: unlocked for good (Works 14).
        InteractResult::Unlock(now) => {
            thing.kind = now;
            Some(Did::Done("unlocked it with its key".to_string()))
        }
        InteractResult::NeedsKey(_) => Some(Did::Failed("open it".to_string(), "it is locked".to_string())),
    }
}
