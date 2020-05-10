use unit::world::WorldPosition;
use world::{EdgeCost, WorldPath};

pub struct PathFollowing {
    path: WorldPath,
    next: usize,
}

impl PathFollowing {
    pub fn new(path: WorldPath) -> Self {
        Self { path, next: 0 }
    }

    pub fn last_waypoint(&self) -> Option<WorldPosition> {
        if self.next == 0 {
            None
        } else {
            self.path.path().get(self.next - 1).map(|node| node.block)
        }
    }

    pub fn next_waypoint(&mut self) -> Option<(WorldPosition, EdgeCost)> {
        let node = self.path.path().get(self.next)?;
        self.next += 1;
        Some((node.block, node.exit_cost))
    }
}
