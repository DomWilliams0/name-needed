use crate::chunk::slab::Slab;

use crate::loader::batch::UpdateBatcher;

use crate::loader::LoadedSlab;
use crate::navigation::AreaNavEdge;
use crate::neighbour::NeighbourOffset;
use crate::{BaseTerrain, OcclusionChunkUpdate, WorldArea, WorldRef};
use common::*;
use futures::channel::mpsc as async_channel;
use std::cell::{Cell, RefCell};

use crate::chunk::WhichChunk;
use std::mem::MaybeUninit;
use std::ops::DerefMut;
use unit::world::{ChunkLocation, SlabLocation};

const SEND_FAILURE_THRESHOLD: usize = 20;

pub struct ChunkFinalizer<D> {
    world: WorldRef<D>,
    updates: async_channel::UnboundedSender<OcclusionChunkUpdate>,
    batcher: UpdateBatcher<LoadedSlab>,
    send_failures: usize,
}

struct FinalizeBatchItem {
    slab: SlabLocation,
    terrain: RefCell<MaybeUninit<Slab>>,
    consumed: Cell<bool>,
}

impl<D> ChunkFinalizer<D> {
    pub fn new(
        world: WorldRef<D>,
        updates: async_channel::UnboundedSender<OcclusionChunkUpdate>,
    ) -> Self {
        Self {
            world,
            updates,
            batcher: UpdateBatcher::default(),
            send_failures: 0,
        }
    }

    pub fn finalize(&mut self, slab: LoadedSlab) {
        // world lock is taken and released often to prevent holding up the main thread

        debug!(
            "submitting completed slab for finalization";
            slab.slab, slab.batch,
        );

        self.batcher.submit(slab.batch, slab);

        // finalize completed batches only, which might not include this update
        for (batch_id, batch_size) in self.batcher.complete_batches() {
            trace!("popping batch"; "id" => batch_id, "size" => batch_size);

            // we know that all dependent slabs and chunks (those in the same batch) are present now
            let (batch, mut items) = self.batcher.pop_batch(batch_id);
            debug_assert_eq!(items.len(), batch_size);

            // sort and group slabs by chunk then slab
            items.sort_unstable_by(|a, b| {
                a.slab
                    .chunk
                    .cmp(&b.slab.chunk)
                    .then_with(|| a.slab.slab.cmp(&b.slab.slab))
            });

            log_scope!(o!(batch));

            let mut chunks = SmallVec::<[ChunkLocation; 8]>::new();

            // put slabs into their respective chunks
            for (chunk, slabs) in items
                .into_iter()
                .group_by(|item| item.slab.chunk)
                .into_iter()
            {
                log_scope!(o!(chunk));
                debug!("populating chunk with slabs");
                let mut world = self.world.borrow_mut();
                world.populate_chunk_with_slabs(chunk, slabs);
                chunks.push(chunk);
            }

            // finalize one chunk at a time
            for chunk in chunks.iter() {
                debug!("finalizing"; chunk);
                self.finalize_chunk(*chunk);
            }
        }
    }

    fn finalize_chunk(&mut self, chunk: ChunkLocation) {
        log_scope!(o!(chunk));

        // navigation
        let area_edges = self.finalize_chunk_navigation(chunk);

        // occlusion
        self.finalize_occlusion(chunk);

        // let chunk = Chunk::with_completed_terrain(chunk, terrain);
        {
            // finally take WorldRef write lock and update chunk
            let mut world = self.world.borrow_mut();
            debug!("adding completed chunk to world");
            debug!("{} edges", area_edges.len());
            world.finalize_chunk(chunk, &area_edges);
        }
    }

