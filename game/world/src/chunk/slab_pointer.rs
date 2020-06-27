use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use std::sync::Arc;

use crate::chunk::slab::Slab;

/// CoW slab
#[derive(Clone)]
pub(crate) struct SlabPointer(Arc<Slab>);

pub trait DeepClone {
    fn deep_clone(&self) -> Self;
}

impl SlabPointer {
    #[inline(always)]
    pub fn new(slab: Slab) -> Self {
        Self(Arc::new(slab))
    }
    pub fn cow_clone(&mut self) -> &mut Slab {
        Arc::make_mut(&mut self.0)
    }

    pub fn expect_mut(&mut self) -> &mut Slab {
        Arc::get_mut(&mut self.0).expect("expected to be the only slab reference")
    }

    pub fn is_exclusive(&self) -> bool {
        Arc::strong_count(&self.0) == 1
    }
}

impl Deref for SlabPointer {
    type Target = Slab;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DeepClone for SlabPointer {
    fn deep_clone(&self) -> Self {
        let slab = &*self.0;
        Self::new(slab.clone())
    }
}

impl Debug for SlabPointer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let ptr = Arc::into_raw(self.0.clone());
        let ret = write!(f, "SlabPointer({:?})", ptr);
        unsafe { Arc::from_raw(ptr) };
        ret
    }
}
