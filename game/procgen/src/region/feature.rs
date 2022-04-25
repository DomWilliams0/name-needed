use std::any::{Any, TypeId};
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::hint::unreachable_unchecked;
use std::ops::Deref;
use std::sync::{Arc, Weak};

use geo::algorithm::map_coords::MapCoordsInplace;
use geo::concave_hull::ConcaveHull;
use geo::coords_iter::CoordsIter;
use geo::prelude::*;
use geo::{Coordinate, Geometry, LineString, MultiPoint, MultiPolygon, Point, Polygon, Rect};
use geo_booleanop::boolean::{BooleanOp, Operation};
use rand_distr::Bernoulli;
use tokio::sync::Mutex;

use common::*;
use unit::world::{BlockCoord, BlockPosition, GlobalSliceIndex, SlabLocation, WorldPosition};

use crate::region::region::{ChunkDescription, ChunkHeightMap};
use crate::region::subfeature::{SharedSubfeature, Subfeature};
use crate::region::subfeatures::Fauna;
use crate::region::unit::RegionLocation;
use crate::region::PlanetPoint;
use crate::{PlanetParams, PlanetParamsRef};

/// Feature discovered during region initialization.
/// Generic param should be `dyn Feature`, but can't use defaults and const params at the same
/// time :(
pub struct RegionalFeatureRaw<F: ?Sized + Feature, const SIZE: usize> {
    /// NON ASYNC MUTEX, do not hold this across .awaits!!
    inner: parking_lot::RwLock<RegionalFeatureInner<SIZE>>,

    typeid: TypeId,
    feature: Mutex<F>,
}

/// Typedef to get around needing to specify dummy generic parameter
pub type RegionalFeature<const SIZE: usize> = RegionalFeatureRaw<dyn Feature, SIZE>;

struct RegionalFeatureInner<const SIZE: usize> {
    /// 2d bounds around feature, only applies to slabs within this polygon
    bounding: RegionalFeatureBoundary,

    /// Inclusive bounds in the z direction for this feature
    z_range: FeatureZRange,

    /// The regions that reference this feature
    regions: Vec<RegionLocation<SIZE>>,
}

/// Either Polygon or MultiPolygon
pub struct RegionalFeatureBoundary(Geometry<f64>);

/// Inclusive bounds in the z direction for a feature
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct FeatureZRange(GlobalSliceIndex, GlobalSliceIndex);

pub type SharedRegionalFeature<const SIZE: usize> = Arc<RegionalFeature<SIZE>>;
pub type WeakRegionalFeatureRef<const SIZE: usize> = Weak<RegionalFeature<SIZE>>;

pub trait Feature: Send + Sync + Debug {
    fn name(&self) -> &'static str;

    /// Increase z range based on this feature e.g. tree height
    fn extend_z_range(&self, range: FeatureZRange) -> FeatureZRange;

    fn apply(&mut self, ctx: &mut ApplyFeatureContext<'_>, bounding: &RegionalFeatureBoundary);

    /// Gut the other and absorb into this one.
    ///
    /// Must downcast other to Self and return false if mismatched
    fn merge_with(&mut self, other: &mut dyn Feature) -> bool;

    fn any_mut(&mut self) -> &mut dyn Any;
}

/// Context for applying a feature to a slab
pub struct ApplyFeatureContext<'a> {
    pub slab: SlabLocation,
    pub chunk_desc: &'a ChunkDescription,
    pub params: PlanetParamsRef,
    pub slab_bounds: &'a Rect<f64>,
    pub subfeatures_tx: tokio::sync::mpsc::UnboundedSender<SharedSubfeature>,
}

