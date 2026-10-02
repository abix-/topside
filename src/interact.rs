//! The one interact path: a person's Use this tick reaches the nearest
//! interactable thing in reach, modforge's `hud::interact` decides
//! what happens, and topside carries out the engine side.

use std::f32::consts::FRAC_PI_2;

use avian3d::prelude::*;
use bevy::prelude::*;
use modforge::crafting::StationKind;
use modforge::hud::{HudState, InteractKind, InteractResult, OpenPanel, interact};
use modforge::item::{self, ItemDef, ItemKind, ItemStack};

use modforge::walk::centre;

use crate::actor::{Dead, Facing, Inventory, Memory, Survival};
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
    pub model: String,
}

/// How thick a door is, in metres (scripts/models.py's panel, made 1 m
/// wide and a wall tall).
const DOOR_THICK: f32 = 0.12;
use modforge::structure::DOORWAY_HEIGHT;

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
            Vec3::new(width / 2.0, DOORWAY_HEIGHT / 2.0, 0.0),
            Rotation::IDENTITY,
            Collider::cuboid(width, DOORWAY_HEIGHT, DOOR_THICK),
        )]),
        Visibility::default(),
    ));
    body.with_child((
        Model(door.model.clone()),
        Transform::from_xyz(width / 2.0, 0.0, 0.0).with_scale(Vec3::new(width, DOORWAY_HEIGHT / crate::structure::WALL_HEIGHT, 1.0)),
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

    /// The way through the doorway on the ground, across the closed door.
    pub fn across(&self) -> Vec2 {
        (self.closed_center - self.hinge).perp().normalize_or_zero()
    }

    /// Which side of the closed door a ground point is on: 1 or -1.
    pub fn side_of(&self, p: Vec2) -> f32 {
        if (p - self.closed_center).dot(self.across()) >= 0.0 { 1.0 } else { -1.0 }
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
/// closed door on their level nearest them within a knock's hearing, and
/// on its side away from them the first spot on the mesh there (`on_mesh`
/// gives the mesh's point for a spot, which must be the spot itself, not
/// somewhere near it): beside the doorway away from the hinge (where the
/// door does not swing), beside it on the hinge's side, straight out from
/// it far enough to pass; at its floor.
pub fn knock_spot<'a>(doors: impl Iterator<Item = (&'a Door, &'a Transform)>, them: Vec3, on_mesh: impl Fn(Vec3) -> Option<Vec3>) -> Option<Vec3> {
    let (door, floor) = doors
        .filter(|(d, at)| !d.open && (at.translation.y - them.y).abs() < 1.5)
        .map(|(d, at)| (d, at.translation.y))
        .filter(|(d, _)| d.closed_center.distance(them.ground()) <= KNOCK_HEARD)
        .min_by(|a, b| a.0.closed_center.distance(them.ground()).total_cmp(&b.0.closed_center.distance(them.ground())))?;
    let away = door.across() * -door.side_of(them.ground());
    let along = (door.closed_center - door.hinge).normalize_or_zero() * KNOCK_ASIDE;
    [away * KNOCK_OUT + along, away * KNOCK_OUT - along, away * KNOCK_OUT_FAR]
        .into_iter()
        .map(|off| on_ground(door.closed_center + off, floor))
        .find_map(|spot| on_mesh(spot).filter(|p| p.ground().distance(spot.ground()) <= modforge::command::KNOCK_ARRIVE / 2.0))
}

/// Every person's Knock this tick: on the closed door in their reach, heard
/// by everyone on its other side within `KNOCK_HEARD` on its level, each
/// remembering who knocked; a player who hears it is in a conversation with
/// the one who knocked (talk::spoken_to), as being spoken to.
pub fn knock(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    doors: Query<(&Door, &Transform)>,
    knockers: Query<(Entity, &crate::actor::Actor, &Transform, &ThisTick, &Name), Without<Dead>>,
    mut people: Query<(Entity, &crate::actor::Actor, &Transform, &mut Memory, Option<&mut Hud>, Has<crate::player::Player>), Without<Dead>>,
) {
    // In ActorId order (authority.md rule 5): of two knocking in one tick,
    // whom the player ends up talking to is the same after a load.
    let order = crate::actor::in_actor_order(knockers.iter().map(|(entity, actor, ..)| (Some(actor.0), entity)));
    for (knocker, id, at, this_tick, name) in order.into_iter().filter_map(|e| knockers.get(e).ok()) {
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
        let knocker_side = door.side_of(feet.ground());
        for (hearer, _, there, mut memory, hud, is_player) in &mut people {
            if hearer == knocker {
                memory.0.did(modforge::memory::Did::Knocked, tick.0);
                continue;
            }
            let ear = feet_of(there);
            if (ear.y - floor).abs() >= 1.5 || ear.ground().distance(door.closed_center) > KNOCK_HEARD || door.side_of(ear.ground()) == knocker_side {
                continue;
            }
            memory.0.did(modforge::memory::Did::HeardKnocking(id.0), tick.0);
            if is_player && let Some(mut hud) = hud {
                crate::talk::spoken_to(&mut commands, hearer, &mut hud, (id.0.0, name.as_str()), tick.0);
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

/// A source whose water comes out of a store (The Tap's tap, drawn from
/// Mara's bunker's store): each drink takes one water from that box.
#[derive(Component, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct DrawsFrom(#[entities] pub Entity);

/// The box this person has open, if any.
#[derive(Component, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct OpenContainer(#[entities] pub Option<Entity>);

/// The crafting station this person has open, if any.
#[derive(Component, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct OpenStation(pub Option<StationKind>);

/// The def a river's water is (content: kind `Source`).
pub const WATER: &str = "water";

/// One mouthful from a source where it stands (a well, a bush, a river's
/// water): modforge's `SurvivalStats::eat` fills the needs, then the one
/// `hud::used` heals and remembers it. Nothing is used up.
fn eat_from_source(
    name: &str,
    registry: &ItemRegistryRes,
    needs: &mut Survival,
    health: &mut Health,
    memory: Option<&mut Memory>,
    now: u64,
) {
    if let Some(def) = registry.0.def(name)
        && let Ok(heal) = needs.0.eat(def)
    {
        crate::hud::used(name, heal, registry, health, memory, now);
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
    tick: Res<crate::SimTick>,
    chunks: Option<Res<crate::terrain::Chunks>>,
    mut people: Query<
        (
            (Entity, &Transform, &ThisTick, &mut Inventory, &mut Hud, &mut OpenContainer, &mut OpenStation),
            (&mut Survival, &mut Health, Option<&mut Memory>, &Facing, Option<&crate::talk::LookingAt>, Option<&crate::actor::Actor>),
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
    let order = crate::actor::in_actor_order(people.iter().map(|((me, ..), (.., actor))| (actor.map(|a| a.0), me)));
    for entity in order {
        let Ok(((me, person, this_tick, mut inventory, mut hud, mut open, mut station), (mut needs, mut health, mut memory, facing, looking, _))) =
            people.get_mut(entity)
        else {
            continue;
        };
        if !this_tick.0.iter().any(|s| matches!(s.action, Action::Use)) {
            continue;
        }
        // Looking at a person: E talks to exactly them (episodes.md
        // "Conversations"), a conversation that ends by itself.
        if let Some((them, id)) = looking.and_then(|l| l.0) {
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
        let nearest = things
            .iter()
            .map(|(e, _, door, _, t, _)| (e, use_point(door, t)))
            .filter(|(_, p)| in_reach(feet, *p))
            .min_by(|a, b| a.1.ground().distance(pointed).total_cmp(&b.1.ground().distance(pointed)))
            .map(|(e, _)| e);
        let Some(Ok((entity, thing, door, item, transform, draws_from))) = nearest.map(|e| things.get_mut(e)) else {
            // Nothing to use in reach: a river's water pointed at is drunk
            // (design.md: rivers are drinkable), from the ground's level.
            if in_reach(feet, on_ground(at, 0.0)) && chunks.as_ref().is_some_and(|c| nearest_water(c, at, pointed).is_some()) {
                eat_from_source(WATER, &registry, &mut needs, &mut health, memory.as_deref_mut(), tick.0);
            }
            continue;
        };
        // A source item (a well, a bush), decided by modforge's interact.
        if thing.kind == InteractKind::Source {
            if let Some(item) = item {
                eat_from_source(&item.stack.item, &registry, &mut needs, &mut health, memory.as_deref_mut(), tick.0);
                // Water drawn from a store: one of its waters goes.
                if let Some(from) = draws_from
                    && let Ok(mut store) = stores.get_mut(from.0)
                {
                    let water = store
                        .0
                        .slots
                        .iter()
                        .flatten()
                        .find(|s| registry.0.def(&s.item).and_then(|d| d.food).is_some_and(|f| f.thirst > 0.0))
                        .map(|s| s.item.clone());
                    if let Some(water) = water {
                        store.0.take_one(&water);
                    }
                }
            }
            continue;
        }
        use_thing(
            &mut commands,
            &registry,
            (entity, thing, door, item, transform),
            &mut inventory,
            &mut hud,
            &mut open,
            &mut station,
        );
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

/// One person uses one thing.
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
) {
    let stack = item.map(|w| w.stack.clone());
    let max_stack = match &stack {
        Some(stack) => match registry.0.def(&stack.item) {
            Some(def) => def.max_stack,
            // Not a registered item: it never enters the inventory.
            None => return,
        },
        None => 0,
    };
    // modforge decides what happens; this carries out the engine side.
    match interact(thing.kind, &mut inventory.0, stack, max_stack, &mut hud.0) {
        InteractResult::ToggleDoor => {
            let Some(mut door) = door else {
                return;
            };
            door.open = !door.open;
            thing.kind = InteractKind::Door { open: door.open };
            // A quarter turn about the hinge, where the door stands.
            let turn = if door.open { FRAC_PI_2 } else { 0.0 };
            transform.rotation = Quat::from_rotation_y(turn) * door.closed;
            if door.open {
                commands.entity(entity).insert(ColliderDisabled);
            } else {
                commands.entity(entity).remove::<ColliderDisabled>();
            }
        }
        InteractResult::PickedUp => commands.entity(entity).despawn(),
        // modforge toggled the panel: E opens the box, E again closes it.
        InteractResult::OpenContainer => {
            open.0 = (hud.0.open_panel == OpenPanel::Inventory).then_some(entity);
        }
        // modforge toggled the station panel: remember which station.
        InteractResult::OpenStation => {
            station.0 = match thing.kind {
                InteractKind::Station(kind) if hud.0.open_panel == OpenPanel::Station => Some(kind),
                _ => None,
            };
        }
        // The note's words are in the HUD state now; nothing moves. A
        // gate stays shut. A full inventory leaves the item lying.
        // A source is eaten from before this (`eat_from_source`), never here.
        InteractResult::Read | InteractResult::Locked(_) | InteractResult::InventoryFull | InteractResult::UseSource => {}
    }
}
