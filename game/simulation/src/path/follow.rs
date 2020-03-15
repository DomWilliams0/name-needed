use common::*;
use unit::world::{WorldPoint, WorldPosition};
use world::{WorldPath, WorldPathSlice};

pub struct PathFollowing {
    path: WorldPath,
    next: usize,
    changed: bool,
}

impl PathFollowing {
    pub fn new(path: WorldPath) -> Self {
        Self {
            path,
            next: 0,
            changed: false,
        }
    }

    pub fn next_waypoint(&mut self, current_pos: &WorldPoint) -> Option<(WorldPosition, bool)> {
        let mut is_final = false;
        let mut changed = false;

        // check distance to current
        let (waypoint, _cost) = &self.path.0[self.next];
        let distance2 = {
            let from = Point2 {
                x: current_pos.0,
                y: current_pos.1,
            };

            let waypoint = WorldPoint::from(*waypoint);
            let to = Point2 {
                x: waypoint.0,
                y: waypoint.1,
            };

            from.distance2(to)
        };

        if distance2 < 0.5f32.powi(2) {
            // move on to next waypoint
            self.next += 1;
            is_final = self.next + 1 == self.path.0.len();
            changed = true;
        }

        self.changed = changed;
        self.path
            .0
            .get(self.next)
            .map(|&(wp, _cost)| (wp, is_final))
    }

    pub fn changed(&self) -> bool {
        self.changed
    }

    pub fn path_remaining(&self) -> WorldPathSlice {
        &self.path.0[self.next..]
    }
}
