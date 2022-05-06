use rstar::{RTree, AABB};

use unit::world::{BlockPosition, GlobalSliceIndex, SlabLocation, SliceBlock};

use crate::region::feature::{ApplyFeatureContext, FeatureZRange, RegionalFeatureBoundary};

use crate::region::{Feature, PlanetPoint, CHUNKS_PER_REGION_SIDE};
use crate::{BiomeType, PlanetParams};
use common::*;

use crate::region::subfeatures::Tree;
use common::random::SmallRngExt;
use geo::prelude::{Contains, Intersects};
use geo::Rect;
use geo_booleanop::boolean::BooleanOp;
use rand_distr::{Distribution, Normal};
use rstar::primitives::GeomWithData;
use std::any::Any;
use std::fmt::{Debug, Formatter};

pub struct ForestFeature {
    trees: PoissonDiskSampling,
}

struct PoissonDiskSampling {
    points: RTree<GeomWithData<[f64; 2], ()>>,

    /// From config
    radius: f64,

    /// From config
    attempts: u32,
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
    fn apply(&mut self, ctx: &mut ApplyFeatureContext<'_>, bounding: &RegionalFeatureBoundary) {
        // deterministic tree placement
        let mut rando_placement = ctx.slab_rando();

        // non deterministic tree characteristics
        let mut tree_rando = SmallRng::new_quick();

        self.trees.spread(
            &mut rando_placement,
            ctx.slab,
            ctx.slab_bounds,
            bounding,
            |point| {
                let tree_base = {
                    // find xy
                    let mut pos = point.into_block(GlobalSliceIndex::new(0)); // dummy z
                    let block = SliceBlock::from(BlockPosition::from(pos));

                    // use xy to find z ground level
                    let block_desc = ctx.chunk_desc.block(block);

                    // validate biome
                    if block_desc.biome() != BiomeType::Forest {
                        return false;
                    }

                    let z = block_desc.ground() + 1;
                    pos.2 = z;
                    pos
                };

                // attempt to place tree
                let tree = {
                    let height = tree_rando.gen_range(5, 8);
                    let w = tree_rando.gen_range(2, 4);
                    let h = tree_rando.gen_range(2, 4);
                    Tree::new(height, (w, h))
                };
                ctx.queue_subfeature(tree, tree_base);
                true
            },
        );

        // TODO attempt to place tree model at location in this slab
    }

    fn merge_with(&mut self, other: &mut dyn Feature) -> bool {
        if let Some(other) = other.any_mut().downcast_mut::<Self>() {
            // steal other's trees
            let n = self.trees.absorb_other(&mut other.trees);
            assert_eq!(
                n, 0,
                "there should be no trees in the forest being gutted!!"
            );
            // debug!("merged {trees} trees from other forest", trees = n; "total" => self.trees.points.size());
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
    pub fn new(params: &PlanetParams) -> Self {
        ForestFeature {
            trees: PoissonDiskSampling::new(params.forest_pds_radius, params.forest_pds_attempts),
        }
    }
}

impl PoissonDiskSampling {
    pub fn new(block_spacing: u32, attempts: u32) -> Self {
        // TODO actual validation
        assert!(block_spacing > 0);

        Self {
            // TODO consider rtree params
            points: RTree::new(),
            // TODO const generic size param
            radius: PlanetPoint::PER_BLOCK * block_spacing as f64,
            attempts,
        }
    }

    fn spread(
        &mut self,
        mut rando: &mut SmallRng,
        slab: SlabLocation,
        slab_bounds: &Rect<f64>,
        full_bounds: &RegionalFeatureBoundary,
        mut add_point: impl FnMut(PlanetPoint) -> bool,
    ) {
        let slab_base = slab_bounds.min();

        // find intersection of full bounds and this slab polygon
        // TODO this does SO many temporary allocations
        let slab_bounds = slab_bounds.to_polygon();
        let bounding = full_bounds.intersection(&slab_bounds);
        let is_in_bounds = |p: [f64; 2]| bounding.contains(&geo::Point::from(p));

        debug_assert!(full_bounds.intersects(&slab_bounds));

        if !bounding.intersects(&slab_bounds) {
            // boundary grazes this slab but doesn't have any blocks in it, nevermind
            return;
        }

        const SIZE: usize = CHUNKS_PER_REGION_SIDE; // TODO add const generic (and use the unspecialised PlanetPoint)

        let initial_point = {
            let mut found = None;
            // try twice as hard to place an initial point
            for _attempt in 0..self.attempts * 2 {
                let random_point = [
                    slab_base.x + rando.gen_range(0.0, 1.0 / SIZE as f64),
                    slab_base.y + rando.gen_range(0.0, 1.0 / SIZE as f64),
                ];

                if is_in_bounds(random_point)
                    && self.is_valid_point(random_point)
                    && add_point(random_point.into())
                {
                    found = Some(random_point);
                    break;
                }
            }

            match found {
                Some([x, y]) => PlanetPoint::new(x, y),
                None => {
                    debug!("failed to place random initial tree in alleged forest"; slab);
                    return;
                }
            }
        };

        let mut active_points = Vec::with_capacity(128);
        active_points.push(initial_point);

        // add_point() already returned true for us to get this far, dont add again

        let initial_point = initial_point.get_array();
        self.points.insert(GeomWithData::new(initial_point, ()));

        debug_assert!(is_in_bounds(initial_point));

        let distr = Normal::new(0.0, 1.0).unwrap();
        while !active_points.is_empty() {
            let point_idx = rando.gen_range(0, active_points.len());
            let point = unsafe { *active_points.get_unchecked(point_idx) }.get_array();

            let len_before = self.points.size();
            for _ in 0..self.attempts {
                let candidate = {
                    // generates a random unit vector
                    // ty https://stackoverflow.com/a/8453514
                    let x: f64 = distr.sample(&mut rando);
                    let y = distr.sample(&mut rando);
                    let unit_mag = (x * x + y * y).sqrt();
                    let [dx, dy] = [x / unit_mag, y / unit_mag];

                    let rando_mag = rando.gen_range(self.radius, self.radius * 2.0);

                    [point[0] + (dx * rando_mag), point[1] + (dy * rando_mag)]
                };

                let candidate_point = PlanetPoint::from(candidate);

                // check candidate
                if is_in_bounds(candidate)
                    && self.is_valid_point(candidate)
                    && add_point(candidate_point)
                {
                    // valid point
                    active_points.push(candidate_point);
                    self.points.insert(GeomWithData::new(candidate, ()));
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
                let min = [candidate[0] - self.radius, candidate[1] - self.radius];
                let max = [candidate[0] + self.radius, candidate[1] + self.radius];
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

    #[test]
    fn poisson_disk_sampling() {
        let mut rando = SmallRng::seed_from_u64(515151515);
        let mut poisson = PoissonDiskSampling::new(8, 20);

        let slab = SlabLocation::new(0, (50, 200));
        let slab_bounds = slab_bounds(slab);
        let forest_bounds = {
            let size = 50.0;
            let rect = Rect::new((-size, -size), (size, size));
            RegionalFeatureBoundary::new_as_is(rect.to_polygon())
        };

        let mut points = vec![];
        poisson.spread(&mut rando, slab, &slab_bounds, &forest_bounds, |p| {
            points.push(p);
            true
        });

        // main test is debug asserts within spread()
        assert!(!points.is_empty());
    }
}
