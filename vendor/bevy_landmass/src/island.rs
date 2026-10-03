use std::sync::Arc;

use bevy_asset::{AssetEvent, Assets};
use bevy_ecs::{
  bundle::Bundle,
  change_detection::DetectChanges,
  component::Component,
  entity::Entity,
  lifecycle::RemovedComponents,
  message::MessageReader,
  query::{Added, Changed, Or, With},
  system::{Local, Query, Res},
};
use bevy_platform::collections::HashSet;
use bevy_transform::{
  components::{GlobalTransform, Transform},
  helper::TransformHelper,
};

use crate::{
  Archipelago, ArchipelagoRef, NavMesh, NavMeshHandle,
  coords::{CoordinateSystem, ThreeD, TwoD},
};

/// A bundle to create islands. The GlobalTransform component is omitted, since
/// this is commonly added in other bundles (which is redundant and can
/// override previous bundles).
#[derive(Bundle)]
pub struct IslandBundle<CS: CoordinateSystem> {
  /// An island marker component.
  pub island: Island,
  /// A reference pointing to the Archipelago to associate this entity with.
  pub archipelago_ref: ArchipelagoRef<CS>,
  /// A handle to the nav mesh that this island needs.
  pub nav_mesh: NavMeshHandle<CS>,
}

pub type Island2dBundle = IslandBundle<TwoD>;
pub type Island3dBundle = IslandBundle<ThreeD>;

/// A marker component that an entity is an island.
#[derive(Component)]
#[require(Transform)]
pub struct Island;

type IslandQuery<'w, 's, CS> = Query<
  'w,
  's,
  (Entity, &'static NavMeshHandle<CS>, &'static ArchipelagoRef<CS>),
  (With<Island>, With<Transform>),
>;

/// Ensures that the island transform and nav mesh are up to date.
///
/// topside patch (todo 9n): only the islands that may differ from their
/// archipelago are looked at, not every island every step (1156 islands,
/// about 0.27 ms a step, none of them moving): those added or changed
/// (handle, archipelago, transform), those gone, those still waiting to be
/// placed (no mesh yet, no archipelago, no transform), and those whose mesh
/// had an asset event. Every island is looked at when an archipelago is
/// added. A mesh swapped in place under the same id is seen the same tick
/// because whoever swaps it marks its islands' handles changed
/// (landmass_rerecast `convert`); an asset event (a frame later) catches
/// any other swap. A parent's move is seen through GlobalTransform, a frame
/// later.
pub(crate) fn sync_islands_to_archipelago<CS: CoordinateSystem>(
  mut archipelagos: Query<(Entity, &mut Archipelago<CS>)>,
  islands: IslandQuery<CS>,
  changed: Query<
    Entity,
    (
      With<Island>,
      Or<(
        Added<Island>,
        Changed<NavMeshHandle<CS>>,
        Changed<ArchipelagoRef<CS>>,
        Changed<Transform>,
        Changed<GlobalTransform>,
      )>,
    ),
  >,
  mut removed_islands: RemovedComponents<Island>,
  mut removed_handles: RemovedComponents<NavMeshHandle<CS>>,
  mut removed_refs: RemovedComponents<ArchipelagoRef<CS>>,
  mut mesh_events: MessageReader<AssetEvent<NavMesh<CS>>>,
  mut waiting: Local<HashSet<Entity>>,
  transform_helper: TransformHelper,
  nav_meshes: Res<Assets<NavMesh<CS>>>,
) {
  let mut check: Vec<Entity> = waiting.drain().collect();
  check.extend(changed.iter());
  check.extend(removed_islands.read());
  check.extend(removed_handles.read());
  check.extend(removed_refs.read());
  let meshes = mesh_events
    .read()
    .map(|event| match *event {
      AssetEvent::Added { id }
      | AssetEvent::Modified { id }
      | AssetEvent::Removed { id }
      | AssetEvent::Unused { id }
      | AssetEvent::LoadedWithDependencies { id } => id,
    })
    .collect::<HashSet<_>>();
  if archipelagos.iter_mut().any(|(_, archipelago)| archipelago.is_added()) {
    check.extend(islands.iter().map(|(entity, ..)| entity));
    for (_, archipelago) in archipelagos.iter() {
      check.extend(archipelago.islands.keys().copied());
    }
  } else if !meshes.is_empty() {
    check.extend(
      islands
        .iter()
        .filter(|(_, handle, _)| meshes.contains(&handle.0.id()))
        .map(|(entity, ..)| entity),
    );
  }
  // In entity order, so islands placed in one step take their ids the same
  // way every time.
  check.sort();
  check.dedup();
  for entity in check {
    if !sync_island(
      entity,
      &mut archipelagos,
      &islands,
      &transform_helper,
      &nav_meshes,
    ) {
      waiting.insert(entity);
    }
  }
}

/// One island put into its archipelago as it is now, or taken out of every
/// archipelago it is no longer placed in. False when the island exists but
/// cannot be placed yet (no archipelago, no mesh, no transform), so it is
/// looked at again next step.
fn sync_island<CS: CoordinateSystem>(
  island_entity: Entity,
  archipelagos: &mut Query<(Entity, &mut Archipelago<CS>)>,
  islands: &IslandQuery<CS>,
  transform_helper: &TransformHelper,
  nav_meshes: &Assets<NavMesh<CS>>,
) -> bool {
  let island = islands.get(island_entity).ok();
  let placed = island.and_then(|(_, island_nav_mesh, archipelago_ref)| {
    if !archipelagos.contains(archipelago_ref.entity) {
      return None;
    }
    let island_nav_mesh = nav_meshes.get(&island_nav_mesh.0)?;
    let island_transform =
      transform_helper.compute_global_transform(island_entity).ok()?;
    Some((archipelago_ref.entity, island_nav_mesh, island_transform))
  });

  for (entity, mut archipelago) in archipelagos.iter_mut() {
    if placed.is_some_and(|(placed_in, ..)| placed_in == entity)
      || !archipelago.islands.contains_key(&island_entity)
    {
      continue;
    }
    let archipelago = archipelago.as_mut();
    if let Some(id) = archipelago.islands.remove(&island_entity) {
      archipelago.archipelago.remove_island(id);
      archipelago.reverse_islands.remove(&id);
    }
  }

  let Some((archipelago_entity, island_nav_mesh, island_transform)) = placed
  else {
    return island.is_none();
  };
  let Ok((_, mut archipelago)) = archipelagos.get_mut(archipelago_entity)
  else {
    return false;
  };

  let island_transform = island_transform.compute_transform();
  let landmass_transform = landmass::Transform {
    translation: CS::from_bevy_position(island_transform.translation),
    rotation: CS::from_bevy_rotation(&island_transform.rotation),
  };

  match archipelago.get_island_mut(island_entity) {
    None => {
      let island_id =
        archipelago.archipelago.add_island(landmass::Island::new(
          landmass_transform,
          island_nav_mesh.nav_mesh.clone(),
        ));
      archipelago.islands.insert(island_entity, island_id);
      archipelago.reverse_islands.insert(island_id, island_entity);
    }
    Some(mut island) => {
      if island.get_transform() != &landmass_transform {
        island.set_transform(landmass_transform);
      }
      if !Arc::ptr_eq(&island.get_nav_mesh(), &island_nav_mesh.nav_mesh) {
        island.set_nav_mesh(island_nav_mesh.nav_mesh.clone());
      }
    }
  };
  true
}
