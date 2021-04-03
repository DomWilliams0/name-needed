use rstar::{Envelope, RTree, RTreeObject, AABB};

use unit::world::{BlockPosition, GlobalSliceIndex, SlabLocation, SliceBlock, CHUNK_SIZE};

use crate::region::feature::{ApplyFeatureContext, FeatureZRange};
use crate::region::region::{ChunkDescription, ChunkHeightMap};
use crate::region::{Feature, PlanetPoint, CHUNKS_PER_REGION_SIDE};
use crate::{BiomeType, BlockType, SlabGrid};
use common::{InnerSpace, IteratorRandom, Rng, SliceRandom, SmallRng, Vector2};
use geo::coords_iter::CoordsIter;
use geo::prelude::Contains;
use geo::Polygon;
use geo_booleanop::boolean::BooleanOp;
use rand_distr::{Distribution, Normal};
use rstar::primitives::PointWithData;
use std::fmt::{Debug, Formatter};
use std::process::exit;

pub struct ForestFeature {
    trees: RTree<PointWithData<(), [f64; 2]>>,
}

const POISSON_DISK_RADIUS: f64 = PlanetPoint::PER_BLOCK * 3.0;

const POISSON_DISK_MAX_ATTEMPTS: usize = 20;

#[deprecated]
static mut DONEZO: bool = false;

impl Feature for ForestFeature {
    fn name(&self) -> &'static str {
        "forest"
    }

    fn extend_z_range(&self, mut range: FeatureZRange) -> FeatureZRange {
        // tree height
        // TODO remove magic value, use real max tree height
        *range.y_mut() += 16;

        // TODO tree roots

        range
    }

    /// Chooses tree positions using Bridson poisson disk sampling
    /// https://www.cct.lsu.edu/~fharhad/ganbatte/siggraph2007/CD2/content/sketches/0250.pdf
    fn apply(
        &mut self,
        loc: SlabLocation,
        ctx: &mut ApplyFeatureContext<'_>,
        bounding: &Polygon<f64>,
    ) {
        let mut rando = ctx.slab_rando(loc);

        // find union of full forest polygon and this slab polygon
        // TODO this does SO many temporary allocations
        let slab_bounds = ctx.slab_bounds.to_polygon();
        let bounding = bounding.intersection(&slab_bounds);

        // TODO this is broken for now
        return;

        let slab_base = (loc.chunk.x() as f64, loc.chunk.y() as f64);

        let initial_point = {
            let block_idx = match (0..20)
                .map(|_| {
                    let idx = rando.gen_range(0, ChunkHeightMap::FULL_SIZE);
                    // TODO unchecked unwrap, can never be None
                    (idx, ctx.chunk_desc.blocks().nth(idx).unwrap())
                })
                .find(|(_, b)| b.biome() == BiomeType::Forest)
            {
                Some((idx, _)) => idx,
                None => {
                    // no forest block found randomly, fallback to the first then, which must exist
                    // seeing as we're growing a forest in this slab
                    match ctx
                        .chunk_desc
                        .blocks()
                        .position(|b| b.biome() == BiomeType::Forest)
                    {
                        Some(i) => i,
                        None => {
                            common::warn!("alleged forest has no forest"; loc);
                            return;
                        }
                    }
                    // .expect("alleged forest has no forest")
                }
            };

            let [x, y, _] = ChunkHeightMap::unflatten(block_idx);

            // offset a bit
            let variation = PlanetPoint::PER_BLOCK / 2.0;
            [
                slab_base.0 + x as f64 + rando.gen_range(-variation, variation),
                slab_base.1 + y as f64 + rando.gen_range(-variation, variation),
            ]
        };

        let mut active_points = Vec::with_capacity(128);
        active_points.push(initial_point);
        self.add_tree(initial_point.into(), ctx);

        let distr = Normal::new(0.0, 1.0).unwrap();
        let is_in_bounds = |p: [f64; 2]| bounding.contains(&geo::Point::from(p));

        while !active_points.is_empty() {
            let point_idx = rando.gen_range(0, active_points.len());
            let point = unsafe { *active_points.get_unchecked(point_idx) };

            let len_before = self.trees.size();
            for _ in 0..POISSON_DISK_MAX_ATTEMPTS {
                let candidate = {
                    // generates a random unit vector
                    // ty https://stackoverflow.com/a/8453514
                    let x: f64 = distr.sample(&mut rando);
                    let y = distr.sample(&mut rando);
                    let unit_mag = (x * x + y * y).sqrt();
                    let [dx, dy] = [x / unit_mag, y / unit_mag];

                    let rando_mag = rando.gen_range(POISSON_DISK_RADIUS, POISSON_DISK_RADIUS * 2.0);

                    [point[0] + (dx * rando_mag), point[1] + (dy * rando_mag)]
                };

                // check candidate
                if is_in_bounds(candidate)
                    && self
                        .trees
                        .locate_in_envelope_intersecting({
                            let min = [
                                candidate[0] - POISSON_DISK_RADIUS,
                                candidate[1] - POISSON_DISK_RADIUS,
                            ];
                            let max = [
                                candidate[0] + POISSON_DISK_RADIUS,
                                candidate[1] + POISSON_DISK_RADIUS,
                            ];
                            &AABB::from_corners(min, max)
                        })
                        .next()
                        .is_none()
                {
                    // valid point
                    active_points.push(candidate);
                    self.add_tree(candidate.into(), ctx);
                } else {
                    // invalid point, ignore candidate
                }
            }

            let len_after = self.trees.size();
            if len_before == len_after {
                // no candidates generated
                active_points.swap_remove(point_idx);
            }
        }

        // // dummy block replacement
        // for (i, block) in chunk_description.blocks().enumerate() {
        //     if block.biome() == BiomeType::Forest {
        //         let z = block.ground() + 1;
        //         let [x, y, _] = SlabGrid::unflatten(i);
        //         slab[&[x, y, z.slice()]].ty = BlockType::SolidWater;
        //     }
        // }

        // TODO generate tree locations with poisson disk sampling
        // TODO attempt to place tree model at location in this slab
        // TODO if a tree/subfeature is cut off, keep track of it as a continuation for the neighbouring slab
    }
}

impl ForestFeature {
    fn add_tree(&mut self, planet_point: PlanetPoint, ctx: &mut ApplyFeatureContext) {
        self.trees
            .insert(PointWithData::new((), planet_point.get_array()));

        // let tree_base = {
        //     // find xy
        //     let mut block = planet_point.into_block(GlobalSliceIndex::new(0));
        //
        //     // use xy to find z ground level
        //     let ground = ctx.chunk_desc.ground_level(block);
        //
        //     block.to_block_position(ground+1)
        // };
        //
        // ctx.terrain[&tree_base.xyz()].ty = BlockType::SolidWater;
    }
}

impl Default for ForestFeature {
    fn default() -> Self {
        Self {
            // TODO consider rtree params
            trees: RTree::new(),
        }
    }
}

impl Debug for ForestFeature {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ForestFeature")
    }
}
