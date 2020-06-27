use unit::world::{WorldPoint, WorldPosition};
use world::{EdgeCost, SearchGoal, WorldPath};

pub struct PathFollowing {
    path: WorldPath,
    final_target: WorldPoint,
    next: usize,
}

impl PathFollowing {
    pub fn new(path: WorldPath, requested_target: WorldPoint, goal: SearchGoal) -> Self {
        let final_target = if let SearchGoal::Adjacent = goal {
            // follow to new, adjacent target instead
            path.target().centred()
        } else {
            // keep precision from requested target
            requested_target
        };
        Self {
            path,
            final_target,
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
