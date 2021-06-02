use crate::society::work_item::work_item::{WorkItem, WorkItemRef};
use common::*;

/// All active work items in a society, represented in a spatial data structure
pub struct WorkItems {
    rtree: rstar::RTree<WorkItemRef>,
    next_handle: WorkItemHandle,
}

/// Opaque unique handle to work items within a society. Used for comparisons
#[derive(Eq, PartialEq, Copy, Clone, Hash)]
pub struct WorkItemHandle(u32);

impl Default for WorkItems {
    fn default() -> Self {
        Self {
            // TODO consider rtree params
            rtree: rstar::RTree::new(),
            next_handle: WorkItemHandle(100),
        }
    }
}

impl WorkItems {
    pub fn add(&mut self, wi: WorkItem) -> WorkItemRef {
        let wi = WorkItemRef::new(wi, self.next_handle);
        self.next_handle.0 = self
            .next_handle
            .0
            .checked_add(1)
            .expect("overflow on work items");

        self.rtree.insert(wi.clone());
        wi
    }
}

impl Debug for WorkItemHandle {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "WorkItemHandle({:#x})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::society::work_item::work_item::{Location, WorkItemImpl};
    use geo::{Geometry, MultiLineString, MultiPolygon, Polygon, Rect, Triangle};

    #[derive(Debug)]
    struct DummyWorkItem;

    impl WorkItemImpl for DummyWorkItem {}

    impl Display for DummyWorkItem {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            Ok(())
        }
    }

    #[test]
    fn basic() {
        let mut items = WorkItems::default();

        let a = items.add(WorkItem::new(
            Location::new(Rect::new((0.0, 0.0), (2.0, 2.0)), 4.0),
            DummyWorkItem,
        ));

        let b = items.add(WorkItem::new(
            Location::new(Rect::new((5.0, 5.0), (2.0, 2.0)), 2.0),
            DummyWorkItem,
        ));

        assert_ne!(a.handle(), b.handle());
    }
}
