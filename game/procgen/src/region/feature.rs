use std::sync::Arc;

use geo::prelude::*;
use geo::{Polygon, Rect};
use rstar::RTree;
use tokio::sync::Mutex;

use common::*;
use unit::world::{GlobalSliceIndex, SlabLocation, SliceBlock, CHUNK_SIZE};

use crate::region::region::{ChunkDescription, ChunkLocation};
use crate::{BiomeType, BlockType, SlabGrid};
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;

/// Feature discovered at region initialization. Belongs in an Arc
#[derive(Debug)] // TODO custom debug to not print full bounding polygon
pub struct RegionalFeature {
    /// 2d bounds around feature, only applies to slabs within this polygon
    bounding: Polygon<f64>,

    /// Inclusive bounds in the z direction for this feature
    z_range: FeatureZRange,

    // TODO make this struct a dst and store trait object inline without extra indirection
    feature: Mutex<Box<dyn Feature>>,
}

/// Inclusive bounds in the z direction for a feature
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct FeatureZRange(GlobalSliceIndex, GlobalSliceIndex);

pub type SharedRegionalFeature = Arc<RegionalFeature>;

pub trait Feature: Send + Sync + Debug {
    fn name(&self) -> &'static str;

    /// Increase z range based on this feature e.g. tree height
    fn extend_z_range(&self, range: FeatureZRange) -> FeatureZRange;

    fn apply(
        &mut self,
        loc: SlabLocation,
        ctx: &mut ApplyFeatureContext<'_>,
        bounding: &Polygon<f64>,
    );
}

/// Context for applying a feature to a slab
pub struct ApplyFeatureContext<'a> {
    pub chunk_desc: &'a ChunkDescription,
    pub terrain: &'a mut SlabGrid,
    pub planet_seed: u64,
    pub slab_bounds: &'a Rect<f64>,
}

impl RegionalFeature {
    pub fn new<F: Feature + 'static>(
        bounding: Polygon<f64>,
        z_range: FeatureZRange,
        feature: F,
    ) -> SharedRegionalFeature {
        debug_assert!(!bounding.is_empty());
        debug!("creating new regional feature"; "centroid" => ?bounding.centroid(), "area" => bounding.unsigned_area(), "feature" => feature.name());

        let extended_z_range = feature.extend_z_range(z_range);
        debug!("extended feature z range"; "original" => ?z_range, "extended" => ?extended_z_range, "feature" => feature.name());

        Arc::new(RegionalFeature {
            bounding,
            z_range,
            feature: Mutex::new(Box::new(feature)),
        })
    }

    pub fn applies_to(&self, slab: SlabLocation, slab_bounds: &Rect<f64>) -> bool {
        // cheap z range check first
        let (slab_bottom, slab_top) = slab.slab.slice_range();
        let FeatureZRange(feature_bottom, feature_top) = self.z_range;

        if !(slab_bottom <= feature_top && feature_bottom <= slab_top) {
            // does not overlap
            return false;
        }

        // more expensive polygon check
        self.bounding.intersects(slab_bounds)
    }

    pub async fn apply_to_slab(&self, loc: SlabLocation, ctx: &mut ApplyFeatureContext<'_>) {
        let mut feature = self.feature.lock().await;
        feature.apply(loc, ctx, &self.bounding);
    }
}

impl FeatureZRange {
    pub fn new(min: GlobalSliceIndex, max: GlobalSliceIndex) -> Self {
        debug_assert!(min <= max);
        Self(min, max)
    }

    pub fn max_of(self, other: Self) -> Self {
        Self(self.0.min(other.0), self.1.max(other.1))
    }

    pub fn null() -> Self {
        Self(GlobalSliceIndex::top(), GlobalSliceIndex::bottom())
    }

    pub fn y_mut(&mut self) -> &mut GlobalSliceIndex {
        &mut self.1
    }
}

impl Debug for FeatureZRange {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}..={}]", self.0.slice(), self.1.slice())
    }
}

impl<'a> ApplyFeatureContext<'a> {
    pub fn slab_rando(&self, slab: SlabLocation) -> SmallRng {
        let seed = {
            // TODO faster hash
            let mut hasher = DefaultHasher::new();

            // hash unique slab location and planet seed
            slab.hash(&mut hasher);
            self.planet_seed.hash(&mut hasher);

            hasher.finish()
        };

        SmallRng::seed_from_u64(seed)
    }
}
