use crate::area::{SlabArea, SlabAreaIndex};
use crate::block::{Block, BlockHeight};
use crate::chunk::slab::{Slab, SlabIndex};
use crate::chunk::slice::SliceOwned;
use crate::grid::{CoordType, Grid, GridImpl};
use crate::grid_declare;
use crate::{CHUNK_DEPTH, CHUNK_SIZE};

grid_declare!(struct _AreaDiscoveryGrid<AreaDiscoveryGridImpl, _AreaDiscoveryGridBlock>, CHUNK_SIZE.as_usize(), CHUNK_SIZE.as_usize(), CHUNK_DEPTH.as_usize());

// TODO shouldnt be pub
#[derive(Default, Copy, Clone)]
pub struct _AreaDiscoveryGridBlock {
    /// is material solid
    solid: bool,

    /// is not a half step
    height: BlockHeight,

    area: SlabAreaIndex,
}

#[derive(Default)]
pub(crate) struct AreaDiscovery {
    grid: _AreaDiscoveryGrid,

    /// flood fill queue TODO share between slabs
    queue: Vec<CoordType>,

    /// current area index to flood fill with
    current: SlabAreaIndex,

    /// all areas in this slab collected during discovery
    areas: Vec<SlabArea>,

    slab_index: SlabIndex,

    below_top_slice: Option<SliceOwned>,
}

impl Into<_AreaDiscoveryGridBlock> for &Block {
    fn into(self) -> _AreaDiscoveryGridBlock {
        _AreaDiscoveryGridBlock {
            solid: self.solid(),
            height: self.block_height(),
            area: Default::default(),
        }
    }
}

impl AreaDiscovery {
    pub fn from_slab(slab: &Slab, below_top_slice: Option<SliceOwned>) -> Self {
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
            slab_index: slab.index(),
            below_top_slice,
        }
    }

    /// Flood fills from every block, increment area index after each flood fill
    /// Returns area count
    pub fn flood_fill_areas(&mut self) -> u8 {
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
        self.queue.push(self.grid.unflatten_index(start));

        while let Some(current) = self.queue.pop() {
            // check self
            if self.grid[&current].area.initialized() {
                continue;
            }

            if !self.is_walkable(current) {
                continue;
            }

            // walkable, nice
            self.grid[&current].area = self.current;
            count += 1;

            // check neighbours
            let [x, y, z] = current;
            const NEIGHBOURS: [(i32, i32); 4] = [(-1, 0), (0, -1), (0, 1), (1, 0)];

            for (dx, dy) in NEIGHBOURS.iter().copied() {
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

                self.queue.push(n);
            }
        }

        // increment area
        if count > 0 {
            self.areas.push(SlabArea {
                slab: self.slab_index,
                area: self.current,
            });
            self.current.increment();
        }
    }

    fn is_walkable(&self, pos: CoordType) -> bool {
        let [x, y, z] = pos;

        let marker = &self.grid[&pos];

        if marker.solid {
            // solid and half block: yes
            // solid and full block: no
            return !marker.height.solid();
        }

        let below: _AreaDiscoveryGridBlock = {
            if z == 0 {
                // this is the bottom slice: check the slab below
                if let Some(below_slice) = &self.below_top_slice {
                    // it is present: make a marker for it
                    (&below_slice[(x as u16, y as u16)]).into()
                } else {
                    // not present: this must be the bottom of the world
                    _AreaDiscoveryGridBlock {
                        solid: false,
                        ..Default::default()
                    }
                }
            } else {
                // not the bottom slice, just get the below block
                self.grid[&[x, y, z - 1]]
            }
        };

        // below not solid either: nope
        if !below.solid {
            return false;
        }

        // below is solid but half block: nope
        if !below.height.solid() {
            return false;
        }

        // below is solid and full: nice
        true
    }

    pub fn areas(&self) -> &[SlabArea] {
        &self.areas
    }

    pub fn apply(self, slab: &mut Slab) {
        let grid = slab.grid_mut();
        for i in self.grid.indices() {
            *grid[i].area_mut() = self.grid[i].area;
        }
    }
}
