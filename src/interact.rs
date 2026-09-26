//! The one interact path: a person's Use this tick reaches the nearest
//! interactable thing in reach, modforge's `hud::interact` decides
//! what happens, and topside carries out the engine side.

use std::f32::consts::FRAC_PI_2;

use avian2d::prelude::*;
use bevy::prelude::*;
use modforge::crafting::StationKind;
use modforge::hud::{HudState, InteractKind, InteractResult, OpenPanel, interact};
use modforge::item::{self, ItemDef, ItemKind, ItemStack};

use modforge::walk::centre;

use crate::actor::{Dead, Inventory, Memory, Survival};
use crate::combat::Health;
use crate::content::ItemRegistryRes;
use crate::input::{Action, ActionQueue, ThisTick};

/// How far a person reaches to use something, in metres.
pub const REACH: f32 = 1.5;

/// Something a person can use, and what kind of thing it is.
#[derive(Component)]
pub struct Interactable {
    pub kind: InteractKind,
}

/// A door. Open, it swings a quarter turn about its hinge and stops
/// nothing.
#[derive(Component)]
pub struct Door {
    pub open: bool,
    pub closed_center: Vec2,
    pub hinge: Vec2,
}

/// Anything from the item system lying in the world: a dropped can, a
/// storage box, the note. Items inside a box are stacks in its
/// `Container`, not WorldItems.
#[derive(Component)]
pub struct WorldItem {
    pub stack: ItemStack,
}

/// The inventory a deployed WorldItem holds (a storage box).
#[derive(Component)]
pub struct Container(pub item::Inventory);

/// The box this person has open, if any.
#[derive(Component, Default)]
pub struct OpenContainer(pub Option<Entity>);

/// The crafting station this person has open, if any.
#[derive(Component, Default)]
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

/// Where a station stands for E: nothing to draw (the furniture is
/// drawn), only the spot.
pub fn spawn_station(commands: &mut Commands, kind: StationKind, at: Vec2) -> Entity {
    commands
        .spawn((
            Interactable {
                kind: InteractKind::Station(kind),
            },
            Transform::from_translation(at.extend(0.0)),
        ))
        .id()
}

/// A small item's square and a deployable's, in metres; scripts/models.py
/// renders item pictures to these sizes (ITEM, DEPLOYABLE), change both
/// together.
const ITEM_SIZE: f32 = 0.35;
const DEPLOYABLE_SIZE: f32 = 0.7;
/// Above floors and furniture, under people.
const ITEM_Z: f32 = 0.8;

/// The ONE path from an item stack to a WorldItem in the world. A
/// small item is a sensor people walk over and E picks up; a
/// deployable is solid, and a box among them holds a `Container`.
pub fn spawn_item(commands: &mut Commands, def: &ItemDef, stack: ItemStack, at: Vec2) -> Entity {
    debug_assert_eq!(def.name, stack.item, "the stack must be of this def");
    let deployable = def.kind == ItemKind::Deployable;
    let size = if deployable { DEPLOYABLE_SIZE } else { ITEM_SIZE };
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
    let mut entity = commands.spawn((
        Interactable { kind },
        WorldItem { stack },
        RigidBody::Static,
        Collider::rectangle(size, size),
        Transform::from_translation(at.extend(ITEM_Z)),
    ));
    // Its picture, rendered to this size by scripts/models.py; never a box
    // (design.md: no coloured boxes).
    match &def.picture {
        Some(name) => {
            entity.insert(crate::picture::Picture(name.clone()));
        }
        None => warn!("item {} has no picture", def.name),
    }
    if !deployable {
        entity.insert(Sensor);
    }
    if let Some(slots) = def.storage {
        entity.insert(Container(item::Inventory::new(slots)));
    }
    entity.id()
}

/// A person's panels, modforge's HUD state: what they have open (the
/// inventory, a box, a station, a note). Everyone has one; the screen
/// draws the player's.
#[derive(Component)]
pub struct Hud(pub HudState);

impl Default for Hud {
    fn default() -> Self {
        Self(HudState::new())
    }
}

/// Where a thing is used from: a door from where it stands closed.
fn use_point(door: Option<&Door>, transform: &Transform) -> Vec2 {
    door.map_or(transform.translation.truncate(), |d| d.closed_center)
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
            (&Transform, &ThisTick, &mut Inventory, &mut Hud, &mut OpenContainer, &mut OpenStation),
            (&mut Survival, &mut Health, Option<&mut Memory>),
        ),
        Without<Dead>,
    >,
    mut things: Query<
        (Entity, &mut Interactable, Option<&mut Door>, Option<&WorldItem>, &mut Transform),
        Without<ThisTick>,
    >,
) {
    for ((person, this_tick, mut inventory, mut hud, mut open, mut station), (mut needs, mut health, mut memory)) in &mut people {
        if !this_tick.0.iter().any(|s| matches!(s.action, Action::Use)) {
            continue;
        }
        // The thing pointed at, the Factorio way: within reach of the
        // person, the one nearest the point they aim at (the mouse, or
        // what the brain means to use); with no aim, the nearest.
        let at = person.translation.truncate();
        let pointed = ActionQueue::aim(&this_tick.0).map_or(at, |(x, y)| Vec2::new(x, y));
        let nearest = things
            .iter()
            .map(|(e, _, door, _, t)| (e, use_point(door, t)))
            .filter(|(_, p)| p.distance(at) <= REACH)
            .min_by(|a, b| a.1.distance(pointed).total_cmp(&b.1.distance(pointed)))
            .map(|(e, _)| e);
        let Some(Ok((entity, thing, door, item, transform))) = nearest.map(|e| things.get_mut(e)) else {
            // Nothing to use in reach: a river's water pointed at is drunk
            // (design.md: rivers are drinkable).
            if chunks.as_ref().is_some_and(|c| nearest_water(c, at, pointed).is_some()) {
                eat_from_source(WATER, &registry, &mut needs, &mut health, memory.as_deref_mut(), tick.0);
            }
            continue;
        };
        // A source item (a well, a bush), decided by modforge's interact.
        if thing.kind == InteractKind::Source {
            if let Some(item) = item {
                eat_from_source(&item.stack.item, &registry, &mut needs, &mut health, memory.as_deref_mut(), tick.0);
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
/// the made chunks' shores (the one answer for what a tile is).
pub fn nearest_water(chunks: &crate::terrain::Chunks, at: Vec2, pointed: Vec2) -> Option<Vec2> {
    let c = crate::terrain::chunk_of(at);
    (c.1 - 1..=c.1 + 1)
        .flat_map(|y| (c.0 - 1..=c.0 + 1).map(move |x| (x, y)))
        .filter_map(|k| chunks.shores.get(&k))
        .flatten()
        .map(|s| centre(s.water))
        // The edge of the water tile nearest the person is what they reach.
        .filter(|w| (*w - at).abs().max_element() <= REACH + 0.5)
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
            let turn = if door.open { FRAC_PI_2 } else { 0.0 };
            let rotation = Quat::from_rotation_z(turn);
            let from_hinge = (door.closed_center - door.hinge).extend(0.0);
            transform.translation =
                (door.hinge.extend(0.0) + rotation * from_hinge).with_z(transform.translation.z);
            transform.rotation = rotation;
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
