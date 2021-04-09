use std::sync::Arc;

use geo::prelude::*;
use geo::{Coordinate, MultiPolygon, Point, Rect};

use tokio::sync::Mutex;

use common::*;
use unit::world::{GlobalSliceIndex, SlabLocation, WorldPosition};

use crate::region::region::ChunkDescription;
use crate::region::PlanetPoint;
use crate::SlabGrid;
use geo::coords_iter::CoordsIter;
use geo_booleanop::boolean::BooleanOp;
use std::any::{Any, TypeId};
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;

/// Feature discovered at region initialization. Belongs in an Arc
pub struct RegionalFeature {
    /// NON ASYNC MUTEX, do not hold this across .awaits!!
    inner: parking_lot::RwLock<RegionalFeatureInner>,

    // TODO make this struct a dst and store trait object inline without extra indirection
    feature: Mutex<Box<dyn Feature>>,

    typeid: TypeId,
}

struct RegionalFeatureInner {
    /// 2d bounds around feature, only applies to slabs within this polygon
    // TODO can this be reduced to a single Polygon to reduce indirection?
    bounding: MultiPolygon<f64>,

    /// Inclusive bounds in the z direction for this feature
    z_range: FeatureZRange,
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
        bounding: &MultiPolygon<f64>,
    );

    /// Gut the other and absorb into this one.
    ///
    /// Must downcast other to Self and return false if mismatched
    fn merge_with(&mut self, other: &mut dyn Feature) -> bool;

    fn any_mut(&mut self) -> &mut dyn Any;
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
        bounding: MultiPolygon<f64>,
        z_range: FeatureZRange,
        feature: F,
    ) -> SharedRegionalFeature {
        debug_assert!(!bounding.is_empty());
        debug_assert!(bounding.iter().all(|p| !p.is_empty()));

        let extended_z_range = feature.extend_z_range(z_range);

        // TODO ensure these are optimised out
        let centroid = bounding.centroid();
        let area = bounding.unsigned_area();
        let name = feature.name();

        let arc = Arc::new(RegionalFeature {
            inner: parking_lot::RwLock::new(RegionalFeatureInner { bounding, z_range }),
            feature: Mutex::new(Box::new(feature)),
            typeid: TypeId::of::<F>(),
        });

        debug!("creating new regional feature"; "centroid" => ?centroid, "area" => ?area, "type" => name,
        "feature" => ?arc.ptr_debug(), "original range" => ?z_range, "extended range" => ?extended_z_range);

        arc
    }

    pub fn applies_to(&self, slab: SlabLocation, slab_bounds: &Rect<f64>) -> bool {
        let inner = self.inner.read();

        // cheap z range check first
        let (slab_bottom, slab_top) = slab.slab.slice_range();
        let slab_range = FeatureZRange::new(slab_bottom, slab_top);

        if !inner.z_range.overlaps_with(slab_range) {
            // does not overlap
            return false;
        }

        // more expensive polygon check
        inner.bounding.intersects(slab_bounds)
    }

    pub async fn apply_to_slab(&self, loc: SlabLocation, ctx: &mut ApplyFeatureContext<'_>) {
        let mut feature = self.feature.lock().await;
        let inner = self.inner.read();
        feature.apply(loc, ctx, &inner.bounding);
    }

    pub fn merge_with_bounds(
        &self,
        other_bounding: &MultiPolygon<f64>,
        other_z_range: FeatureZRange,
    ) {
        let mut inner = self.inner.write();

        inner.bounding = inner.bounding.union(other_bounding);
        inner.z_range = inner.z_range.max_of(other_z_range);
    }

    pub async fn merge_with_other(
        &self,
        other: SharedRegionalFeature,
    ) -> Result<(), (TypeId, TypeId)> {
        // debug_assert_eq!(
        //     self.typeid, other.typeid,
        //     "can't merge {:?} with {:?}",
        //     self.typeid, other.typeid
        // );

        let merged;
        {
            // try to merge features
            let mut other_feature = other.feature.lock().await;
            let mut this_feature = self.feature.lock().await;
            merged = this_feature.merge_with(&mut **other_feature);
        }

        if !merged {
            return Err((self.typeid, other.typeid));
        }

        {
            // now merge bounding polygons
            let other_inner = other.inner.read();
            self.merge_with_bounds(&other_inner.bounding, other_inner.z_range);
        }

        Ok(())
    }

    /// Dirty way to compare distinct instances by pointer value
    pub fn ptr_debug(self: &Arc<Self>) -> impl Debug {
        // TODO give each feature a guid instead
        let ptr = Arc::as_ptr(self);

        #[derive(Debug)]
        struct RegionalFeature(*const u8);

        RegionalFeature(ptr as *const _)
    }

    pub fn display<'a>(self: &'a Arc<Self>) -> impl Display + 'a {
        let ptr = Arc::as_ptr(self);

        struct RegionalFeature<'a>(*const u8, &'a Arc<super::RegionalFeature>);

        impl Display for RegionalFeature<'_> {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                write!(f, "{:?}: ", self.0)?;
                if let Ok(feature) = self.1.feature.try_lock() {
                    write!(f, "{:?}", &feature)
                } else {
                    write!(f, "<locked>")
                }
            }
        }

        RegionalFeature(ptr as *const _, self)
    }

    /// Assumes [applies_to] has already been checked for slab. Pretty expensive, and panics
    /// if in invalid region
    pub fn applies_to_block(&self, block: WorldPosition) -> bool {
        let pos = PlanetPoint::from_block(block).unwrap(); // cheeky panic
        let inner = self.inner.read();
        inner.bounding.contains(&Coordinate::from(pos.get_array()))
    }

    /// Nop if feature mutex is not immediately available, i.e. does not block
    pub fn bounding_points(
        &self,
        z_range: (GlobalSliceIndex, GlobalSliceIndex),
        per_point: impl FnMut(PlanetPoint),
    ) {
        if let Some(inner) = self.inner.try_read() {
            if inner
                .z_range
                .overlaps_with(FeatureZRange::new(z_range.0, z_range.1))
            {
                inner
                    .bounding
                    .iter()
                    .flat_map(|poly| poly.exterior().points_iter())
                    .map(|Point(Coordinate { x, y })| PlanetPoint::new(x, y))
                    .for_each(per_point);
            }
        }
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

    pub fn overlaps_with(self, other: Self) -> bool {
        other.0 <= self.1 && self.0 <= other.1
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
        SmallRng::seed_from_u64(slab_rando_seed(slab, self.planet_seed))
    }
}