impl<const SIZE: usize> RegionalFeatureRaw<dyn Feature, SIZE> {
    pub fn new<F: Feature + 'static>(
        bounding: RegionalFeatureBoundary,
        z_range: FeatureZRange,
        feature: F,
    ) -> SharedRegionalFeature<SIZE> {
        debug_assert!(!bounding.is_empty());

        let extended_z_range = feature.extend_z_range(z_range);

        // TODO ensure these are optimised out
        let centroid = bounding.centroid();
        let area = bounding.unsigned_area();
        let name = feature.name();

        let arc = Arc::new(RegionalFeatureRaw {
            inner: parking_lot::RwLock::new(RegionalFeatureInner {
                bounding,
                z_range,
                regions: Vec::new(),
            }),
            feature: Mutex::new(feature),
            typeid: TypeId::of::<F>(),
        }) as Arc<RegionalFeature<SIZE>>;

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

    pub async fn apply_to_slab(&self, ctx: &mut ApplyFeatureContext<'_>) {
        let mut feature = self.feature.lock().await;
        let inner = self.inner.read();
        feature.apply(ctx, &inner.bounding);
    }

    /// Gut the other and absorb it into this's bounds
    pub fn merge_with_bounds(
        &self,
        other_bounding: RegionalFeatureBoundary,
        other_z_range: FeatureZRange,
    ) {
        let mut inner = self.inner.write();

        let (n_before, area_before): (usize, f64);
        #[cfg(debug_assertions)]
        {
            n_before = inner.bounding.coords_iter().count();
            area_before = inner.bounding.unsigned_area();
        }

        inner.bounding.merge(other_bounding);

        #[cfg(debug_assertions)]
        {
            let n_after = inner.bounding.coords_iter().count();
            let area_after = inner.bounding.unsigned_area();
            trace!("feature polygon merge"; "after" => ?(n_after, area_after), "before" => ?(n_before, area_before));
        }

        inner.z_range = inner.z_range.max_of(other_z_range);
    }

    /// `self` and `other` must not be the same feature, because feature mutex is not reentrant.
    /// Guts `other`.
    /// Returns vec of regions that reference the `other` feature
    pub async fn merge_with_other(
        self: &Arc<Self>,
        other: &SharedRegionalFeature<SIZE>,
    ) -> Result<Vec<RegionLocation<SIZE>>, (TypeId, TypeId)> {
        debug_assert!(!Arc::ptr_eq(self, other));

        // debug_assert_eq!(
        //     self.typeid, other.typeid,
        //     "can't merge {:?} with {:?}",
        //     self.typeid, other.typeid
        // );

        let merged;
        {
            // TODO this only serves as an assert - revisit the need to merge non-rasterised features
            let mut other_feature = other.feature.lock().await;
            let mut this_feature = self.feature.lock().await;
            merged = this_feature.merge_with(&mut *other_feature);
        }

        if !merged {
            return Err((self.typeid, other.typeid));
        }

        let regions;
        {
            // now merge bounding polygons, gutting the other
            let mut other_inner = other.inner.write();
            let other_bounding = std::mem::take(&mut other_inner.bounding);
            self.merge_with_bounds(other_bounding, other_inner.z_range);

            // steal regions
            regions = std::mem::take(&mut other_inner.regions);
        }

        Ok(regions)
    }

    /// Dirty way to compare distinct instances by pointer value
    pub fn ptr_debug(self: &Arc<Self>) -> impl Debug {
        // TODO give each feature a guid instead
        let ptr = Arc::as_ptr(self);

        #[derive(Debug)]
        struct RegionalFeature(*const u8);

        RegionalFeature(ptr as *const _)
    }

    #[allow(clippy::needless_lifetimes)] // false positive, you don't knooow me
    pub fn display<'a>(self: &'a Arc<Self>) -> impl Display + 'a {
        let ptr = Arc::as_ptr(self);

        struct RegionalFeature<'a, const SIZE: usize>(
            *const u8,
            &'a Arc<super::RegionalFeature<SIZE>>,
        );

        impl<const SIZE: usize> Display for RegionalFeature<'_, SIZE> {
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

    /// Unique opaque value per feature
    pub fn unique_id(self: &Arc<Self>) -> usize {
        Arc::as_ptr(self) as *const () as usize
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
                    .coords_iter()
                    .map(|Coordinate { x, y }| PlanetPoint::new(x, y))
                    .for_each(per_point);
            }
        }
    }

    pub fn is_boundary_empty(&self) -> bool {
        self.inner.read().bounding.is_empty()
    }

    pub fn add_region(&self, region: RegionLocation<SIZE>) {
        self.add_regions(once(region));
    }

    pub fn add_regions(&self, regions: impl Iterator<Item = RegionLocation<SIZE>> + Clone) {
        let mut inner = self.inner.write();

        if cfg!(debug_assertions) {
            let regions = regions.clone();
            for region in regions {
                assert!(
                    !inner.regions.contains(&region),
                    "duplicate region {:?} in feature {:?}",
                    region,
                    self
                );
            }
        }
        inner.regions.extend(regions);
    }

    // TODO create guard struct/owned ref to avoid needing to clone the vec temporarily
    pub fn regions(&self) -> Vec<RegionLocation<SIZE>> {
        let inner = self.inner.read();
        inner.regions.clone()
    }
}

