//! The one interact path: a person's Use this tick reaches the nearest
//! interactable thing in reach, modforge's `hud::interact` decides
//! what happens, and topside carries out the engine side.

use std::f32::consts::FRAC_PI_2;

use avian2d::prelude::*;
use bevy::prelude::*;
use modforge::crafting::StationKind;
use modforge::hud::{HudState, InteractKind, InteractResult, OpenPanel, interact};
use modforge::item::{self, ItemDef, ItemKind, ItemStack};
use modforge::structure::Rgb;

use crate::actor::{Dead, Inventory};
use crate::content::ItemRegistryRes;
use crate::input::{Action, ThisTick};

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

/// A small item's square and a deployable's, in metres.
const ITEM_SIZE: f32 = 0.35;
const DEPLOYABLE_SIZE: f32 = 0.7;
/// Above floors and furniture, under people.
const ITEM_Z: f32 = 0.8;

/// The ONE path from an item stack to a WorldItem in the world. A
/// small item is a sensor people walk over and E picks up; a
/// deployable is solid, and a box among them holds a `Container`.
pub fn spawn_item(commands: &mut Commands, def: &ItemDef, stack: ItemStack, at: Vec2, color: Rgb) -> Entity {
    debug_assert_eq!(def.name, stack.item, "the stack must be of this def");
    let deployable = def.kind == ItemKind::Deployable;
    let size = if deployable { DEPLOYABLE_SIZE } else { ITEM_SIZE };
    let kind = if def.storage.is_some() {
        InteractKind::Container
    } else if def.kind == ItemKind::Note {
        InteractKind::Read
    } else {
        InteractKind::Pickup
    };
    let mut entity = commands.spawn((
        Interactable { kind },
        WorldItem { stack },
        RigidBody::Static,
        Collider::rectangle(size, size),
        Sprite {
            color: Color::srgb(color[0], color[1], color[2]),
            custom_size: Some(Vec2::splat(size)),
            ..default()
        },
        Transform::from_translation(at.extend(ITEM_Z)),
    ));
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
    mut people: Query<
        (&Transform, &ThisTick, &mut Inventory, &mut Hud, &mut OpenContainer, &mut OpenStation),
        Without<Dead>,
    >,
    mut things: Query<
        (Entity, &mut Interactable, Option<&mut Door>, Option<&WorldItem>, &mut Transform),
        Without<ThisTick>,
    >,
) {
    for (person, this_tick, mut inventory, mut hud, mut open, mut station) in &mut people {
        if !this_tick.0.iter().any(|s| matches!(s.action, Action::Use)) {
            continue;
        }
        let at = person.translation.truncate();
        let nearest = things
            .iter()
            .map(|(e, _, door, _, t)| (e, use_point(door, t).distance(at)))
            .filter(|(_, d)| *d <= REACH)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(e, _)| e);
        let Some(Ok(thing)) = nearest.map(|e| things.get_mut(e)) else {
            continue;
        };
        use_thing(&mut commands, &registry, thing, &mut inventory, &mut hud, &mut open, &mut station);
    }
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
        InteractResult::Read | InteractResult::Locked(_) | InteractResult::InventoryFull => {}
    }
}
