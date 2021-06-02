use crate::society::work_item::spatial::WorkItemHandle;
use common::geo::prelude::HasDimensions;
use common::*;
use geo::bounding_rect::BoundingRect;
use geo::coords_iter::CoordsIter;
use geo::Geometry;
use std::cell::RefCell;
use std::hash::Hasher;
use std::ops::Deref;
use std::sync::Arc;
use unit::world::WorldPoint;

#[derive(Debug)]
pub struct WorkItem {
    loc: Location,
    inner: Box<dyn WorkItemImpl>,
}

#[derive(Debug)]
pub struct Location {
    loc_xy: Geometry<f32>,
    z: f32,
}

/// Shared reference and a society-unique handle
#[derive(Clone)]
pub struct WorkItemRef(Arc<RefCell<WorkItem>>, WorkItemHandle);

/// Display impl should fit into "Working on <impl>"
pub trait WorkItemImpl: Debug + Display {}

impl WorkItem {
    pub fn new(loc: Location, wi: impl WorkItemImpl + 'static) -> Self {
        WorkItem {
            loc,
            inner: Box::new(wi),
        }
    }

    /// A point nearby to the work item, dependant on the type of the [Location] polygon
    pub fn nearby(&self) -> WorldPoint {
        let coord = self.loc.loc_xy.coords_iter().next().unwrap(); // already checked polygon is not empty
        WorldPoint::new_unchecked(coord.x, coord.y, self.loc.z)
    }

    fn aabb(&self) -> rstar::AABB<[f32; 3]> {
        let bounds_xy = self.loc.loc_xy.bounding_rect().expect("invalid polygon");
        let min = bounds_xy.min();
        let max = bounds_xy.max();
        // 2d rect at the correct z coordinate
        rstar::AABB::from_corners([min.x, min.y, self.loc.z], [max.x, max.y, self.loc.z])
    }
}

impl Location {
    pub fn new(loc_xy: impl Into<Geometry<f32>>, z: f32) -> Self {
        let loc_xy = loc_xy.into();
        // TODO return option instead of asserts
        assert!(!loc_xy.is_empty(), "empty polygon");
        assert!(loc_xy
            .coords_iter()
            .all(|c| c.x.is_finite() && c.y.is_finite()));

        Self { loc_xy, z }
    }
}

impl WorkItemRef {
    pub(in crate::society::work_item) fn new(wi: WorkItem, handle: WorkItemHandle) -> Self {
        Self(Arc::new(RefCell::new(wi)), handle)
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub const fn handle(&self) -> WorkItemHandle {
        self.1
    }
}

impl Debug for WorkItemRef {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "WorkItem(")?;

        match self.0.try_borrow() {
            Err(_) => write!(f, "<locked>)"),
            Ok(wi) => write!(f, "{:?} | at {:?})", wi.inner, wi.loc,),
        }
    }
}

impl Display for WorkItemRef {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.0.try_borrow() {
            Err(_) => write!(f, "something"),
            Ok(wi) => Display::fmt(&*wi.inner, f),
        }
    }
}

impl Deref for WorkItemRef {
    type Target = RefCell<WorkItem>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Hash for WorkItemRef {
    /// Only considers unique society handle
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.handle().hash(state)
    }
}

impl PartialEq for WorkItemRef {
    /// Only considers unique society handle
    fn eq(&self, other: &Self) -> bool {
        self.handle() == other.handle()
    }
}

impl Eq for WorkItemRef {}

impl rstar::RTreeObject for WorkItemRef {
    type Envelope = rstar::AABB<[f32; 3]>;

    fn envelope(&self) -> Self::Envelope {
        self.0.borrow().aabb()
    }
}

impl rstar::PointDistance for WorkItemRef {
    fn distance_2(&self, point: &[f32; 3]) -> f32 {
        self.0.borrow().aabb().distance_2(point)
    }
}