impl<F: ?Sized + Feature, const SIZE: usize> Drop for RegionalFeatureRaw<F, SIZE> {
    fn drop(&mut self) {
        let ptr = self as *mut _;
        trace!("dropping feature {:?} @ {:?}", self, ptr);
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
    pub fn slab_rando(&self) -> SmallRng {
        SmallRng::seed_from_u64(slab_rando_seed(self.slab, self.params.seed()))
    }

    /// To be called by [Feature]s during application to a slab
    ///
    /// root assumed to be within this slab
    pub fn queue_subfeature<F: Subfeature + 'static>(
        &mut self,
        subfeature: F,
        root: WorldPosition,
    ) {
        let _ = self
            .subfeatures_tx
            .send(SharedSubfeature::new(subfeature, root));
    }
}

fn slab_rando_seed(slab: SlabLocation, planet_seed: u64) -> u64 {
    // TODO faster and non-random hash
    let mut hasher = DefaultHasher::new();

    // hash unique slab location and planet seed
    slab.hash(&mut hasher);
    planet_seed.hash(&mut hasher);

    hasher.finish()
}

impl<F: ?Sized + Feature, const SIZE: usize> Debug for RegionalFeatureRaw<F, SIZE> {
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
                dbg.field("feature", &feature);
            }
            None => {
                dbg.field("feature", &"<locked>");
            }
        }

        dbg.finish()
    }
}

impl RegionalFeatureBoundary {
    /// Merges the other into this via union
    pub fn merge(&mut self, other: RegionalFeatureBoundary) {
        // check for empty polygons
        match (self.0.is_empty(), other.0.is_empty()) {
            (true, false) => {
                trace!("this boundary is empty but other isn't, just take the other");
                *self = other;
            }
            (false, true) => {
                trace!("other boundary is empty but this isn't, merge is nop");
            }
            (false, false) => {
                // neither is empty, actually merge (normal case)
                *self = Self::new_multi_as_is(self.union(&other));
                debug_assert!(!self.is_empty(), "union of 2 non-empty boundaries is empty");
            }
            (true, true) => {
                // both are empty, oh god
                unreachable!("can't merge 2 empty boundaries")
            }
        };

        self.iter_polys_mut(|points| {
            let new_points = Self::simplify_boundary(std::mem::take(points));
            let old = std::mem::replace(points, new_points);
            debug_assert!(old.is_empty());
            std::mem::forget(old);
        });

        debug_assert!(
            self.geometry().coords_iter().count() > 0,
            "simplified merged boundary is empty"
        );
    }

    fn iter_polys_mut(&mut self, mut per_poly: impl FnMut(&mut Vec<Coordinate<f64>>)) {
        match &mut self.0 {
            Geometry::Polygon(p) => p.exterior_mut(|ext| per_poly(&mut ext.0)),
            Geometry::MultiPolygon(p) => {
                for poly in &mut p.0 {
                    poly.exterior_mut(|ext| per_poly(&mut ext.0));
                }
            }
            _ => unsafe { unreachable_debug() },
        }
    }

