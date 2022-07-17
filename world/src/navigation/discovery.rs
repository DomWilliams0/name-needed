use std::collections::HashMap;

use common::*;
use unit::world::{BlockCoord, SlabIndex, SlabPositionAsCoord, SLAB_SIZE};
use unit::world::{LocalSliceIndex, RangePosition, SlabPosition, CHUNK_SIZE};

use crate::block::Block;
use crate::chunk::slab::SlabGridImpl;
use crate::chunk::slice::Slice;
use crate::navigation::{BlockGraph, ChunkArea, EdgeCost, SlabAreaIndex};
use crate::neighbour::SlabNeighbours;
use crate::occlusion::OcclusionOpacity;
use crate::WorldContext;
use grid::{grid_declare, GridImpl};
use std::ops::Deref;

grid_declare!(struct AreaDiscoveryGrid<AreaDiscoveryGridImpl, AreaDiscoveryGridBlock>,
    CHUNK_SIZE.as_usize(),
    CHUNK_SIZE.as_usize(),
    SLAB_SIZE.as_usize()
);

#[derive(Default, Copy, Clone)]
struct AreaDiscoveryGridBlock {
    opacity: OcclusionOpacity,

    area: SlabAreaIndex,
}

#[derive(Default)]
pub(crate) struct AreaDiscovery<'a, C: WorldContext> {
    grid: AreaDiscoveryGrid,

    /// flood fill queue, pair of (pos, pos this was reached from) TODO share between slabs
    queue: Vec<(SlabPosition, Option<(SlabPosition, EdgeCost)>)>,

    /// current area index to flood fill with
    current: SlabAreaIndex,

    /// all areas in this slab collected during discovery
    areas: Vec<ChunkArea>,

    /// all block graphs collected during discovery
    block_graphs: HashMap<ChunkArea, BlockGraph>,

    slab_index: SlabIndex,

    below_top_slice: Option<Slice<'a, C>>,
}

impl<C: WorldContext> From<&Block<C>> for AreaDiscoveryGridBlock {
    fn from(block: &Block<C>) -> Self {
        AreaDiscoveryGridBlock {
            opacity: block.opacity().into(),
            area: Default::default(),
        }
    }
}

#[derive(Eq, PartialEq)]
enum VerticalOffset {
    Above,
    Below,
}

impl<'a, C: WorldContext> AreaDiscovery<'a, C> {
    pub fn from_slab(
        slab: &impl Deref<Target = SlabGridImpl<C>>,
        slab_index: SlabIndex,
        below_top_slice: Option<Slice<'a, C>>,
    ) -> Self {
        let mut grid = AreaDiscoveryGrid::default();

        for i in grid.indices() {
            // indices are certainly valid - TODO unchecked unwrap
            let b = slab.index(i).unwrap();
            *(grid.index_mut(i).unwrap()) = b.into();
        }

        Self {
            grid,
            queue: Vec::new(),
            current: SlabAreaIndex::FIRST,
            areas: Vec::new(),
            block_graphs: HashMap::new(),
            slab_index,
            below_top_slice,
        }
    }

    /// Flood fills from every block, increment area index after each flood fill
    /// Returns area count
    pub fn flood_fill_areas(&mut self) -> u16 {
        let range = {
            let (s0_start, _) = self.grid.slice_range(0);
            let end = AreaDiscoveryGrid::FULL_SIZE;
            s0_start..end
        };

        for idx in range {
            let block = self.grid.index(idx).unwrap(); // certainly valid
            if !block.area.initialized() {
                let coord = AreaDiscoveryGrid::unflatten_panic(idx);
                self.do_flood_fill(coord);
            }
        }

        let SlabAreaIndex(area) = self.current;
        area - 1
    }

