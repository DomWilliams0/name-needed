use std::collections::HashMap;

use unit::dim::CHUNK_SIZE;
use unit::world::{BlockCoord, SlabIndex, SLAB_SIZE};

use crate::block::Block;
use crate::chunk::slab::Slab;
use crate::chunk::slice::Slice;
use crate::grid::{CoordType, Grid, GridImpl};
use crate::grid_declare;
use crate::navigation::{BlockGraph, ChunkArea, EdgeCost, SlabAreaIndex};

grid_declare!(struct _AreaDiscoveryGrid<AreaDiscoveryGridImpl, _AreaDiscoveryGridBlock>,
    CHUNK_SIZE.as_usize(),
    CHUNK_SIZE.as_usize(),
    SLAB_SIZE.as_usize()
);

// TODO shouldnt be pub
#[derive(Default, Copy, Clone)]
pub struct _AreaDiscoveryGridBlock {
    /// is material solid
    solid: bool,

    area: SlabAreaIndex,
}

#[derive(Default)]
pub(crate) struct AreaDiscovery<'a> {
    grid: _AreaDiscoveryGrid,

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
    above_bot_slice: Option<Slice<'a>>,
}

impl Into<_AreaDiscoveryGridBlock> for &Block {
    fn into(self) -> _AreaDiscoveryGridBlock {
        _AreaDiscoveryGridBlock {
            solid: self.opacity().solid(),
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
        slab: &Slab,
        below_top_slice: Option<Slice<'a>>,
        above_bot_slice: Option<Slice<'a>>,
    ) -> Self {
        let mut grid = _AreaDiscoveryGrid::default();

        for i in grid.indices() {
            let b: &Block = &slab.grid()[i];
            grid[i] = b.into();
        }

        Self {
            grid,
            queue: Vec::new(),
            current: SlabAreaIndex::FIRST,
            areas: Vec::new(),
            block_graphs: HashMap::new(),
            slab_index: slab.index(),
            below_top_slice,
            above_bot_slice,
        }
    }

    /// Flood fills from every block, increment area index after each flood fill
    /// Returns area count
    pub fn flood_fill_areas(&mut self) -> u16 {
        let range = {
            let (s0_start, _) = _AreaDiscoveryGrid::slice_range(0);
            let end = _AreaDiscoveryGrid::FULL_SIZE;
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
        self.queue.push((self.grid.unflatten_index(start), None));
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
            for n in Neighbours::new(current) {
                let cost = EdgeCost::Walk;
                let src = Some((current, cost));
                self.queue.push((n, src));
            }

            // check vertical neighbours for jump access only

            // don't queue the slab above's neighbours if we're at the top of the slab
            if current[2] < SLAB_SIZE.as_i32() - 1 {
                // only check the above slab if its not solid and a jump is possible
                if !self
                    .get_vertical_offset(current, VerticalOffset::Above)
                    .solid
                {
                    let [x, y, z] = current;
                    let above = [x, y, z + 1];

                    for n in Neighbours::new(above) {
                        self.queue.push((n, Some((current, EdgeCost::JumpUp))));
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
        }
    }

    fn is_walkable(&self, pos: CoordType) -> bool {
        let marker = &self.grid[&pos];

        if marker.solid {
            return false;
        }

        let below = self.get_vertical_offset(pos, VerticalOffset::Below);

        // below not solid either: nope
        if !below.solid {
            return false;
        }

        // below is solid and full: nice
        true
    }

    fn get_vertical_offset(
        &self,
        block: CoordType,
        offset: VerticalOffset,
    ) -> _AreaDiscoveryGridBlock {
        let [x, y, z] = block;
        const TOP: i32 = SLAB_SIZE.as_i32() - 1;

        match z {
            // top of the slab: check slab above
            TOP if offset == VerticalOffset::Above => {
                if let Some(above_slice) = &self.above_bot_slice {
                    // it is present
                    (&above_slice[(x as BlockCoord, y as BlockCoord)]).into()
                } else {
                    // not present: this must be the top of the world
                    _AreaDiscoveryGridBlock {
                        solid: false,
                        ..Default::default()
                    }
                }
            }

            // bottom of the slab: check slab below
            0 if offset == VerticalOffset::Below => {
                if let Some(below_slice) = &self.below_top_slice {
                    // it is present
                    (&below_slice[(x as BlockCoord, y as BlockCoord)]).into()
                } else {
                    // not present: this must be the bottom of the world
                    _AreaDiscoveryGridBlock {
                        solid: false,
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

    pub fn apply(self, slab: &mut Slab) {
        let grid = slab.grid_mut();
        for i in self.grid.indices() {
            *grid[i].area_mut() = self.grid[i].area;
        }
    }
}

struct Neighbours {
    block: CoordType,
    idx: usize,
}

impl Neighbours {
    const HORIZONTAL_OFFSETS: [(i32, i32); 4] = [(-1, 0), (0, -1), (0, 1), (1, 0)];

    fn new(block: CoordType) -> Self {
        Self { block, idx: 0 }
    }
}

impl Iterator for Neighbours {
    type Item = CoordType;

    fn next(&mut self) -> Option<Self::Item> {
        let [x, y, z] = self.block;

        for (i, &(dx, dy)) in Self::HORIZONTAL_OFFSETS.iter().enumerate().skip(self.idx) {
            self.idx = i + 1;

            let n = {
                let (nx, ny) = (x + dx, y + dy);

                if nx < 0 || nx >= CHUNK_SIZE.as_i32() {
                    continue;
                }

                if ny < 0 || ny >= CHUNK_SIZE.as_i32() {
                    continue;
                }

                [nx, ny, z]
            };

            return Some(n);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use crate::navigation::discovery::Neighbours;

    #[test]
    fn neighbours() {
        let n = Neighbours::new([2, 2, 2]);
        assert_eq!(n.count(), 4);
    }
}