    /*    fn iter_polys(&self, mut per_poly: impl FnMut(&Vec<Coordinate<f64>>)) {
            match &self.0 {
                Geometry::Polygon(p) => per_poly(&p.exterior().0),
                Geometry::MultiPolygon(p) => {
                    for poly in &p.0 {
                        per_poly(&poly.exterior().0);
                    }
                }
                _ => unsafe { unreachable_debug() },
            }
        }

        /// Collapse into point cloud
        #[deprecated]
        fn collapse(self, extend_me: Option<Vec<Coordinate<f64>>>) -> Vec<Coordinate<f64>> {
            let extract_points = |p: Polygon<f64>| {
                let (ext, int) = p.into_inner();
                if !int.is_empty() {
                    warn!("interior is not empty"; "interiors" => int.len());
                }
                ext.0
            };

            match self.0 {
                Geometry::Polygon(p) => {
                    let more_points = extract_points(p);
                    match extend_me {
                        None => more_points,
                        Some(mut p) => {
                            p.extend(more_points);
                            p
                        }
                    }
                }
                Geometry::MultiPolygon(p) => {
                    let mut points = extend_me;
                    for poly in p.0 {
                        match points.as_mut() {
                            None => points = Some(extract_points(poly)),
                            Some(p) => p.extend(extract_points(poly)),
                        }
                    }

                    debug_assert!(points.is_some(), "no polygons?");
                    points.unwrap_or_default()
                }
                _ => unsafe { unreachable_debug() },
            }
        }
    */
    /// Points should be from row scanning, and pre-expanded horizontally.
    ///
    /// * traces boundary as convex hull
    /// * simplifies boundary
    /// * expands polygon vertically
    ///
    /// Returns (boundary, number of points in polygon)
    pub fn new<const SIZE: usize>(
        points: Vec<Point<f64>>,
        y_range: (f64, f64),
        params: &PlanetParams,
    ) -> (Self, usize) {
        let points = MultiPoint(points);

        // trace boundary
        let mut polygon = points.concave_hull(params.feature_concavity);

        // simplify boundary
        polygon = {
            let (exterior, interior) = polygon.into_inner();
            let orig_len = exterior.0.len();
            let simplified = Self::simplify_boundary(exterior.0);
            debug!(
                "simplified feature boundary from {before} to {after}",
                before = orig_len,
                after = simplified.len()
            );
            Polygon::new(LineString(simplified), interior)
        };

        // run polygon simplification after basic simplification to ensure points are still aligned
        // from row scanning
        // polygon = polygon.simplify(&(PlanetPoint::PER_BLOCK * 4.0));

        // expand top and bottom X% points
        let (bottom_y, top_y) = {
            let threshold = params.region_feature_vertical_expansion_threshold; // %
            let (min, max) = y_range;
            let diff = (max - min) * threshold;
            debug_assert!(diff > 0.0);
            (min + diff, max - diff)
        };

        let expansion = params.region_feature_expansion as f64
            * crate::region::unit::PlanetPoint::<SIZE>::PER_BLOCK;
        polygon.map_coords_inplace(|&(x, mut y)| {
            if y < bottom_y {
                // expand downwards
                y -= expansion * 1.0;
            } else if y > top_y {
                // expand upwards
                y += expansion * 1.0;
            }

            (x, y)
        });

        let n = polygon.exterior().0.len();
        (Self(Geometry::Polygon(polygon)), n)
    }

    fn simplify_boundary(points: Vec<Coordinate<f64>>) -> Vec<Coordinate<f64>> {
        use common::cgmath::Vector2;
        let mut new_points = Vec::with_capacity(points.len()); // worst case no simplification
        let orig_last = points.last().copied();

        let mut last_delta = Vector2::zero();
        for (a, b) in points.into_iter().tuple_windows() {
            let this_delta = Vector2::from((b - a).x_y()).normalize();

            // ok to compare to f64 vectors exactly for equality, because all points have been created
            // on block boundaries
            if this_delta != last_delta {
                // new direction!

                // add start point for new line
                new_points.push(a);

                // track delta
                last_delta = this_delta;
            }

            // otherwise continue in direction, skipping redundant points
        }

        // add last point untouched if necessary
        if let Some((orig_last, new_last)) = orig_last.zip(new_points.last()) {
            if orig_last != *new_last {
                new_points.push(orig_last);
            }
        }

        new_points
    }

    #[inline]
    pub fn empty() -> Self {
        Self(Geometry::MultiPolygon(MultiPolygon(vec![])))
    }

    #[inline]
    pub fn geometry(&self) -> &Geometry<f64> {
        &self.0
    }

    fn new_multi_as_is(mut polygon: MultiPolygon<f64>) -> Self {
        if polygon.0.len() == 1 {
            // indirection removed B-)
            Self(Geometry::Polygon(polygon.0.remove(0)))
        } else {
            Self(Geometry::MultiPolygon(polygon))
        }
    }

    #[cfg(test)]
    pub fn new_as_is(polygon: Polygon<f64>) -> Self {
        Self(Geometry::Polygon(polygon))
    }
}

