#![doc = include_str!("../README.md")]

use std::sync::Arc;

use bevy_app::{FixedPreUpdate, Plugin};
use bevy_asset::{AssetHandleProvider, AssetId, Assets, Handle};
use bevy_ecs::{
  bundle::Bundle,
  change_detection::DetectChangesMut,
  component::Component,
  intern::Interned,
  lifecycle::HookContext,
  resource::Resource,
  schedule::{ScheduleLabel, SystemSet},
  world::{DeferredWorld, World},
};
use bevy_landmass::{ArchipelagoRef3d, Island};
use bevy_platform::collections::{HashMap, hash_map::Entry};

mod raw_conversion;
pub use raw_conversion::convert_rerecast_navmesh_to_landmass_navmesh;

pub struct LandmassRerecastPlugin {
  schedule: Interned<dyn ScheduleLabel>,
}

impl Default for LandmassRerecastPlugin {
  fn default() -> Self {
    Self { schedule: FixedPreUpdate.intern() }
  }
}

impl LandmassRerecastPlugin {
  /// Sets the schedule for running the plugin. Defaults to
  /// [`FixedPreUpdate`].
  #[must_use]
  pub fn in_schedule(mut self, schedule: impl ScheduleLabel) -> Self {
    self.schedule = schedule.intern();
    self
  }
}

// topside patch: no conversion system. The game converts each mesh the
// moment it lands (`convert`) and drops a mapping when its island goes
// (`forget`), so a chunk's mesh and its landmass mesh land at the same
// tick, at a load too, with no asset event between them.
impl Plugin for LandmassRerecastPlugin {
  fn build(&self, app: &mut bevy_app::App) {
    let _ = self.schedule;
    app.init_resource::<RerecastToLandmassIds>();
  }
}

/// topside patch: the rerecast mesh `rerecast_id`, just landed, converted
/// into its island's landmass mesh now.
pub fn convert(world: &mut World, rerecast_id: AssetId<bevy_rerecast::Navmesh>) {
  let Some(landmass_id) = world.resource::<RerecastToLandmassIds>().get(rerecast_id) else {
    return;
  };
  let Some(rerecast_mesh) = world.resource_mut::<Assets<bevy_rerecast::Navmesh>>().remove(rerecast_id) else {
    return;
  };
  let landmass_mesh = match convert_rerecast_navmesh_to_landmass_navmesh(&rerecast_mesh).validate() {
    Ok(landmass_mesh) => landmass_mesh,
    Err(err) => {
      bevy_log::error!("Failed to validate rerecast mesh when converting to landmass: {err}");
      return;
    }
  };
  world
    .resource_mut::<Assets<bevy_landmass::NavMesh3d>>()
    .insert(landmass_id, bevy_landmass::NavMesh { nav_mesh: Arc::new(landmass_mesh) })
    .unwrap();
  // topside patch (todo 9n): the islands on this mesh are marked changed,
  // so bevy_landmass's island sync, which looks only at changed islands,
  // swaps a rebuilt mesh in this same tick.
  let mut handles = world.query::<&mut bevy_landmass::NavMeshHandle3d>();
  for mut handle in handles.iter_mut(world) {
    if handle.0.id() == landmass_id {
      handle.set_changed();
    }
  }
}

/// topside patch: the island of `rerecast_id` is gone; its landmass mesh is
/// no longer kept.
pub fn forget(world: &mut World, rerecast_id: AssetId<bevy_rerecast::Navmesh>) {
  world.resource_mut::<RerecastToLandmassIds>().remove(rerecast_id);
}

/// System set for systems converting between `landmass` and `rerecast`.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct LandmassRerecastSystems;

/// A replacement for [`bevy_landmass::Island3dBundle`] that uses a rerecast
/// mesh instead.
#[derive(Bundle)]
pub struct Island3dBundle {
  /// An island marker component.
  pub island: Island,
  /// A reference pointing to the Archipelago to associate this entity with.
  pub archipelago_ref: ArchipelagoRef3d,
  /// A handle to the nav mesh that this island needs.
  pub nav_mesh: NavMeshHandle3d,
}

