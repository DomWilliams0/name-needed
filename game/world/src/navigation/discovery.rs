use std::collections::HashMap;

use common::*;
use unit::world::CHUNK_SIZE;
use unit::world::{BlockCoord, SlabIndex, SLAB_SIZE};

use crate::block::Block;
use crate::chunk::slab::SlabGridImpl;
use crate::chunk::slice::Slice;
use crate::navigation::{BlockGraph, ChunkArea, EdgeCost, SlabAreaIndex};
use crate::neighbour::SlabNeighbours;
use crate::occlusion::OcclusionOpacity;
use grid::{grid_declare, CoordType, GridImpl};
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
pub(crate) struct AreaDiscovery<'a> {
    grid: AreaDiscoveryGrid,

    /// flood fill queue, pair of (pos, pos this was reached from) TODO share between slabs
    queue: Vec<(CoordType, Option<(CoordType, EdgeCost)>)>,

    /// current area index to flood fill with
    current: SlabAreaIndex,

    /// all areas in this slab collected during discovery
    areas: Vec<ChunkArea>,

    /// all block graphs collected during discovery
    block_graphs: HashMap<ChunkArea, BlockGraph>,

    slab_index: SlabIndex,

    below_top_slice: Option<Slice<'a>>,
}

impl From<&Block> for AreaDiscoveryGridBlock {
    fn from(block: &Block) -> Self {
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

impl<'a> AreaDiscovery<'a> {
    pub fn from_slab(
        slab: &impl Deref<Target = SlabGridImpl>,
        slab_index: SlabIndex,
        below_top_slice: Option<Slice<'a>>,
    ) -> Self {
        let mut grid = AreaDiscoveryGrid::default();

        for i in grid.indices() {
            let b: &Block = &slab[i];
            grid[i] = b.into();
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
            if !self.grid[idx].area.initialized() {
                self.do_flood_fill(idx);
            }
        }

        let SlabAreaIndex(area) = self.current;
        area - 1
    }

    fn do_flood_fill(&mut self, start: usize) {
        let mut count = 0;

        self.queue.clear();
        self.queue.push((self.grid.unflatten(start), None));
        let mut graph = BlockGraph::new();

        while let Some((current, src)) = self.queue.pop() {
            let check_neighbours = match self.grid[&current].area.ok() {
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
                graph.add_edge(&src, &current, src_cost, self.slab_index);
            }

            if !check_neighbours {
                // we were only adding an edge here so we're done here
                continue;
            }

            // assign area
            self.grid[&current].area = self.current;
            count += 1;

            // add horizontal neighbours
            for n in SlabNeighbours::new(current) {
                let cost = EdgeCost::Walk;
                let src = Some((current, cost));
                self.queue.push((n, src));
            }

            // check vertical neighbours for jump access

            // don't queue the slab above's neighbours if we're at the top of the slab
            if current[2] < SLAB_SIZE.as_i32() - 1 {
                // only check for jump ups if the block directly above is not solid
                if self
                    .get_vertical_offset(current, VerticalOffset::Above)
                    .opacity
                    .transparent()
                {
                    let [x, y, z] = current;
                    let above = [x, y, z + 1];

                    for n in SlabNeighbours::new(above) {
                        self.queue.push((n, Some((current, EdgeCost::JumpUp))));
                    }
                }
            }

            // don't queue the slab below's neighbours if we're at the bottom of the slab
            if current[2] > 0 {
                for n_adjacent in SlabNeighbours::new(current) {
                    // only check for jump downs if the block directly above that is not solid
                    // (mirrored check for jump ups above)

                    if self.grid[&n_adjacent].opacity.transparent() {
                        let [x, y, z] = n_adjacent;
                        let n_below = [x, y, z - 1]; // the check above ensures we stay in this slab

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

    fn is_walkable(&self, pos: CoordType) -> bool {
        let marker = &self.grid[&pos];

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
        block: CoordType,
        offset: VerticalOffset,
    ) -> AreaDiscoveryGridBlock {
        let [x, y, z] = block;
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

                self.grid[&[x, y, offset_z]]
            }
        }
    }

    /// Moves area->block graphs map out of self
    pub fn areas_with_graph(&mut self) -> impl Iterator<Item = (ChunkArea, BlockGraph)> {
        let block_graphs = std::mem::replace(&mut self.block_graphs, HashMap::new());
        block_graphs.into_iter()
    }

    /// Assign areas to the blocks in the slab
    pub fn apply(self, slab: &mut SlabGridImpl) {
        for i in slab.indices() {
            *slab[i].area_mut() = self.grid[i].area;
        }
    }
}