impl Default for RegionalFeatureBoundary {
    fn default() -> Self {
        Self::empty()
    }
}

#[inline]
unsafe fn unreachable_debug() -> ! {
    if cfg!(debug_assertions) {
        unreachable!()
    } else {
        unreachable_unchecked()
    }
}

impl Deref for RegionalFeatureBoundary {
    type Target = Geometry<f64>;

    fn deref(&self) -> &Self::Target {
        self.geometry()
    }
}

impl Centroid for RegionalFeatureBoundary {
    type Output = Option<Point<f64>>;

    fn centroid(&self) -> Self::Output {
        match &self.0 {
            Geometry::Polygon(p) => p.centroid(),
            Geometry::MultiPolygon(p) => p.centroid(),
            _ => {
                // safety: struct cant be created with another type
                unsafe { unreachable_debug() }
            }
        }
    }
}

impl BooleanOp<f64> for RegionalFeatureBoundary {
    fn boolean(&self, rhs: &Self, operation: Operation) -> MultiPolygon<f64> {
        use Geometry::*;
        match (&self.0, &rhs.0) {
            (Polygon(a), Polygon(b)) => a.boolean(b, operation),
            (Polygon(a), MultiPolygon(b)) => a.boolean(b, operation),
            (MultiPolygon(a), Polygon(b)) => a.boolean(b, operation),
            (MultiPolygon(a), MultiPolygon(b)) => a.boolean(b, operation),
            _ => {
                // safety: struct cant be created with another type
                unsafe { unreachable_debug() }
            }
        }
    }
}
impl BooleanOp<f64, Polygon<f64>> for RegionalFeatureBoundary {
    fn boolean(&self, rhs: &Polygon<f64>, operation: Operation) -> MultiPolygon<f64> {
        use Geometry::*;
        match &self.0 {
            Polygon(a) => a.boolean(rhs, operation),
            MultiPolygon(a) => a.boolean(rhs, operation),
            _ => {
                // safety: struct cant be created with another type
                unsafe { unreachable_debug() }
            }
        }
    }
}

pub(crate) async fn generate_loose_subfeatures(ctx: &mut ApplyFeatureContext<'_>) {
    let mut rando = SmallRng::from_entropy(); // non deterministic

    let skip_distr = Bernoulli::new(0.1).unwrap();

    // occasional fauna
    for (i, block) in ctx.chunk_desc.blocks().iter().enumerate() {
        let biome = block.biome();

        if !biome.has_fauna() || !skip_distr.sample(&mut rando) {
            continue;
        }

        let fauna_species = block
            .biome()
            .choose_fauna(&mut rando)
            .expect("bad fauna biome definition");

        let subfeature = Fauna {
            species: fauna_species,
        };
        let root = {
            let [cx, cy, _]: [i32; 3] = ChunkHeightMap::unflatten(i).unwrap(); // certainly valid
            debug_assert!(BlockCoord::try_from(cx).is_ok()); // ensure dims are fine before casts
            BlockPosition::new_unchecked(cx as BlockCoord, cy as BlockCoord, block.ground())
                .to_world_position(ctx.slab.chunk)
        };
        if let Err(err) = ctx
            .subfeatures_tx
            .send(SharedSubfeature::new(subfeature, root))
        {
            warn!("failed to send subfeature"; "err" => %err);
            break;
        }
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

    #[test]
    fn simplify_polygon() {
        let p = |x, y| geo::Coordinate::<f64> { x, y };
        let points = vec![
            // start at origin
            p(0.0, 0.0),
            // straight line to the right
            p(1.0, 0.0), // redundant
            p(2.0, 0.0), // redundant
            p(3.0, 0.0), // last point in this line, keep it
            // up
            p(3.0, 1.0),
            p(3.0, 2.0),
            p(3.0, 3.0),
            // rando
            p(-5.0, 6.0),
            // back to start
            p(0.0, 0.0),
        ];

        let simplified = RegionalFeatureBoundary::simplify_boundary(points);
        eprintln!("{:#?}", simplified);
        assert_eq!(
            simplified,
            vec![
                p(0.0, 0.0),
                p(3.0, 0.0),
                p(3.0, 3.0),
                p(-5.0, 6.0),
                p(0.0, 0.0)
            ]
        );
    }
}