    fn do_flood_fill(&mut self, start: SlabPositionAsCoord) {
        let mut count = 0;

        self.queue.clear();
        self.queue.push((*start, None));
        let mut graph = BlockGraph::new();

        while let Some((current, src)) = self.queue.pop() {
            let check_neighbours = match self
                .grid
                .get_unchecked(SlabPositionAsCoord(current))
                .area
                .ok()
            {
                None => {
                    // not seen before, check for walkability
                    if !self.is_walkable(current) {
                        continue;
                    }

                    // then check neighbours
                    true
                }
                Some(a) if a == self.current => {
                    // seen before and in the same area, make edge only
                    false
                }
                Some(_) => {
                    // seen before but in another area, skip
                    continue;
                }
            };

            // create edges
            if let Some((src, src_cost)) = src {
                graph.add_edge(src, current, src_cost, self.slab_index);
            }

            if !check_neighbours {
                // we were only adding an edge here so we're done here
                continue;
            }

            // assign area
            self.grid
                .get_unchecked_mut(SlabPositionAsCoord(current))
                .area = self.current;
            count += 1;

            // add horizontal neighbours
            for n in SlabNeighbours::new(current) {
                let cost = EdgeCost::Walk;
                let src = Some((current, cost));
                self.queue.push((n, src));
            }

            // check vertical neighbours for jump access

            // don't queue the slab above's neighbours if we're at the top of the slab
            if current.z().slice() < SLAB_SIZE.as_i32() - 1 {
                // only check for jump ups if the block directly above is not solid
                if self
                    .get_vertical_offset(current, VerticalOffset::Above)
                    .opacity
                    .transparent()
                {
                    let (x, y, z) = current.xyz();
                    // xy come from an existing slab position, and we've checked z is not at the top
                    let above =
                        SlabPosition::new_unchecked(x, y, LocalSliceIndex::new_unchecked(z + 1));

                    for n in SlabNeighbours::new(above) {
                        self.queue.push((n, Some((current, EdgeCost::JumpUp))));
                    }
                }
            }

            // don't queue the slab below's neighbours if we're at the bottom of the slab
            if current.z().slice() > 0 {
                for n_adjacent in SlabNeighbours::new(current) {
                    // only check for jump downs if the block directly above that is not solid
                    // (mirrored check for jump ups above)

                    let adjacent = self.grid.get_unchecked_mut(SlabPositionAsCoord(n_adjacent));
                    if adjacent.opacity.transparent() {
                        let (x, y, z) = n_adjacent.xyz();
                        // xy come from an existing slab position, and we've checked z is not at the bottom
                        let n_below = SlabPosition::new_unchecked(
                            x,
                            y,
                            LocalSliceIndex::new_unchecked(z - 1),
                        );
                        self.queue
                            .push((n_below, Some((current, EdgeCost::JumpDown))));
                    }
                }
            }
        }

        // increment area
        if count > 0 {
            let area = ChunkArea {
                slab: self.slab_index,
                area: self.current,
            };

            self.areas.push(area);
            self.current.increment();

            // store graph
            self.block_graphs.insert(area, graph);
            debug!("area has {count} blocks", count = count; "area" => ?area);
        }
    }

    fn is_walkable(&self, pos: SlabPosition) -> bool {
        let marker = self.grid.get_unchecked(SlabPositionAsCoord(pos));

        if marker.opacity.solid() {
            return false;
        }

        let below = self.get_vertical_offset(pos, VerticalOffset::Below);

        // below not solid either: nope
        if below.opacity.transparent() {
            return false;
        }

        // below is solid and full: nice
        true
    }

    /// Can check below into slab below, but not above into slab above
    fn get_vertical_offset(
        &self,
        block: SlabPosition,
        offset: VerticalOffset,
    ) -> AreaDiscoveryGridBlock {
        let (x, y, z) = block.xyz();
        const TOP: i32 = SLAB_SIZE.as_i32() - 1;

        match z {
            // top of the slab: never check the slab above
            TOP if offset == VerticalOffset::Above => unreachable!(),

            // bottom of the slab: check slab below
            0 if offset == VerticalOffset::Below => {
                if let Some(below_slice) = &self.below_top_slice {
                    // it is present
                    (&below_slice[(x as BlockCoord, y as BlockCoord)]).into()
                } else {
                    // not present: this must be the bottom of the world
                    AreaDiscoveryGridBlock {
                        opacity: OcclusionOpacity::Unknown,
                        ..Default::default()
                    }
                }
            }

            // not top or bottom, just get the block
            z => {
                let offset_z = match offset {
                    VerticalOffset::Above => z + 1,
                    VerticalOffset::Below => z - 1,
                };

                let pos =
                    SlabPosition::new_unchecked(x, y, LocalSliceIndex::new_unchecked(offset_z));
                *self.grid.get_unchecked(SlabPositionAsCoord(pos))
            }
        }
    }

    /// Moves area->block graphs map out of self
    pub fn areas_with_graph(&mut self) -> impl Iterator<Item = (ChunkArea, BlockGraph)> {
        let block_graphs = std::mem::take(&mut self.block_graphs);
        block_graphs.into_iter()
    }

    /// Assign areas to the blocks in the slab
    pub fn apply(self, slab: &mut SlabGridImpl<C>) {
        for i in slab.indices() {
            // indices are valid
            // TODO use unchecked unwrap here
            let src = self.grid.index(i).unwrap();
            let dst = slab.index_mut(i).unwrap();
            *dst.area_mut() = src.area;
        }
    }
}
