//! Rasterization of features to actual blocks via subfeatures

use crate::{GeneratedBlock, PlanetParams, SlabGrid};
use common::*;

use crate::region::region::SlabContinuations;
use grid::GridImpl;
use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};
use unit::world::{
    ChunkLocation, RangePosition, SlabLocation, SlabPosition, SlabPositionAsCoord, WorldPosition,
};
use world_types::EntityDescription;

/// Rasterizable object that places blocks within a slab, possibly leaking over the edge into other
/// slabs. In case of seepage, the subfeature is queued as a continuation for the neighbour slab.
///
/// Note the neighbour slab could already be loaded!!
pub trait Subfeature: Send + Debug {
    // TODO pass in a "mask" of xyz ranges that can optionally be used to trim trying to place blocks in a neighbour
    fn rasterize(
        &mut self,
        root: WorldPosition,
        rasterizer: &mut Rasterizer,
    ) -> Option<SubfeatureEntity>;
}

/// Entity corresponding to a rasterized subfeature
pub struct SubfeatureEntity(pub EntityDescription);

/// Wrapper around an Arc<Mutex>
#[derive(Clone)]
pub struct SharedSubfeature(Arc<tokio::sync::Mutex<SubfeatureInner>>);

pub struct SubfeatureInner<F: ?Sized + Subfeature = dyn Subfeature> {
    /// A slab is added when this subfeature has been applied to it
    // TODO reduce smallvec inline size, this is excessive and never spills
    completed: SmallVec<[SlabLocation; 9]>,

    /// Root block in starting slab - all neighbours are relative to this slab
    root: WorldPosition,

    subfeature: F,
}

pub enum SlabContinuation {
    // TODO use dynstack here
    Unloaded(Vec<SharedSubfeature>),
    Loaded,
}

#[derive(Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Debug)]
pub struct SlabNeighbour([i8; 3]);

/// Subfeatures call into a Rasterizer to place blocks, so the internals can change transparently
/// in the future
pub struct Rasterizer {
    // TODO reuse borrowed vec allocation
    this_slab: Vec<(SlabPosition, GeneratedBlock)>,

    /// Set of neighbours touched by protruding blocks
    neighbours: ArrayVec<SlabNeighbour, 9>,

    /// Blocks that protrude into other ALREADY LOADED neighbour slabs
    other_blocks: Vec<(SlabNeighbour, WorldPosition, GeneratedBlock)>,

    slab: SlabLocation,
}

impl Rasterizer {
    pub fn new(slab: SlabLocation) -> Self {
        Self {
            slab,
            this_slab: Vec::with_capacity(16),
            neighbours: ArrayVec::new(),
            other_blocks: Vec::new(),
        }
    }

    pub fn place_block(&mut self, pos: WorldPosition, block: impl Into<GeneratedBlock>) {
        let block = block.into();
        match resolve_slab(self.slab, pos) {
            None => self.this_slab.push((SlabPosition::from(pos), block)),
            Some(n) => {
                if !self.neighbours.contains(&n) {
                    self.neighbours.push(n);
                }
                self.other_blocks.push((n, pos, block));
            }
        }
    }

    /// Don't expect any more calls to place_block
    pub fn finish(&mut self) {
        // sort by slab neighbour for efficient removal later
        self.other_blocks.sort_unstable_by_key(|(n, _, _)| *n);
    }

    /// Call once
    pub fn internal_blocks(&mut self) -> impl Iterator<Item = (SlabPosition, GeneratedBlock)> {
        std::mem::take(&mut self.this_slab).into_iter()
    }

    /// Blocks are not removed from the underlying vec, to avoid reshuffling
    pub fn protruding_blocks(
        &self,
        neighbour: SlabNeighbour,
    ) -> impl Iterator<Item = (WorldPosition, GeneratedBlock)> + '_ {
        // other_blocks expected to be sorted by slab neighbour
        self.other_blocks
            .iter()
            .skip_while(move |(n, _, _)| *n != neighbour) // skip until start of contiguous region
            .take_while(move |(n, _, _)| *n == neighbour) // take until end of region
            .map(|(_, pos, block)| (*pos, *block))
    }

    pub fn touched_neighbours(&self) -> &[SlabNeighbour] {
        &self.neighbours
    }
}

impl SubfeatureInner {
    pub fn rasterize(&mut self, rasterizer: &mut Rasterizer) -> Option<SubfeatureEntity> {
        self.subfeature.rasterize(self.root, rasterizer)
    }

    pub fn register_applied_slab(&mut self, slab: SlabLocation) {
        if self.has_already_applied_to(slab) {
            warn!("reregistering slab in subfeature as complete"; slab, self as &Self);
        } else {
            self.completed.push(slab);
        }
    }

    fn has_already_applied_to(&self, slab: SlabLocation) -> bool {
        self.completed.contains(&slab)
    }
}

/// None if within this slab, Some(diff) if within a neighbour. Direction is slab->neighbour
///
/// TODO handle case where block is multiple slabs over from root slab
fn resolve_slab(slab: SlabLocation, block: WorldPosition) -> Option<SlabNeighbour> {
    // (chunk x, chunk y, slab index)
    let [bx, by, bz]: [i32; 3] = {
        let z = block.slice().slab_index().as_i32();
        let (x, y) = ChunkLocation::from(block).xy();
        [x, y, z]
    };

    let [sx, sy, sz]: [i32; 3] = [slab.chunk.x(), slab.chunk.y(), slab.slab.as_i32()];

    // diff in this slab->block slab direction
    let diff = [bx - sx, by - sy, bz - sz];
    debug_assert!(
        diff.iter().all(|d| d.abs() <= 1),
        "slab is not adjacent (slab={:?}, block={:?}, diff={:?})",
        slab,
        block,
        diff
    );

    match diff {
        [0, 0, 0] => None,
        [dx, dy, dz] => Some(SlabNeighbour([dx as i8, dy as i8, dz as i8])),
    }
}

