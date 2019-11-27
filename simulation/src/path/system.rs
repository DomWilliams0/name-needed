use log::*;
use rand::prelude::*;

use world::{BlockPosition, WorldPathSlice, CHUNK_SIZE};

use crate::ecs::*;
use crate::path::follow::PathFollowing;
use crate::steer::Steering;
use crate::Transform;

/// Holds the current path to follow
#[derive(Default)]
pub struct FollowPath {
    path: Option<PathFollowing>,
}

impl Component for FollowPath {}

impl FollowPath {
    /// As much of the path that has been calculated so far
    pub fn path(&self) -> Option<WorldPathSlice> {
        self.path.as_ref().map(|p| p.path_remaining())
    }
}

/// System to assign steering behaviour from current path, if any
pub struct PathSteeringSystem;

impl System for PathSteeringSystem {
    fn tick_system(&mut self, data: &TickData) {
        data.ecs_world
            .matcher_with_entities::<All<(Read<Transform>, Write<FollowPath>, Write<Steering>)>>()
            .for_each(|(e, (transform, path, steer))| {
                let following = match path.path {
                    Some(ref mut path) => path,
                    None => return,
                };

                *steer = match following.next_waypoint(&transform.position) {
                    // waypoint
                    Some((waypoint, false)) => {
                        if following.changed() {
                            debug!("{}: heading towards {:?}", NiceEntity(e), waypoint);
                        }
                        Steering::seek(waypoint.into())
                    }

                    // last waypoint
                    Some((waypoint, true)) => {
                        if following.changed() {
                            debug!(
                                "{}: heading towards final waypoint {:?}",
                                NiceEntity(e),
                                waypoint
                            );
                        }
                        Steering::arrive(waypoint.into())
                    }

                    // path over
                    None => {
                        debug!("{}: arrived at destination", NiceEntity(e));
                        path.path = None;
                        Steering::default()
                    }
                }
            });
    }
}

/// Temporary (!!) system to assign a path. Will be replaced by a proper system (mark my words).
/// Look, it's even already deprecated
#[deprecated(note = "Make sure this is replaced, thx")]
pub struct TempPathAssignmentSystem;

impl System for TempPathAssignmentSystem {
    fn tick_system(&mut self, data: &TickData) {
        let mut rand = rand::thread_rng();
        let world = data.voxel_world.borrow();

        data.ecs_world
            .matcher_with_entities::<All<(Read<Transform>, Write<FollowPath>)>>()
            .for_each(|(e, (transform, path))| {
                if path.path.is_none() {
                    // uh oh, new path needed

                    // get random destination with limit on attempts
                    const ATTEMPTS: i32 = 10;
                    let mut attempts_left = ATTEMPTS;
                    let target = loop {
                        let (x, y) = {
                            let random_chunk = world
                                .all_chunks()
                                .choose(&mut rand)
                                .expect("world should have >0 chunks");
                            let x = rand.gen_range(0, CHUNK_SIZE.as_u16());
                            let y = rand.gen_range(0, CHUNK_SIZE.as_u16());
                            let block = BlockPosition::from((x, y, 0));
                            let world_pos = block.to_world_pos(random_chunk.pos());
                            (world_pos.0, world_pos.1)
                        };

                        // find accessible place in world
                        if let target @ Some(_) = world.find_accessible_block_in_column(x, y) {
                            break target;
                        }

                        attempts_left -= 1;
                        if attempts_left < 0 {
                            warn!(
                                "{}: tried and failed {} times to find a random place to path find to",
                                NiceEntity(e),
                                ATTEMPTS,
                            );
                            break None;
                        }
                    };

                    // calculate path and set as target
                    let position = transform.position;
                    let full_path = target.and_then(|target| world.find_path(position, target));

                    match full_path {
                        Some(_) => info!(
                            "{}: found path from {:?} to {:?}",
                            NiceEntity(e),
                            position,
                            target
                        ),
                        None => debug!(
                            "{}: failed to find a path from {:?} to {:?}",
                            NiceEntity(e),
                            position,
                            target
                        ),
                    }

                    path.path = full_path.map(PathFollowing::new);
                }
            });
    }
}
