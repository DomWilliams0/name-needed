use crate::ai::dse::BreakBlockDse;
use crate::ai::AiContext;
use ai::Dse;
use unit::world::WorldPosition;

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub enum Task {
    BreakBlock(WorldPosition),
    // TODO HaulBlocks(block type, near position)
    // TODO PlaceBlocks(block type, at position)
}

// TODO temporary box allocation is gross
impl From<&Task> for Box<dyn Dse<AiContext>> {
    fn from(task: &Task) -> Self {
        match task {
            Task::BreakBlock(range) => Box::new(BreakBlockDse(*range)),
        }
    }
}
