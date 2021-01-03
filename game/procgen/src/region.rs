use crate::continent::{ContinentMap, Generator};
use crate::PlanetParams;
use common::*;
use std::rc::Rc;

/// Each region is broken up into this many chunks per side, i.e. this^2 for total number of chunks
const CHUNKS_PER_REGION_SIDE: usize = 16;

pub struct Regions {
    params: PlanetParams,
    regions: Vec<((u32, u32), Region)>,
}

/// Each pixel in the continent map is a region. Each region is a 2d grid of chunks.
///
/// When any slab in a chunk in a region is requested, the whole region is created and features
/// calculated (trees placements as part of forests, rivers, ore placements) but not applied, only stored along with
/// their 3d bounds.
///
/// Slab requests into that region are produced initially from a block distribution, such as:
///     * all air if above ground
///     * surface blocks (grass, stone etc) if at ground level
///     * solid stone underground
/// Then the regional features are applied if they intersect with this slab.
pub struct Region {}

impl Regions {
    pub fn new(params: &PlanetParams) -> Self {
        Regions {
            params: params.clone(),
            regions: Vec::with_capacity(64),
        }
    }

    // TODO result for out of range
    pub fn get_or_create(&mut self, coords: (u32, u32), generator: Rc<Generator>) -> &Region {
        match self.region_index(coords) {
            Ok(idx) => &self.regions[idx].1,
            Err(idx) => {
                debug!("creating new region"; "region" => ?coords);
                let region = Region::create(coords, generator, &self.params);
                self.regions.insert(idx, (coords, region));
                &self.regions[idx].1
            }
        }
    }

    pub fn get_existing(&self, region: (u32, u32)) -> Option<&Region> {
        self.region_index(region)
            .ok()
            .map(|idx| &self.regions[idx].1)
    }

    fn region_index(&self, region: (u32, u32)) -> Result<usize, usize> {
        self.regions.binary_search_by_key(&region, |(pos, _)| *pos)
    }
}

impl Region {
    fn create(coords: (u32, u32), generator: Rc<Generator>, params: &PlanetParams) -> Self {
        let (x, y) = (coords.0 as f64, coords.1 as f64);

        // for i in 0..CHUNKS_PER_REGION_SIDE {
        //     let div = 1.0 / CHUNKS_PER_REGION_SIDE as f64;
        //
        //     let rx = x + (div * i as f64);
        //     let ry = y;
        //     let f = generator.sample((rx, ry));
        // }

        todo!();

        Region {}
    }
}
