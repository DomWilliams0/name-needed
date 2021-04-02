use crate::region::region::ChunkLocation;
use crate::SlabGrid;
use common::*;
use geo::prelude::*;
use geo::{Polygon, Rect};
use std::sync::Arc;
use tokio::sync::Mutex;
use unit::world::{GlobalSliceIndex, SlabLocation};

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

    fn apply(&mut self, loc: SlabLocation, slab: &mut SlabGrid);
}

impl RegionalFeature {
    pub fn new<F: Feature + 'static>(
        bounding: Polygon<f64>,
        z_range: FeatureZRange,
        feature: F,
    ) -> SharedRegionalFeature {
        debug_assert!(!bounding.is_empty());

        let extended_z_range = feature.extend_z_range(z_range);
        debug!("extended feature z range"; "original" => ?z_range, "extended" => ?extended_z_range, "feature" => feature.name());

        Arc::new(RegionalFeature {
            bounding,
            z_range,
            feature: Mutex::new(Box::new(feature)),
        })
    }

    pub fn applies_to(&self, slab: SlabLocation) -> bool {
        // cheap z range check first
        let (slab_bottom, slab_top) = slab.slab.slice_range();
        let FeatureZRange(feature_bottom, feature_top) = self.z_range;

        if !(slab_bottom <= feature_top && feature_bottom <= slab_top) {
            // does not overlap
            return false;
        }

        // more expensive polygon check
        let slab_bounds = {
            let ChunkLocation(min_x, min_y) = slab.chunk;
            let ChunkLocation(max_x, max_y) = slab.chunk + (1, 1); // 1 chunk over diagonally
            Rect::new((min_x as f64, min_y as f64), (max_x as f64, max_y as f64))
        };

        self.bounding.intersects(&slab_bounds)
    }

    pub async fn apply_to_slab(&self, loc: SlabLocation, slab: &mut SlabGrid) {
        let mut feature = self.feature.lock().await;
        feature.apply(loc, slab);
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
}

#[derive(Debug)]
pub struct ForestFeature {}

impl Feature for ForestFeature {
    fn name(&self) -> &'static str {
        "forest"
    }

    fn extend_z_range(&self, mut range: FeatureZRange) -> FeatureZRange {
        // tree height
        // TODO remove magic value, use real max tree height
        range.1 += 16;

        // TODO tree roots

        range
    }

    fn apply(&mut self, loc: SlabLocation, slab: &mut SlabGrid) {
        // TODO generate tree locations with poisson disk sampling
        // TODO attempt to place tree model at location in this slab
        // TODO if a tree/subfeature is cut off, keep track of it as a continuation for the neighbouring slab
    }
}

impl Default for ForestFeature {
    fn default() -> Self {
        Self {}
    }
}

impl Debug for FeatureZRange {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}..={}]", self.0.slice(), self.1.slice())
    }
}