    fn finalize_chunk_navigation(
        &mut self,
        chunk: ChunkLocation,
    ) -> Vec<(WorldArea, WorldArea, AreaNavEdge)> {
        let mut area_edges = Vec::new(); // TODO reuse buf

        for (direction, offset) in NeighbourOffset::aligned() {
            let world = self.world.borrow();

            let neighbour = chunk + offset;
            let neighbour_terrain = match world.find_chunk_with_pos(neighbour) {
                Some(chunk) => chunk.raw_terrain(),
                None => continue, // chunk is not loaded
            };

            let this_terrain = world.find_chunk_with_pos(chunk).unwrap(); // should be present

            let mut links = Vec::new(); // TODO reuse buf
            let mut ports = Vec::new(); // TODO reuse buf
                                        // TODO is it worth combining occlusion+nav by doing cross chunk iteration only once?
            this_terrain.raw_terrain().cross_chunk_pairs_nav_foreach(
                neighbour_terrain,
                direction,
                |src_area, dst_area, edge_cost, i, z| {
                    trace!("adding cross-chunk link to neighbour {neighbour:?}",
                        neighbour = neighbour; "to_area" => ?dst_area,
                        "from_area" => ?src_area, "direction" => ?direction, "xy" => i, "z" => ?z
                    );

                    let src_area = src_area.into_world_area(chunk);
                    let dst_area = dst_area.into_world_area(neighbour);

                    links.push((src_area, dst_area, edge_cost, i, z));
                },
            );

            links.sort_unstable_by_key(|(_, _, _, i, _)| *i);

            for ((src_area, dst_area), group) in links
                .iter()
                .group_by(|(src, dst, _, _, _)| (src, dst))
                .into_iter()
            {
                let direction = NeighbourOffset::between_aligned(src_area.chunk, dst_area.chunk);

                AreaNavEdge::discover_ports_between(
                    direction,
                    group.map(|(_, _, cost, idx, z)| (*cost, *idx, *z)),
                    &mut ports,
                );
                for edge in ports.drain(..) {
                    area_edges.push((*src_area, *dst_area, edge));
                }
            }
        }

        area_edges
    }
    fn finalize_occlusion(&mut self, chunk: ChunkLocation) {
        // TODO limit checks to range around the actual changes
        for (direction, offset) in NeighbourOffset::offsets() {
            let world = self.world.borrow();

            let neighbour = chunk + offset;
            let neighbour_terrain = match world.find_chunk_with_pos(neighbour) {
                Some(terrain) => terrain,
                // chunk is not loaded
                None => continue,
            };

            let this_terrain = world.find_chunk_with_pos(chunk).unwrap(); // should be present

            // TODO reuse/pool bufs, and initialize with proper expected size
            let mut this_terrain_updates = Vec::with_capacity(1200);
            let mut other_terrain_updates = Vec::with_capacity(1200);

            this_terrain.raw_terrain().cross_chunk_pairs_foreach(
                neighbour_terrain.raw_terrain(),
                direction,
                |which, block_pos, opacity| {
                    // TODO is it worth attempting to filter out updates that have no effect during the loop, or keep filtering them during consumption instead
                    match which {
                        WhichChunk::ThisChunk => {
                            // update opacity now for this chunk being loaded
                            this_terrain_updates.push((block_pos, opacity));
                        }
                        WhichChunk::OtherChunk => {
                            // queue opacity changes for the other chunk
                            other_terrain_updates.push((block_pos, opacity));
                        }
                    }
                },
            );

            // queue occlusion updates for next tick
            macro_rules! queue_updates {
                ($updates:expr, $chunk:expr) => {
                let updates = $updates;
                let chunk = $chunk;

                if !updates.is_empty() {
                    debug!(
                        "queueing {count} occlusion updates to apply to {chunk:?} next tick",
                        count = updates.len(),
                        chunk = chunk,
                    );
                    let send_result = self.updates.unbounded_send(OcclusionChunkUpdate(
                        chunk,
                        updates,
                    ));

                    if let Err(err) = send_result {
                        self.send_failures += 1;
                        warn!(
                        "failed to send chunk update, this is error {errors}/{threshold}",
                        errors = self.send_failures,
                        threshold = SEND_FAILURE_THRESHOLD;
                        "error" => %err,
                    );

                        if self.send_failures >= SEND_FAILURE_THRESHOLD {
                            crit!("error threshold reached, panicking");
                            panic!("chunk finalization error threshold passed: {}", err)
                        }
                    }
                }

                };
            }

            queue_updates!(this_terrain_updates, chunk);
            queue_updates!(other_terrain_updates, neighbour);
        }
    }
}

impl FinalizeBatchItem {
    fn initialized(slab: SlabLocation, terrain: Slab) -> Self {
        Self {
            slab,
            terrain: RefCell::new(MaybeUninit::new(terrain)),
            consumed: Cell::new(false),
        }
    }

    fn consume(&self) -> (SlabLocation, Slab) {
        let was_consumed = self.consumed.replace(true);
        assert!(!was_consumed, "slab has already been consumed");

        // steal terrain
        let terrain = unsafe {
            let mut t = self.terrain.borrow_mut();
            std::mem::replace(t.deref_mut(), MaybeUninit::uninit()).assume_init()
        };

        (self.slab, terrain)
    }

    fn get(&self, slab_pos: SlabLocation) -> Option<&Slab> {
        if self.consumed.get() || self.slab != slab_pos {
            None
        } else {
            let terrain_ref = self.terrain.borrow();
            let slab = unsafe { &*terrain_ref.as_ptr() };
            Some(slab)
        }
    }
}

impl Debug for FinalizeBatchItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(
            f,
            "FinalizeBatchItem(slab={}, consumed={:?})",
            self.slab,
            self.consumed.get()
        )
    }
}
