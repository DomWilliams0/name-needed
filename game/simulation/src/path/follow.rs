use common::NormalizedFloat;
use common::*;
use unit::world::{WorldPoint, WorldPosition};
use world::{EdgeCost, SearchGoal, WorldPath};

use crate::path::PathToken;

#[derive(Debug)]
pub enum PathRequest {
    // TODO dont manually set the exact follow speed - choose a preset e.g. wander,dawdle,walk,fastwalk,run,sprint
    NavigateTo {
        target: WorldPoint,
        goal: SearchGoal,
        speed: NormalizedFloat,
        token: PathToken,
    },
    ClearCurrent,
}
pub struct PathFollowing {
    path: WorldPath,
    final_target: WorldPoint,
    next: usize,
}

impl PathFollowing {
    pub fn new(path: WorldPath, requested_target: WorldPoint, goal: SearchGoal) -> Self {
        let final_target = match goal {
            SearchGoal::Arrive => {
                // keep precision from requested target
                requested_target
            }
            _ => {
                // follow to new, adjusted target instead
                path.target().centred()
            }
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
