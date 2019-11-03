use log::*;
use rand::prelude::*;
use specs::prelude::*;
use specs_derive::Component;

use world::{BlockPosition, WorldRef, CHUNK_SIZE};

use crate::path::follow::PathFollowing;
use crate::steer::{Steering, SteeringBehaviour};
use crate::Position;

/// Holds the current path to follow
#[derive(Component, Default)]
#[storage(VecStorage)]
pub struct FollowPath {
    path: Option<PathFollowing>,
}

/// System to assign steering behaviour from current path, if any
pub struct PathSteeringSystem;

impl<'a> System<'a> for PathSteeringSystem {
    type SystemData = (
        ReadStorage<'a, Position>,
        WriteStorage<'a, FollowPath>,
        WriteStorage<'a, Steering>,
    );

    fn run(&mut self, (pos, mut path, mut steer): Self::SystemData) {
        for (pos, path, steer) in (&pos, &mut path, &mut steer).join() {
            let following = match path.path {
                Some(ref mut path) => path,
                None => continue,
            };

            *steer = match following.next_waypoint(pos) {
                // waypoint
                Some((waypoint, false)) => {
                    if following.changed() {
                        debug!("heading towards {:?}", waypoint);
                    }
                    Steering::seek(waypoint.into())
                }

                // last waypoint
                Some((waypoint, true)) => {
                    if following.changed() {
                        debug!("heading towards final waypoint {:?}", waypoint);
                    }
                    Steering::arrive(waypoint.into())
                }

                // path over
                None => {
                    debug!("arrived at destination");
                    path.path = None;
                    Steering::default()
                }
            }
        }
    }
}

/// Temporary (!!) system to assign a path. Will be replaced by a proper system (mark my words).
/// Look, it's even already deprecated
#[deprecated(note = "Make sure this is replaced, thx")]
pub struct TempPathAssignmentSystem;

impl<'a> System<'a> for TempPathAssignmentSystem {
    type SystemData = (
        Read<'a, WorldRef>,
        ReadStorage<'a, Position>,
        WriteStorage<'a, FollowPath>,
    );

    fn run(&mut self, (world, pos, mut path): Self::SystemData) {
        let mut rand = rand::thread_rng();
        let world = world.borrow();
        for (pos, path) in (&pos, &mut path).join() {
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
                            "tried and failed {} times to find a random place to path find to",
                            ATTEMPTS,
                        );
                        break None;
                    }
                };

                // calculate path and set as target
                let full_path = target.and_then(|target| world.find_path(*pos, target));

                match full_path {
                    Some(_) => info!("found path from {:?} to {:?}", pos, target),
                    None => warn!("failed to find a path from {:?} to {:?}", pos, target),
                }

                path.path = full_path.map(PathFollowing::new);
            }
        }
    }
}
