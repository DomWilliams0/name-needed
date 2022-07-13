use std::cell::RefCell;
use std::pin::Pin;

use common::*;
use unit::world::{WorldPosition, WorldPositionRange};
use world::block::BlockDurability;
use world::loader::WorldTerrainUpdate;
use world::BlockDamageResult;
use world_types::BlockType;

use crate::ecs::EcsWorld;
use crate::simulation::TerrainUpdatesRes;
use crate::{ComponentWorld};

pub type QueuedUpdates = RawQueuedUpdates<naive::NaiveImpl>;

// TODO perfect use case for a per-tick arena allocator
// TODO dynstack impl

pub struct RawQueuedUpdates<Q> {
    inner: RefCell<Q>,
}

pub trait QueuedUpdatesImpl: Default {
    /// Returns index of this new update
    fn queue<F: 'static + FnOnce(Pin<&mut EcsWorld>) -> Result<(), Box<dyn Error>>>(
        &mut self,
        name: &'static str,
        update: F,
    ) -> usize;

    fn len(&self) -> usize;

    fn drain_and_execute<N: FnMut(&'static str), R: FnMut(&'static str, BoxedResult<()>)>(
        &mut self,
        per_name: N,
        per_result: R,
        world: Pin<&mut EcsWorld>,
    );
}

impl<Q: Default> Default for RawQueuedUpdates<Q> {
    fn default() -> Self {
        Self {
            inner: RefCell::new(Q::default()),
        }
    }
}

impl<Q: QueuedUpdatesImpl> RawQueuedUpdates<Q> {
    pub fn queue<F: 'static + FnOnce(Pin<&mut EcsWorld>) -> Result<(), Box<dyn Error>>>(
        &self,
        name: &'static str,
        update: F,
    ) {
        let mut inner = self.inner.borrow_mut();
        let n = inner.queue(name, update);

        trace!("queued update #{n} for next tick", n = n; "name" => name)
    }

    pub fn execute(&mut self, world: Pin<&mut EcsWorld>) {
        let mut inner = self.inner.borrow_mut();
        let n = inner.len();
        if n > 0 {
            debug!("running {count} queued updates", count = n);

            let mut i = 0usize;
            inner.drain_and_execute(
                |name| {
                    // TODO try to use a slog scope here
                    debug!("executing queued update #{}", n; "name" => name);
                    i += 1;
                },
                |name, result| {
                    match result {
                        Err(e) => warn!("queued update failed"; "error" => %e, "name" => name),
                        Ok(_) => trace!("queued update was successful"; "name" => name),
                    };
                },
                world,
            );
        }

        debug_assert_eq!(inner.len(), 0);
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

mod naive {
    use super::*;

    /// Vec of boxes
    pub struct NaiveImpl(
        Vec<(
            &'static str,
            Box<dyn FnOnce(Pin<&mut EcsWorld>) -> BoxedResult<()>>,
        )>,
    );

    impl Default for NaiveImpl {
        fn default() -> Self {
            Self(Vec::with_capacity(256))
        }
    }

    impl QueuedUpdatesImpl for NaiveImpl {
        fn queue<F: 'static + FnOnce(Pin<&mut EcsWorld>) -> BoxedResult<()>>(
            &mut self,
            name: &'static str,
            update: F,
        ) -> usize {
            let old_len = self.0.len();
            self.0.push((name, Box::new(update)));
            old_len
        }

        fn len(&self) -> usize {
            self.0.len()
        }

        fn drain_and_execute<N: FnMut(&'static str), R: FnMut(&'static str, BoxedResult<()>)>(
            &mut self,
            mut per_name: N,
            mut per_result: R,
            mut world: Pin<&mut EcsWorld>,
        ) {
            for (name, update) in self.0.drain(..) {
                per_name(name);
                per_result(name, update(world.as_mut()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queued_update::naive::NaiveImpl;

    fn do_basic<Q: QueuedUpdatesImpl>() {
        let mut updates = RawQueuedUpdates::<Q>::default();

        let ecs = {
            let mut world = EcsWorld::new();
            world.insert(Vec::<i32>::new());
            world
        };

        updates.queue("nice", |w| {
            let results = w.resource_mut::<Vec<i32>>();
            results.push(123);
            info!("NICE!");
            Ok(())
        });

        updates.queue("cool", |w| {
            let results = w.resource_mut::<Vec<i32>>();
            results.push(456);
            info!("COOL!");
            Err("damn".into())
        });

        futures::pin_mut!(ecs);
        updates.execute(ecs.as_mut());

        let results = ecs.resource::<Vec<i32>>();
        assert_eq!(results, &vec![123, 456]);
    }

    #[test]
    fn basic_naive() {
        // logging::for_tests();
        do_basic::<NaiveImpl>()
    }
}
