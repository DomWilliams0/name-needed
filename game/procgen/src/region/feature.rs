use geo::Polygon;
use std::sync::Arc;

/// Feature discovered at region initialization. Belongs in an Arc
pub struct RegionalFeature {
    /// 3d bounds around feature, only applies to slabs within this polygon
    bounding: Polygon<f64>,

    // TODO make this struct a dst and store trait object inline without extra indirection
    feature: Box<dyn Feature>,
}

pub type SharedRegionalFeature = Arc<RegionalFeature>;

pub trait Feature: Send + Sync {}

impl RegionalFeature {
    pub fn new<F: Feature + 'static>(bounding: Polygon<f64>, feature: F) -> SharedRegionalFeature {
        Arc::new(RegionalFeature {
            bounding,
            feature: Box::new(feature),
        })
    }
}

pub struct ForestFeature {}

impl Feature for ForestFeature {}

impl Default for ForestFeature {
    fn default() -> Self {
        Self {}
    }
}
