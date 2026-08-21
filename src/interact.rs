//! The one interact path: a raycast from the camera finds the
//! nearest interactable, feeds the prompt into the HUD, and E
//! dispatches to modforge's interact function.

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};
use modforge::hud::{InteractKind, InteractResult, OpenPanel, interact};
use modforge::item::ItemStack;
use modforge::structure::Aabb;

use crate::actor::Inventory;
use crate::cel::CelMaterial;
use crate::content::ItemRegistryRes;
use crate::hud::{HoveredSlot, HudRes, panel_changed};
use crate::player::Player;
use crate::structure::Door;

const INTERACT_RANGE: f32 = 3.0;
const ITEM_HALF: f32 = 0.2;

#[derive(Component)]
pub struct Interactable {
    pub kind: InteractKind,
    pub aabb: Aabb,
}

#[derive(Component)]
pub struct WorldItem {
    pub stack: ItemStack,
}

#[derive(Resource, Default)]
pub struct InteractInput(pub bool);

/// A crate, chest, or corpse: an inventory in the world that opens
/// through the interact path and transfers with the player.
#[derive(Component)]
pub struct Container(pub modforge::item::Inventory);

/// The container the player has open, if any. Engine-side handle;
/// modforge only knows "a container is open" through `OpenPanel`.
#[derive(Resource, Default)]
pub struct OpenContainer(pub Option<Entity>);

pub const CONTAINER_SLOTS: usize = 12;
const CONTAINER_SIZE: f32 = 0.7;

/// The ONE path from an inventory to a container entity in the
/// world: a solid box with a collider, opened with E.
pub fn spawn_container(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<CelMaterial>,
    contents: modforge::item::Inventory,
    position: Vec3,
    color: [f32; 3],
) -> Entity {
    let half = CONTAINER_SIZE / 2.0;
    commands
        .spawn((
            Interactable {
                kind: InteractKind::Container,
                aabb: Aabb {
                    min: position - Vec3::splat(half),
                    max: position + Vec3::splat(half),
                },
            },
            Container(contents),
            RigidBody::Static,
            Collider::cuboid(CONTAINER_SIZE, CONTAINER_SIZE, CONTAINER_SIZE),
            Mesh3d(meshes.add(Cuboid::new(CONTAINER_SIZE, CONTAINER_SIZE, CONTAINER_SIZE))),
            MeshMaterial3d(materials.add(CelMaterial::from_srgb(color[0], color[1], color[2]))),
            Transform::from_translation(position),
            Visibility::default(),
        ))
        .id()
}

/// The ONE path from an item stack to a pickup entity in the world.
pub fn spawn_item(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<CelMaterial>,
    stack: ItemStack,
    position: Vec3,
    color: [f32; 3],
) -> Entity {
    let aabb = Aabb {
        min: position - Vec3::splat(ITEM_HALF),
        max: position + Vec3::splat(ITEM_HALF),
    };
    let mesh = meshes.add(Cuboid::new(
        ITEM_HALF * 2.0,
        ITEM_HALF * 2.0,
        ITEM_HALF * 2.0,
    ));
    let mat = materials.add(CelMaterial::from_srgb(color[0], color[1], color[2]));
    commands
        .spawn((
            Interactable {
                kind: InteractKind::Pickup,
                aabb,
            },
            WorldItem { stack },
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform::from_translation(position),
            Visibility::default(),
        ))
        .id()
}

/// Nearest interactable the player is looking at this frame.
#[derive(Resource, Default)]
pub struct LookingAt {
    pub entity: Option<Entity>,
    pub kind: Option<InteractKind>,
    pub distance: f32,
}

pub fn capture_interact_input(keys: Res<ButtonInput<KeyCode>>, mut input: ResMut<InteractInput>) {
    input.0 |= keys.just_pressed(KeyCode::KeyE);
}

/// Ray-AABB slab test. Returns the distance along the ray to the
/// nearest face, or None if the ray misses.
fn ray_aabb(origin: Vec3, dir: Vec3, aabb: &Aabb) -> Option<f32> {
    let inv = Vec3::new(1.0 / dir.x, 1.0 / dir.y, 1.0 / dir.z);
    let t1 = (aabb.min - origin) * inv;
    let t2 = (aabb.max - origin) * inv;
    let tmin = t1.min(t2);
    let tmax = t1.max(t2);
    let enter = tmin.x.max(tmin.y).max(tmin.z);
    let exit = tmax.x.min(tmax.y).min(tmax.z);
    if enter <= exit && exit >= 0.0 {
        let t = if enter >= 0.0 { enter } else { exit };
        if t <= INTERACT_RANGE {
            return Some(t);
        }
    }
    None
}

pub fn update_looking_at(
    players: Query<&Transform, With<Player>>,
    interactables: Query<(Entity, &Interactable)>,
    doors: Query<(Entity, &Door)>,
    mut looking_at: ResMut<LookingAt>,
) {
    looking_at.entity = None;
    looking_at.kind = None;
    looking_at.distance = f32::MAX;

    let Ok(player) = players.single() else {
        return;
    };
    let origin = player.translation;
    let dir = player.forward().as_vec3();

    for (entity, door) in &doors {
        if let Some(t) = ray_aabb(origin, dir, &door.collider)
            && t < looking_at.distance
        {
            looking_at.entity = Some(entity);
            looking_at.kind = Some(InteractKind::Door { open: door.open });
            looking_at.distance = t;
        }
    }

    for (entity, interactable) in &interactables {
        if let Some(t) = ray_aabb(origin, dir, &interactable.aabb)
            && t < looking_at.distance
        {
            looking_at.entity = Some(entity);
            looking_at.kind = Some(interactable.kind);
            looking_at.distance = t;
        }
    }
}

pub fn interact_on_press(
    mut commands: Commands,
    mut input: ResMut<InteractInput>,
    looking_at: Res<LookingAt>,
    mut hud: ResMut<HudRes>,
    registry: Res<ItemRegistryRes>,
    mut open_container: ResMut<OpenContainer>,
    mut hovered: ResMut<HoveredSlot>,
    mut players: Query<&mut Inventory, With<Player>>,
    mut doors: Query<(&mut Door, &mut Transform)>,
    items: Query<&WorldItem>,
    mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if !std::mem::take(&mut input.0) {
        return;
    }
    let Some(entity) = looking_at.entity else {
        return;
    };
    let Some(kind) = looking_at.kind else {
        return;
    };
    let Ok(mut inventory) = players.single_mut() else {
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
        InteractResult::InventoryFull => {}
    }
}
