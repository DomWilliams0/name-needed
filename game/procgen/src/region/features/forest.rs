use rstar::{RTree, AABB};

use unit::world::{BlockPosition, GlobalSliceIndex, SlabLocation, SliceBlock};

use crate::region::feature::{ApplyFeatureContext, FeatureZRange};
use crate::region::region::ChunkHeightMap;
use crate::region::{Feature, PlanetPoint};
use crate::{BiomeType, BlockType};
use common::Rng;

use geo::prelude::Contains;
use geo::MultiPolygon;
use geo_booleanop::boolean::BooleanOp;
use rand_distr::{Distribution, Normal};
use rstar::primitives::PointWithData;
use std::any::Any;
use std::fmt::{Debug, Formatter};

pub struct ForestFeature {
    trees: RTree<PointWithData<(), [f64; 2]>>,
}

const POISSON_DISK_RADIUS: f64 = PlanetPoint::PER_BLOCK * 3.0;

const POISSON_DISK_MAX_ATTEMPTS: usize = 20;

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
        bounding: &MultiPolygon<f64>,
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

        // TODO generate tree locations with poisson disk sampling
        // TODO attempt to place tree model at location in this slab
        // TODO if a tree/subfeature is cut off, keep track of it as a continuation for the neighbouring slab
    }

    fn merge_with(&mut self, other: &mut dyn Feature) -> bool {
        if let Some(other) = other.any_mut().downcast_mut::<Self>() {
            // steal other's trees
            // TODO replace this rtree with a new bulk loaded one?
            // TODO PR to move nodes out of the tree instead of copy
            let n = other.trees.size();
            let stolen_trees = std::mem::replace(&mut other.trees, RTree::new());
            for tree in stolen_trees.iter() {
                self.trees.insert(*tree);
            }

            common::debug!("merged {trees} trees from other forest", trees = n; "total" => self.trees.size());

            true
        } else {
            // type mismatch
            false
        }
    }

    fn any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl ForestFeature {
    fn add_tree(&mut self, planet_point: PlanetPoint, ctx: &mut ApplyFeatureContext) {
        self.trees
            .insert(PointWithData::new((), planet_point.get_array()));

        let tree_base = {
            // find xy
            let pos = planet_point.into_block(GlobalSliceIndex::new(0));
            let block = SliceBlock::from(BlockPosition::from(pos));

            // use xy to find z ground level
            let ground = ctx.chunk_desc.ground_level(block);

            block.to_block_position(ground + 1)
        };

        ctx.terrain[&tree_base.xyz()].ty = BlockType::SolidWater;
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
