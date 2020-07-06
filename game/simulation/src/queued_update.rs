use std::cell::RefCell;

use common::*;
use unit::world::WorldPosition;
use world::block::{BlockDurability, BlockType};
use world::loader::{TerrainUpdatesRes, WorldTerrainUpdate};
use world::BlockDamageResult;

use crate::ecs::EcsWorld;
use crate::ComponentWorld;

type Update = dyn FnOnce(&mut EcsWorld) -> Result<(), Box<dyn Error>>;
type Entry = (&'static str, Box<Update>);

pub struct QueuedUpdates {
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
        self.updates.borrow_mut().push((name, update))
    }

    pub fn execute(&mut self, world: &mut EcsWorld) {
        let mut vec = self.updates.borrow_mut();
        if !vec.is_empty() {
            debug!("running {} queued updates", vec.len());

            for (name, update) in vec.drain(..) {
                match update(world) {
                    Err(e) => warn!("queued update '{}' failed: {}", name, e),
                    Ok(_) => trace!("queued update '{}' was successful", name),
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
                terrain_updates.push(WorldTerrainUpdate::with_block(block, BlockType::Air))
            }

            Ok(())
        });
    }
}