fn slab_rando_seed(slab: SlabLocation, planet_seed: u64) -> u64 {
    // TODO faster hash
    let mut hasher = DefaultHasher::new();

    // hash unique slab location and planet seed
    slab.hash(&mut hasher);
    planet_seed.hash(&mut hasher);

    hasher.finish()
}

impl Debug for RegionalFeature {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let inner = self.inner.try_read();
        let feature = self.feature.try_lock().ok();
        let mut dbg = f.debug_struct("RegionalFeature");
        match inner {
            Some(inner) => {
                dbg.field(
                    "bounding point count",
                    &inner.bounding.coords_iter().count(),
                );
                dbg.field("z range", &inner.z_range);
            }
            None => {
                dbg.field("inner", &"<locked>");
            }
        }

        match feature {
            Some(feature) => {
                dbg.field("name", &feature.name());
                dbg.field("feature", &*feature);
            }
            None => {
                dbg.field("feature", &"<locked>");
            }
        }

        dbg.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slab_rando_differs() {
        let a = SlabLocation::new(7, (2, 3));
        let b = SlabLocation::new(7, (3, 3));
        let c = SlabLocation::new(8, (2, 3));

        let planet1 = 1238123;
        let planet2 = 9182391;

        let inputs = vec![a, b, c]
            .into_iter()
            .cartesian_product(vec![planet1, planet2].into_iter())
            .collect_vec();

        let mut seeds1 = inputs
            .iter()
            .copied()
            .map(|(slab, planet)| slab_rando_seed(slab, planet))
            .collect_vec();

        let seeds2 = inputs
            .iter()
            .copied()
            .map(|(slab, planet)| slab_rando_seed(slab, planet))
            .collect_vec();

        // reproducible
        assert_eq!(seeds1, seeds2);

        // no dups
        seeds1.sort();
        assert_eq!(seeds1.iter().copied().dedup().collect_vec(), seeds1);
    }
}
