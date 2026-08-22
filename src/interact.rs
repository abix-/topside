//! The one interact path: a raycast from the camera finds the
//! nearest interactable, feeds the prompt into the HUD, and E
//! dispatches to modforge's interact function.

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};
use modforge::hud::{InteractKind, InteractResult, OpenPanel, interact};
use modforge::item::{ItemDef, ItemKind, ItemStack};

use crate::actor::Inventory;
use crate::cel::CelMaterial;
use crate::content::ItemRegistryRes;
use crate::hud::{HoveredSlot, HudRes, panel_changed};
use crate::input::{Action, ActionQueue, ThisTick};
use crate::player::Player;
use crate::structure::{Door, Locked};

const INTERACT_RANGE: f32 = 3.0;
const ITEM_HALF: f32 = 0.2;

/// What E does to this entity. Where it is and how big is its Avian
/// collider; the E prompt asks Avian, never a second box.
#[derive(Component)]
pub struct Interactable {
    pub kind: InteractKind,
}

/// Anything from the item system sitting in the world: a dropped
/// can, a storage box, a crafting bench (design.md). Items inside a
/// box are stacks in its `Container`, not WorldItems.
#[derive(Component)]
pub struct WorldItem {
    pub stack: ItemStack,
}

/// The inventory a deployed WorldItem holds (a storage box). Opens
/// through the interact path and transfers with the player.
#[derive(Component)]
pub struct Container(pub modforge::item::Inventory);

/// The container the player has open, if any. Engine-side handle;
/// modforge only knows "a container is open" through `OpenPanel`.
#[derive(Resource, Default)]
pub struct OpenContainer(pub Option<Entity>);

const DEPLOYABLE_HALF: f32 = 0.35;

/// The ONE path from an item stack to a WorldItem entity. A pickup
/// is a small box E picks up; a deployable is a solid box E uses
/// (with `Container` when its def has storage).
pub fn spawn_item(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<CelMaterial>,
    def: &ItemDef,
    stack: ItemStack,
    position: Vec3,
    color: [f32; 3],
) -> Entity {
    debug_assert_eq!(def.name, stack.item, "the stack must be of this def");
    let deployable = def.kind == ItemKind::Deployable;
    let half = if deployable { DEPLOYABLE_HALF } else { ITEM_HALF };
    let mut entity = commands.spawn((
        Interactable {
            kind: if def.storage.is_some() {
                InteractKind::Container
            } else if def.kind == ItemKind::Note {
                InteractKind::Read
            } else {
                InteractKind::Pickup
            },
        },
        WorldItem { stack },
        Mesh3d(meshes.add(Cuboid::new(half * 2.0, half * 2.0, half * 2.0))),
        MeshMaterial3d(materials.add(CelMaterial::from_srgb(color[0], color[1], color[2]))),
        Transform::from_translation(position),
        Visibility::default(),
        // The one box: Avian's. The E ray finds it; a deployable
        // also blocks the body, a small item is a sensor the walk
        // and the swing pass through.
        RigidBody::Static,
        Collider::cuboid(half * 2.0, half * 2.0, half * 2.0),
    ));
    if !deployable {
        entity.insert(Sensor);
    }
    if let Some(slots) = def.storage {
        entity.insert(Container(modforge::item::Inventory::new(slots)));
    }
    entity.id()
}

/// Nearest interactable the player is looking at this frame.
#[derive(Resource, Default)]
pub struct LookingAt {
    pub entity: Option<Entity>,
    pub kind: Option<InteractKind>,
    pub distance: f32,
}

/// What the E prompt names: one Avian ray from the eye along the
/// look, the first thing it touches within reach. A door or an
/// interactable gives the kind; a wall in between gives nothing.
pub fn update_looking_at(
    spatial: SpatialQuery,
    players: Query<(Entity, &Transform, Option<&Children>), With<Player>>,
    interactables: Query<&Interactable>,
    doors: Query<(&Door, Option<&Locked>)>,
    mut looking_at: ResMut<LookingAt>,
) {
    looking_at.entity = None;
    looking_at.kind = None;
    looking_at.distance = f32::MAX;

    let Ok((player, transform, children)) = players.single() else {
        return;
    };
    let own: Vec<Entity> = std::iter::once(player)
        .chain(children.into_iter().flat_map(|c| c.iter()))
        .collect();
    let filter = SpatialQueryFilter::from_excluded_entities(own);
    let Some(hit) = spatial.cast_ray(
        transform.translation,
        transform.forward(),
        INTERACT_RANGE,
        true,
        &filter,
    ) else {
        return;
    };
    let kind = if let Ok((door, locked)) = doors.get(hit.entity) {
        match locked {
            Some(Locked(level)) => InteractKind::Gate { level: *level },
            None => InteractKind::Door { open: door.open },
        }
    } else if let Ok(interactable) = interactables.get(hit.entity) {
        interactable.kind
    } else {
        return;
    };
    looking_at.entity = Some(hit.entity);
    looking_at.kind = Some(kind);
    looking_at.distance = hit.distance;
}

/// `Use` among this tick's actions: interact with what is looked at.
pub fn interact_on_press(
    mut commands: Commands,
    looking_at: Res<LookingAt>,
    mut hud: ResMut<HudRes>,
    registry: Res<ItemRegistryRes>,
    mut open_container: ResMut<OpenContainer>,
    mut hovered: ResMut<HoveredSlot>,
    mut players: Query<(&mut Inventory, &ThisTick), With<Player>>,
    mut doors: Query<(&mut Door, &mut Transform)>,
    items: Query<&WorldItem>,
    mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let Ok((mut inventory, this_tick)) = players.single_mut() else {
        return;
    };
    if !ActionQueue::has(&this_tick.0, Action::Use) {
        return;
    }
    let Some(entity) = looking_at.entity else {
        return;
    };
    let Some(kind) = looking_at.kind else {
        return;
    };
    // modforge decides what happens; this system carries out the
    // engine side of the result and nothing more.
    let stack = items.get(entity).ok().map(|w| w.stack.clone());
    let max_stack = match &stack {
        Some(stack) => match registry.0.def(&stack.item) {
            Some(def) => def.max_stack,
            // Not a registered item: it never enters the inventory.
            None => return,
        },
        None => 0,
    };
    match interact(kind, &mut inventory.0, stack, max_stack, &mut hud.0) {
        InteractResult::ToggleDoor => {
            if let Ok((mut door, mut transform)) = doors.get_mut(entity) {
                door.open = !door.open;
                *transform = door.transform();
            }
        }
        InteractResult::PickedUp => commands.entity(entity).despawn(),
        InteractResult::OpenContainer => {
            // modforge toggled the panel: E opens the crate, E again
            // closes it. Same follow-up as the inventory keys.
            panel_changed(&mut hud.0, &mut hovered, &mut open_container, &mut cursors);
            if hud.0.open_panel == OpenPanel::Inventory {
                open_container.0 = Some(entity);
            }
        }
        // The note's words are in the HUD state now; nothing moves.
        // The gate stays shut; the prompt already says so.
        InteractResult::Read | InteractResult::Locked(_) | InteractResult::InventoryFull => {}
    }
}
