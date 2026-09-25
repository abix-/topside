//! The one interact path: a person's Use this tick reaches the nearest
//! interactable thing in reach, modforge's `hud::interact` decides
//! what happens, and topside carries out the engine side.

use std::f32::consts::FRAC_PI_2;

use avian2d::prelude::*;
use bevy::prelude::*;
use modforge::hud::{HudState, InteractKind, InteractResult, OpenPanel, interact};
use modforge::item::{self, ItemDef, ItemKind, ItemStack};
use modforge::structure::Rgb;

use crate::actor::{Dead, Inventory};
use crate::content::ItemRegistryRes;
use crate::input::{Action, ThisTick};
use crate::player::Player;

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

/// The box the player has open, if any.
#[derive(Resource, Default)]
pub struct OpenContainer(pub Option<Entity>);

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

/// The HUD's state, modforge's.
#[derive(Resource)]
pub struct HudRes(pub HudState);

impl Default for HudRes {
    fn default() -> Self {
        Self(HudState::new())
    }
}

/// The player's Use this tick reaches the nearest interactable within
/// `REACH`; modforge says what happens and topside does it.
pub fn interact_on_use(
    mut commands: Commands,
    mut hud: ResMut<HudRes>,
    mut open: ResMut<OpenContainer>,
    registry: Res<ItemRegistryRes>,
    mut players: Query<(&Transform, &ThisTick, &mut Inventory), (With<Player>, Without<Dead>)>,
    mut things: Query<
        (Entity, &mut Interactable, Option<&mut Door>, Option<&WorldItem>, &mut Transform),
        Without<Player>,
    >,
) {
    let Ok((player, this_tick, mut inventory)) = players.single_mut() else {
        return;
    };
    if !this_tick.0.iter().any(|s| matches!(s.action, Action::Use)) {
        return;
    }
    let at = player.translation.truncate();
    let distance = |door: Option<&Door>, transform: &Transform| {
        door.map_or(transform.translation.truncate(), |d| d.closed_center).distance(at)
    };
    let Some((entity, mut thing, door, item, mut transform)) = things
        .iter_mut()
        .filter(|(_, _, door, _, transform)| distance(door.as_deref(), transform) <= REACH)
        .min_by(|a, b| distance(a.2.as_deref(), &a.4).total_cmp(&distance(b.2.as_deref(), &b.4)))
    else {
        return;
    };
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
        // The note's words are in the HUD state now; nothing moves. A
        // gate stays shut. A full inventory leaves the item lying.
        InteractResult::Read | InteractResult::Locked(_) | InteractResult::InventoryFull => {}
    }
}
