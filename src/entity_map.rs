//! The slot identity spine, taken from Endless: GpuSlotPool
//! allocates GPU slot indices (authoritative for buffer sizing and
//! dispatch bounds); EntityMap links slots to Bevy entities
//! (authoritative for identity and live counts). See the GPU
//! offload section of docs/authority.md.

use bevy::prelude::*;

/// Buffer sizing bound for every slot-indexed GPU buffer. Room for
/// the 1000 persistent people plus horde walkers, animals, and
/// headroom.
pub const MAX_ENTITIES: usize = 8192;

/// LIFO slot allocator. `count()` is the high-water mark.
#[derive(Resource, Default)]
pub struct GpuSlotPool {
    free: Vec<usize>,
    next: usize,
}

impl GpuSlotPool {
    pub fn alloc(&mut self) -> Option<usize> {
        if let Some(slot) = self.free.pop() {
            return Some(slot);
        }
        if self.next < MAX_ENTITIES {
            let slot = self.next;
            self.next += 1;
            Some(slot)
        } else {
            None
        }
    }

    pub fn free(&mut self, slot: usize) {
        self.free.push(slot);
    }

    /// High-water mark. The one source for buffer sizing and
    /// dispatch bounds, never for live counts (EntityMap owns those).
    pub fn count(&self) -> usize {
        self.next
    }
}

/// Slot-to-entity index. The one source for identity checks and
/// live counts.
#[derive(Resource, Default)]
pub struct EntityMap {
    slot_to_entity: Vec<Option<Entity>>,
    npc_count: usize,
}

impl EntityMap {
    pub fn insert_npc(&mut self, slot: usize, entity: Entity) {
        if self.slot_to_entity.len() <= slot {
            self.slot_to_entity.resize(slot + 1, None);
        }
        self.slot_to_entity[slot] = Some(entity);
        self.npc_count += 1;
    }

    pub fn remove_npc(&mut self, slot: usize) {
        if let Some(e) = self.slot_to_entity.get_mut(slot)
            && e.take().is_some()
        {
            self.npc_count -= 1;
        }
    }

    pub fn entity_for_slot(&self, slot: usize) -> Option<Entity> {
        self.slot_to_entity.get(slot).copied().flatten()
    }

    pub fn npc_count(&self) -> usize {
        self.npc_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freed_slot_is_reused_lifo() {
        let mut pool = GpuSlotPool::default();
        let a = pool.alloc().unwrap();
        let b = pool.alloc().unwrap();
        assert_ne!(a, b);
        pool.free(a);
        assert_eq!(pool.alloc().unwrap(), a);
        assert_eq!(pool.count(), 2);
    }

    #[test]
    fn count_is_high_water_mark_not_live_count() {
        let mut pool = GpuSlotPool::default();
        let a = pool.alloc().unwrap();
        pool.alloc().unwrap();
        pool.free(a);
        assert_eq!(pool.count(), 2);
    }

    #[test]
    fn entity_map_tracks_live_npc_count() {
        let mut map = EntityMap::default();
        let e = Entity::from_raw_u32(1).unwrap();
        map.insert_npc(5, e);
        assert_eq!(map.npc_count(), 1);
        assert_eq!(map.entity_for_slot(5), Some(e));
        map.remove_npc(5);
        assert_eq!(map.npc_count(), 0);
        assert_eq!(map.entity_for_slot(5), None);
        map.remove_npc(5);
        assert_eq!(map.npc_count(), 0);
    }
}
