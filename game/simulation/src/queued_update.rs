use std::cell::RefCell;

use common::*;
use unit::world::{WorldPosition, WorldPositionRange};
use world::block::{BlockDurability, BlockType};
use world::loader::{TerrainUpdatesRes, WorldTerrainUpdate};
use world::BlockDamageResult;

use crate::ecs::EcsWorld;
use crate::ComponentWorld;

type Update = dyn FnOnce(&mut EcsWorld) -> Result<(), Box<dyn Error>>;
type Entry = (&'static str, Box<Update>);

pub struct QueuedUpdates {
    // TODO use dynstack for updates to avoid a separate box per entry
    // TODO perfect use case for a per-tick arena allocator
    updates: RefCell<Vec<Entry>>,
}

impl Default for QueuedUpdates {
    fn default() -> Self {
        Self {
            updates: RefCell::new(Vec::with_capacity(256)),
        }
    }
}

impl QueuedUpdates {
    pub fn queue<F: 'static + FnOnce(&mut EcsWorld) -> Result<(), Box<dyn Error>>>(
        &self,
        name: &'static str,
        update: F,
    ) {
        // TODO pool/reuse these boxes
        let update = Box::new(update);
        let mut updates = self.updates.borrow_mut();
        let old_len = updates.len();
        updates.push((name, update));

        trace!("queued update #{n} for next tick", n = old_len; "name" => name)
    }

    pub fn execute(&mut self, world: &mut EcsWorld) {
        let mut vec = self.updates.borrow_mut();
        if !vec.is_empty() {
            debug!("running {count} queued updates", count = vec.len());

            for (name, update) in vec.drain(..) {
                log_scope!(o!("queued_update" => name));

                match update(world) {
                    Err(e) => warn!("queued update failed"; "error" => %e),
                    Ok(_) => trace!("queued update was successful"),
                }
            }
        }
    }

    pub fn queue_block_damage(&self, block: WorldPosition, damage: BlockDurability) {
        self.queue("damage block", move |world| {
            let world_ref = world.voxel_world();
            let mut voxel_world = world_ref.borrow_mut();

            if let Some(BlockDamageResult::Broken) = voxel_world.damage_block(block, damage) {
                let terrain_updates = world.resource_mut::<TerrainUpdatesRes>();
                terrain_updates.push(WorldTerrainUpdate::new(
                    WorldPositionRange::with_single(block),
                    BlockType::Air,
                ))
            }

            Ok(())
        });
    }
}
