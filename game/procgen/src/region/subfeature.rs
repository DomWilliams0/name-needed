//! Rasterization of features to actual blocks via subfeatures

use crate::{GeneratedBlock, PlanetParams, SlabGrid};
use common::*;

use crate::region::region::SlabContinuations;
use std::ops::DerefMut;
use std::sync::Arc;
use unit::world::{ChunkLocation, RangePosition, SlabLocation, SlabPosition, WorldPosition};

/// Rasterizable object that places blocks within a slab, possibly leaking over the edge into other
/// slabs. In case of seepage, the subfeature is queued as a continuation for the neighbour slab.
///
/// Note the neighbour slab could already be loaded!!
pub trait Subfeature: Send + Debug {
    // TODO pass in a "mask" of xyz ranges that can optionally be used to trim trying to place blocks in a neighbour
    fn rasterize(&mut self, root: WorldPosition, rasterizer: &mut Rasterizer);
}

/// Wrapper around an Arc<Mutex>
#[derive(Clone)]
pub struct SharedSubfeature(Arc<tokio::sync::Mutex<SubfeatureInner>>);

pub struct SubfeatureInner {
    /// A slab is added when this subfeature has been applied to it
    completed: SmallVec<[SlabLocation; 9]>,

    /// Root block in starting slab - all neighbours are relative to this slab
    root: WorldPosition,

    // TODO inline dyn subfeature or use pooled allocation
    subfeature: Box<dyn Subfeature>,
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
    neighbours: ArrayVec<SlabNeighbour, 9>,

    slab: SlabLocation,
}

impl Rasterizer {
    pub fn new(slab: SlabLocation) -> Self {
        Self {
            slab,
            this_slab: Vec::with_capacity(16),
            neighbours: ArrayVec::new(),
        }
    }

    pub fn place_block(&mut self, pos: WorldPosition, block: impl Into<GeneratedBlock>) {
        match resolve_slab(self.slab, pos) {
            None => self.this_slab.push((SlabPosition::from(pos), block.into())),
            Some(n) => {
                if !self.neighbours.contains(&n) {
                    self.neighbours.push(n);
                }
            }
        }
    }

    /// Call once
    pub fn internal_blocks(&mut self) -> impl Iterator<Item = (SlabPosition, GeneratedBlock)> {
        std::mem::take(&mut self.this_slab).into_iter()
    }

    pub fn touched_neighbours(&mut self) -> &[SlabNeighbour] {
        &self.neighbours
    }
}

impl SubfeatureInner {
    pub fn rasterize(&mut self, rasterizer: &mut Rasterizer) {
        self.subfeature.rasterize(self.root, rasterizer);
    }

    pub fn register_applied_slab(&mut self, slab: SlabLocation) {
        if self.has_already_applied_to(slab) {
            warn!("reregistering slab in subfeature as complete"; slab, &*self);
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
            subfeature: Box::new(subfeature),
        })))
    }

    pub async fn lock(&self) -> impl DerefMut<Target = SubfeatureInner> + '_ {
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
    ) {
        debug!("rasterizing subfeature {}", if continuations.is_some() {"with propagation"} else {"in isolation"}; slab, &self);
        // TODO if continuations is None, set a flag to ignore boundary leaks
        let mut rasterizer = Rasterizer::new(slab);

        // collect blocks from subfeature
        self.lock().await.rasterize(&mut rasterizer);

        // apply blocks within this slab
        let mut count = 0usize;
        for (pos, block) in rasterizer.internal_blocks() {
            let (x, y, z) = pos.xyz();
            let coord = [x as i32, y as i32, z];
            terrain[coord] = block;

            count += 1;
            trace!("placing block within slab"; slab, "pos" => ?pos.xyz(), "block" => ?block, &self);
        }

        let mut self_guard = self.lock().await;
        self_guard.register_applied_slab(slab);
        debug!("placed {count} blocks within slab", count = count; &self);

        if let Some(continuations) = continuations {
            // queue up blocks for other slabs
            let neighbours = rasterizer.touched_neighbours();
            if neighbours.is_empty() {
                // nothing to do
                return;
            }

            debug!("subfeature leaks into {count} neighbours", count = neighbours.len(); &self, "neighbours" => ?neighbours);

            for neighbour in neighbours.iter() {
                debug_assert_ne!(neighbour.0, [0, 0, 0]); // sanity check

                // find neighbour slab location
                let neighbour = match neighbour.offset(slab) {
                    Some(n) if params.is_chunk_in_range(n.chunk) => n,
                    _ => {
                        // TODO neighbour slab should wrap around the planet
                        debug!("neighbour slab is out of range"; slab, "offset" => ?neighbour, &self);
                        continue;
                    }
                };

                if self_guard.has_already_applied_to(neighbour) {
                    // TODO this is never hit?
                    debug!("skipping neighbour because subfeature has already been applied"; neighbour, &self);
                    continue;
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
                        // TODO handle this by queueing block updates to the already loaded chunk
                        warn!(
                            "neighbour slab is already loaded when applying subfeature";
                             "neighbour" => neighbour, &self,
                        );
                        continue;
                    }
                };
            }
        }
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
            self.subfeature, self.completed, self.root
        )
    }
}

impl Drop for SubfeatureInner {
    fn drop(&mut self) {
        debug!("goodbye from {:?}", self);
    }
}

slog_kv_debug!(SubfeatureInner, "subfeature");
slog_kv_debug!(SharedSubfeature, "subfeature");
