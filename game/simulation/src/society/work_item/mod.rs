//! Long-term [Job]-producing society-level tasks that multiple people can contribute to over time.

pub use spatial::{WorkItemHandle, WorkItems};
pub use work_item::{Location, WorkItem, WorkItemRef};
pub use work_items::*;

mod spatial;
mod work_item;
mod work_items;
