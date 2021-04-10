use rstar::{RTree, AABB};

use unit::world::{BlockPosition, GlobalSliceIndex, SlabLocation, SliceBlock};

use crate::region::feature::{ApplyFeatureContext, FeatureZRange, RegionalFeatureBoundary};
use crate::region::region::{BlockHeight, ChunkHeightMap};
use crate::region::{Feature, PlanetPoint, CHUNKS_PER_REGION_SIDE};
use crate::BlockType;
use common::*;

use geo::prelude::{Contains, Intersects};
use geo::Rect;
use geo_booleanop::boolean::BooleanOp;
use rand_distr::{Distribution, Normal};
use rstar::primitives::PointWithData;
use std::any::Any;
use std::fmt::{Debug, Formatter};

#[derive(Default)]
pub struct ForestFeature {
    trees: PoissonDiskSampling,
}

struct PoissonDiskSampling {
    points: RTree<PointWithData<(), [f64; 2]>>,
}

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
        bounding: &RegionalFeatureBoundary,
    ) {
        let mut rando = ctx.slab_rando(loc);
        self.trees.spread(
            &mut rando,
            loc,
            ctx.slab_bounds,
            bounding,
            ctx.chunk_desc.blocks(),
            // TODO pass filter closure to check the biome of the tree block too, because feature hull is not perfect
            |point| {
                let tree_base = {
                    // find xy
                    let pos = point.into_block(GlobalSliceIndex::new(0));
                    let block = SliceBlock::from(BlockPosition::from(pos));

                    // use xy to find z ground level
                    let ground = ctx.chunk_desc.ground_level(block);

                    block.to_block_position(ground + 1)
                };

                ctx.terrain[&tree_base.xyz()].ty = BlockType::SolidWater;
            },
        );

        // TODO attempt to place tree model at location in this slab
        // TODO if a tree/subfeature is cut off, keep track of it as a continuation for the neighbouring slab
    }

    fn merge_with(&mut self, other: &mut dyn Feature) -> bool {
        if let Some(other) = other.any_mut().downcast_mut::<Self>() {
            // steal other's trees
            let n = self.trees.absorb_other(&mut other.trees);
            debug!("merged {trees} trees from other forest", trees = n; "total" => self.trees.points.size());
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

impl Default for PoissonDiskSampling {
    fn default() -> Self {
        Self {
            // TODO consider rtree params
            points: RTree::new(),
        }
    }
}

impl PoissonDiskSampling {
    /// Number of blocks between each tree
    /// TODO put this in planet params
    const BLOCK_DISTANCE: u32 = 8;
    const MAX_ATTEMPTS: usize = 20;

    const RADIUS: f64 = PlanetPoint::PER_BLOCK * Self::BLOCK_DISTANCE as f64;

    fn spread(
        &mut self,
        mut rando: &mut SmallRng,
        slab: SlabLocation,
        slab_bounds: &Rect<f64>,
        full_bounds: &RegionalFeatureBoundary,
        chunk_blocks: &[BlockHeight],
        mut add_point: impl FnMut(PlanetPoint),
    ) {
        let slab_base = slab_bounds.min();

        // find intersection of full bounds and this slab polygon
        // TODO this does SO many temporary allocations
        let slab_bounds = slab_bounds.to_polygon();
        let bounding = full_bounds.intersection(&slab_bounds);
        let is_in_bounds = |p: [f64; 2]| bounding.contains(&geo::Point::from(p));

        debug_assert!(full_bounds.intersects(&slab_bounds));
        debug_assert!(bounding.intersects(&slab_bounds));
        debug_assert_eq!(chunk_blocks.len(), ChunkHeightMap::FULL_SIZE);

        const SIZE: usize = CHUNKS_PER_REGION_SIDE; // TODO add const generic

        let initial_point = {
            let mut found = None;
            for _attempt in 0..Self::MAX_ATTEMPTS * 2 {
                let random_point = [
                    slab_base.x + rando.gen_range(0.0, 1.0 / SIZE as f64),
                    slab_base.y + rando.gen_range(0.0, 1.0 / SIZE as f64),
                ];

                if is_in_bounds(random_point) && self.is_valid_point(random_point) {
                    found = Some(random_point);
                    break;
                }
            }

            match found {
                Some([x, y]) => PlanetPoint::new(x, y),
                None => {
                    warn!("failed to place random initial tree in alleged forest"; slab);
                    return;
                }
            }
        };

        let mut active_points = Vec::with_capacity(128);
        active_points.push(initial_point);

        add_point(initial_point);
        self.points
            .insert(PointWithData::new((), initial_point.get_array()));
        debug_assert!(is_in_bounds(initial_point.get_array()));

        let distr = Normal::new(0.0, 1.0).unwrap();
        while !active_points.is_empty() {
            let point_idx = rando.gen_range(0, active_points.len());
            let point = unsafe { *active_points.get_unchecked(point_idx) }.get_array();

            let len_before = self.points.size();
            for _ in 0..Self::MAX_ATTEMPTS {
                let candidate = {
                    // generates a random unit vector
                    // ty https://stackoverflow.com/a/8453514
                    let x: f64 = distr.sample(&mut rando);
                    let y = distr.sample(&mut rando);
                    let unit_mag = (x * x + y * y).sqrt();
                    let [dx, dy] = [x / unit_mag, y / unit_mag];

                    let rando_mag = rando.gen_range(Self::RADIUS, Self::RADIUS * 2.0);

                    [point[0] + (dx * rando_mag), point[1] + (dy * rando_mag)]
                };

                // check candidate
                if is_in_bounds(candidate) && self.is_valid_point(candidate) {
                    // valid point
                    active_points.push(candidate.into());
                    add_point(candidate.into());
                    self.points.insert(PointWithData::new((), candidate));
                } else {
                    // invalid point, ignore candidate
                }
            }

            if len_before == self.points.size() {
                // no candidates generated
                active_points.swap_remove(point_idx);
            }
        }
    }

    fn absorb_other(&mut self, other: &mut Self) -> usize {
        // TODO replace this rtree with a new bulk loaded one?
        // TODO PR to move nodes out of the tree instead of copy
        let stolen_points = std::mem::replace(&mut other.points, RTree::new());
        let n = stolen_points.size();
        for tree in stolen_points.iter() {
            self.points.insert(*tree);
        }

        n
    }

    fn is_valid_point(&self, candidate: [f64; 2]) -> bool {
        self.points
            .locate_in_envelope_intersecting({
                let min = [candidate[0] - Self::RADIUS, candidate[1] - Self::RADIUS];
                let max = [candidate[0] + Self::RADIUS, candidate[1] + Self::RADIUS];
                &AABB::from_corners(min, max)
            })
            .next()
            .is_none()
    }
}

impl Debug for ForestFeature {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ForestFeature")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::planet::slab_bounds;
    use crate::BiomeType;

    #[test]
    fn poisson_disk_sampling() {
        let mut rando = SmallRng::seed_from_u64(515151515);
        let mut poisson = PoissonDiskSampling::default();

        let slab = SlabLocation::new(0, (50, 200));
        let slab_bounds = slab_bounds(slab);
        let forest_bounds = {
            let size = 50.0;
            let rect = Rect::new((-size, -size), (size, size));
            RegionalFeatureBoundary::with_single(rect.to_polygon())
        };

        let chunk_blocks = vec![
            {
                let mut b = BlockHeight::default();
                b.set_biome(BiomeType::Forest);
                b
            };
            ChunkHeightMap::FULL_SIZE
        ];

        let mut points = vec![];
        poisson.spread(
            &mut rando,
            slab,
            &slab_bounds,
            &forest_bounds,
            &chunk_blocks,
            |p| {
                points.push(p);
            },
        );

        // main test is debug asserts within spread()
        assert!(!points.is_empty());
    }
}