impl SlabNeighbour {
    /// None if new slab is out of range of the absolute world limits (not planet)
    pub fn offset(self, slab: SlabLocation) -> Option<SlabLocation> {
        if let Some(chunk) = slab.chunk.try_add((self.0[0] as i32, self.0[1] as i32)) {
            if let Some(slab) = slab.slab.try_add(self.0[2] as i32) {
                return Some(SlabLocation::new(slab, chunk));
            }
        }
        None
    }
}

impl Default for SlabContinuation {
    fn default() -> Self {
        Self::Unloaded(Vec::new())
    }
}

impl SharedSubfeature {
    pub fn new(subfeature: impl Subfeature + 'static, root: WorldPosition) -> Self {
        Self(Arc::new(tokio::sync::Mutex::new(SubfeatureInner {
            completed: SmallVec::new(),
            root,
            subfeature,
        })))
    }

    pub async fn lock(&self) -> MutexGuard<'_, SubfeatureInner> {
        self.0.lock().await
    }

    /// continuations=Some: first time running this on the root slab, will propagate subfeature to
    ///                     neighbouring slabs
    /// continuations=None: applying after being leaked from the root slab, don't propagate
    pub async fn apply(
        self,
        slab: SlabLocation,
        terrain: &mut SlabGrid,
        continuations: Option<&mut SlabContinuations>,
        params: &PlanetParams,
        protruding_blocks: &Arc<Mutex<Vec<(WorldPosition, GeneratedBlock)>>>,
    ) -> Option<SubfeatureEntity> {
        debug!("rasterizing subfeature {}", if continuations.is_some() {"with propagation"} else {"in isolation"}; slab, &self);
        // TODO if continuations is None, set a flag to ignore boundary leaks
        let mut rasterizer = Rasterizer::new(slab);

        // collect blocks and potential entity from subfeature
        let entity = self.lock().await.rasterize(&mut rasterizer);
        rasterizer.finish();

        // apply blocks within this slab
        let mut count = 0usize;
        for (pos, block) in rasterizer.internal_blocks() {
            *terrain.get_unchecked_mut(SlabPositionAsCoord(pos)) = block;

            count += 1;
            trace!("placing block within slab"; slab, "pos" => ?pos.xyz(), "block" => ?block, &self);
        }

        self.lock().await.register_applied_slab(slab);
        if count > 0 {
            debug!("placed {count} blocks within slab", count = count; &self);
        }

        if let Some(continuations) = continuations {
            // queue up blocks for other slabs
            let neighbours = rasterizer.touched_neighbours();
            if neighbours.is_empty() {
                // nothing to do
                return entity;
            }

            debug!("subfeature leaks into {count} neighbours", count = neighbours.len(); &self, "neighbours" => ?neighbours);

            for neighbour_offset in neighbours.iter() {
                debug_assert_ne!(neighbour_offset.0, [0, 0, 0]); // sanity check

                // find neighbour slab location
                let neighbour = match neighbour_offset.offset(slab) {
                    Some(n) if params.is_chunk_in_range(n.chunk) => n,
                    _ => {
                        // TODO neighbour slab should wrap around the planet
                        debug!("neighbour slab is out of range"; slab, "offset" => ?neighbour_offset, &self);
                        continue;
                    }
                };

                if cfg!(debug_assertions) {
                    // shouldn't happen
                    let self_guard = self.lock().await;
                    assert!(!self_guard.has_already_applied_to(neighbour));
                }

                // add to neighbour's continuations
                use SlabContinuation::*;

                let mut continuations_guard = continuations.lock().await;
                match continuations_guard
                    .entry(neighbour)
                    .or_insert_with(SlabContinuation::default)
                {
                    Unloaded(subfeatures) => {
                        debug!(
                            "adding subfeature to unloaded neighbour's continuations";
                             "neighbour" => ?neighbour, &self,
                        );
                        subfeatures.push(self.clone());
                    }
                    Loaded => {
                        // push block updates to apply to already-loaded neighbour slab
                        let block_updates = rasterizer.protruding_blocks(*neighbour_offset);
                        let mut protruding_blocks = protruding_blocks.lock().await;
                        let len_before = protruding_blocks.len();
                        protruding_blocks.extend(block_updates);

                        debug!(
                            "neighbour slab is already loaded when applying subfeature, queueing {count} block updates",
                            count = protruding_blocks.len() - len_before; "neighbour" => neighbour, &self,
                        );
                    }
                };
            }
        }

        entity
    }
}

impl Debug for SharedSubfeature {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // TODO beware that subfeatures dont live for long so the pointer is likely to be reused
        write!(f, "SubfeatureRef({:?}, ", Arc::as_ptr(&self.0))?;
        if let Ok(inner) = self.0.try_lock() {
            write!(f, "{:?}", inner)?;
        } else {
            write!(f, "<locked>")?;
        }

        write!(f, ")")
    }
}

impl Debug for SubfeatureInner {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Subfeature({:?}, completed={:?}, root={:?})",
            &self.subfeature, self.completed, self.root
        )
    }
}

slog_kv_debug!(&SubfeatureInner, "subfeature");
slog_kv_debug!(SharedSubfeature, "subfeature");
