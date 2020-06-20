use unit::world::{WorldPoint, WorldPosition};
use world::{EdgeCost, WorldPath};

pub struct PathFollowing {
    path: WorldPath,
    final_target: WorldPoint,
    next: usize,
}

impl PathFollowing {
    pub fn new(path: WorldPath, target: WorldPoint) -> Self {
        Self {
            path,
            final_target: target,
            next: 0,
        }
    }

    pub fn next_waypoint(&mut self) -> Option<(WorldPoint, EdgeCost)> {
        let path_len = self.path.path().len();

        let node = self.path.path().get(self.next)?;

        let waypoint = if self.next == path_len - 1 {
            // last waypoint, use exact target point instead of waypoint block pos
            self.final_target
        } else {
            node.block.centred()
        };

        self.next += 1;
        Some((waypoint, node.exit_cost))
    }

    pub const fn target(&self) -> WorldPoint {
        self.final_target
    }

    pub fn waypoints(&self) -> impl Iterator<Item = &WorldPosition> {
        self.path
            .path()
            .iter()
            .skip(self.next)
            .map(|node| &node.block)
    }
}
