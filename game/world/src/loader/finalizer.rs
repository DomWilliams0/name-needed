use crate::loader::batch::UpdateBatcher;

use crate::loader::LoadedSlab;
use crate::navigation::AreaNavEdge;
use crate::neighbour::NeighbourOffset;
use crate::{BaseTerrain, OcclusionChunkUpdate, WorldArea, WorldContext, WorldRef};
use common::*;
use futures::channel::mpsc as async_channel;

use crate::chunk::slice::unflatten_index;
use crate::chunk::WhichChunk;
use crate::occlusion::NeighbourOpacity;

use unit::world::{ChunkLocation, SlabIndex};

const SEND_FAILURE_THRESHOLD: usize = 20;

pub struct SlabFinalizer<C: WorldContext> {
    world: WorldRef<C>,
    updates: async_channel::UnboundedSender<OcclusionChunkUpdate>,
    batcher: UpdateBatcher<LoadedSlab>,
    send_failures: usize,
}

impl<C: WorldContext> SlabFinalizer<C> {
    pub fn new(
        world: WorldRef<C>,
        updates: async_channel::UnboundedSender<OcclusionChunkUpdate>,
    ) -> Self {
        Self {
            world,
            updates,
            batcher: UpdateBatcher::default(),
            send_failures: 0,
        }
    }

    pub async fn finalize(&mut self, slab: LoadedSlab) {
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
                    .then_with(|| a.slab.slab.is_negative().cmp(&b.slab.slab.is_negative()))
                    .then_with(|| a.slab.slab.cmp(&b.slab.slab))
            });

            // find min and max slabs in the whole set. this assumes the batch is a cuboid!
            let slab_range = items
                .iter()
                .minmax_by_key(|slab| slab.slab.slab)
                .into_option()
                .map(|(min, max)| (min.slab.slab, max.slab.slab))
                .unwrap(); // batch can't be empty

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
                world.populate_chunk_with_slabs(chunk, slab_range, slabs);
                chunks.push(chunk);
            }

            // finalize one chunk at a time
            for chunk in chunks.into_iter() {
                log_scope!(o!(chunk));
                self.finalize_chunk_between_slabs(chunk, slab_range).await;
            }
        }
    }

    /// Lower and upper slab range is inclusive
    async fn finalize_chunk_between_slabs(
        &mut self,
        chunk: ChunkLocation,
        slab_range: (SlabIndex, SlabIndex),
    ) {
        debug!("finalizing slab range in chunk"; "lower" => slab_range.0, "upper" => slab_range.1);

        // mark slabs complete
        {
            let world = self.world.borrow();
            if let Some(chunk) = world.find_chunk_with_pos(chunk) {
                let slabs = (slab_range.0.as_i32()..=slab_range.1.as_i32()).map(SlabIndex);
                chunk.mark_slabs_complete(slabs);
            }
        }

        // navigation
        let area_edges = self.finalize_chunk_navigation(chunk, slab_range);

        // occlusion
        self.finalize_occlusion(chunk, slab_range);

        {
            // finally take WorldRef write lock and update chunk
            let mut world = self.world.borrow_mut();
            debug!(
                "adding completed chunk to world with {area_edges} area edges",
                area_edges = area_edges.len()
            );
            world.finalize_chunk(chunk, &area_edges, slab_range);
        }
    }

    fn finalize_chunk_navigation(
        &mut self,
        chunk: ChunkLocation,
        slab_range: (SlabIndex, SlabIndex),
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
                slab_range,
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
    fn finalize_occlusion(&mut self, chunk: ChunkLocation, slab_range: (SlabIndex, SlabIndex)) {
        // TODO only propagate across chunk boundaries if the changes were near to a boundary?

        // propagate across chunk boundaries
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
                slab_range,
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

            drop(world);

            // queue occlusion updates for next tick
            // TODO prevent mesh being rendered if there are queued occlusion changes?
            self.send_update(OcclusionChunkUpdate(chunk, this_terrain_updates));
            self.send_update(OcclusionChunkUpdate(neighbour, other_terrain_updates));
        }

        // propagate across slab boundaries within chunk
        {
            let world = self.world.borrow();
            let this_terrain = world.find_chunk_with_pos(chunk).unwrap(); // should be present

            let chunk_updates = this_terrain
                .raw_terrain()
                .slab_boundary_slices()
                .flat_map(|(lower_slice_idx, lower, upper)| {
                    lower.into_iter().enumerate().filter_map(move |(i, b)| {
                        let this_block = b.opacity();
                        let block_above = (*upper)[i].opacity();

                        // this block should be solid and the one above it should not be
                        if this_block.solid() && block_above.transparent() {
                            let this_block = unflatten_index(i);

                            let block_pos = this_block.to_block_position(lower_slice_idx);
                            let opacity = NeighbourOpacity::with_slice_above(this_block, upper);
                            Some((block_pos, opacity))
                        } else {
                            None
                        }
                    })
                })
                .collect_vec();

            drop(world);
            self.send_update(OcclusionChunkUpdate(chunk, chunk_updates));
        }
    }

    fn send_update(&mut self, update: OcclusionChunkUpdate) {
        if update.1.is_empty() {
            return;
        }

        debug!(
            "queueing {count} occlusion updates to apply to {chunk:?} next tick",
            count = update.1.len(),
            chunk = update.0,
        );
        let send_result = self.updates.unbounded_send(update);

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
}
