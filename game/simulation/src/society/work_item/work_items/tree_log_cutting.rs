use crate::society::work_item::work_item::WorkItemImpl;
use common::*;

#[derive(Debug, Default)]
pub struct TreeLogCuttingWorkItem {}

impl WorkItemImpl for TreeLogCuttingWorkItem {}

impl Display for TreeLogCuttingWorkItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "cutting up tree into logs")
    }
}