/// A replacement for [`bevy_landmass::NavMeshHandle3d`] that stores a
/// [`bevy_rerecast::Navmesh`] handle.
#[derive(Component, Clone, Debug)]
#[component(immutable, on_insert=on_insert_rerecast_navmesh, on_discard=on_discard_rerecast_navmesh)]
pub struct NavMeshHandle3d(pub Handle<bevy_rerecast::Navmesh>);

// Due to https://github.com/rust-lang/rust/issues/73191, users could be using
// `bevy_landmass::NavMeshHandle` instead of `bevy_landmass::NavMeshHandle3d`.
// So we provide this type alias so it's easy to merge.
pub type NavMeshHandle = NavMeshHandle3d;

/// OnInsert hook for `NavMeshHandle3d` to insert an associated
/// `bevy_landmass::NavMeshHandle3d`.
fn on_insert_rerecast_navmesh(
  mut world: DeferredWorld,
  HookContext { entity, .. }: HookContext,
) {
  let rerecast_id =
    world.entity(entity).get::<crate::NavMeshHandle3d>().unwrap().0.id();
  let landmass_handles =
    world.resource::<Assets<bevy_landmass::NavMesh3d>>().get_handle_provider();

  // Keep track of the mapping from rerecast to landmass.
  // topside patch: no message to convert an already loaded mesh; the game
  // spawns each island before its mesh lands and converts it then.
  let (landmass_handle, _) = world
    .resource_mut::<RerecastToLandmassIds>()
    .get_or_alloc(rerecast_id, landmass_handles);

  // Insert a landmass handle so that landmass can see this entity.
  world
    .commands()
    .entity(entity)
    .insert(bevy_landmass::NavMeshHandle(landmass_handle));
}

/// OnReplace hook for `NavMeshHandle3d` to remove the associated
/// `bevy_landmass::NavMeshHandle3d`.
fn on_discard_rerecast_navmesh(
  mut world: DeferredWorld,
  HookContext { entity, .. }: HookContext,
) {
  let mut commands = world.commands();
  if let Ok(mut entity) = commands.get_entity(entity) {
    // Remove the landmass handle from this entity. It's ok if the user deleted
    // it though.
    entity.try_remove::<bevy_landmass::NavMeshHandle3d>();
  }
}

/// A resource to keep track of the mapping from rerecast nav meshes into their
/// landmass equivalents.
#[derive(Resource, Default)]
struct RerecastToLandmassIds(
  HashMap<AssetId<bevy_rerecast::Navmesh>, Handle<bevy_landmass::NavMesh3d>>,
);

impl RerecastToLandmassIds {
  /// Looks up the landmass handle for `rerecast_id` or allocates one if one
  /// isn't present.
  ///
  /// Returns the handle and a bool indicating whether the handle was found
  /// (false) or was allocated (true).
  // Note: we pass the raw handle provider since we need mutable access to this
  // resource in the hook.
  fn get_or_alloc(
    &mut self,
    rerecast_id: AssetId<bevy_rerecast::Navmesh>,
    landmass_handle_provider: AssetHandleProvider,
  ) -> (Handle<bevy_landmass::NavMesh3d>, bool) {
    match self.0.entry(rerecast_id) {
      Entry::Occupied(entry) => (entry.get().clone(), false),
      Entry::Vacant(entry) => {
        let handle = landmass_handle_provider.reserve_handle().typed();
        entry.insert(handle.clone());
        (handle, true)
      }
    }
  }

  /// Removes the mapping for `rerecast_id`.
  fn remove(&mut self, rerecast_id: AssetId<bevy_rerecast::Navmesh>) {
    self.0.remove(&rerecast_id);
  }

  /// Looks up the landmass mesh ID associated with the rerecast mesh ID.
  fn get(
    &self,
    rerecast_id: AssetId<bevy_rerecast::Navmesh>,
  ) -> Option<AssetId<bevy_landmass::NavMesh3d>> {
    self.0.get(&rerecast_id).map(|handle| handle.id())
  }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod test;
