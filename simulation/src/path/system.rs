use rand::prelude::*;
use specs::prelude::*;
use specs_derive::Component;

use world::navigation::Path as NavPath;
use world::{ChunkPosition, CHUNK_SIZE};

use crate::simulation::WorldResource;
use crate::steer::Steering;

/// Holds the current path to follow
#[derive(Component, Default)]
#[storage(VecStorage)]
pub struct FollowPath {
    pub path: Option<NavPath>,
}

/// System to assign steering behaviour from current path, if any
pub struct PathSteeringSystem;

impl<'a> System<'a> for PathSteeringSystem {
    type SystemData = (ReadStorage<'a, FollowPath>, WriteStorage<'a, Steering>);

    fn run(&mut self, (path, mut steer): Self::SystemData) {
        for (path, steer) in (&path, &mut steer).join() {
            // TODO
        }
    }
}

/// Temporary (!!) system to assign a path. Will be replaced by a proper system (mark my words).
/// Look, it's even already deprecated
#[deprecated(note = "Make sure this is replaced, thx")]
pub struct TempPathAssignmentSystem;

impl<'a> System<'a> for TempPathAssignmentSystem {
    type SystemData = (Read<'a, WorldResource>, WriteStorage<'a, FollowPath>);

    fn run(&mut self, (world, mut path): Self::SystemData) {
        let mut rand = rand::thread_rng();
        let world = world.0.get();
        for path in &mut path.join() {
            if path.path.is_none() {
                // uh oh, new path needed

                // get random destination with limit on attempts
                let mut attempts_left = 10;
                let target = loop {
                    let x = rand.gen_range(0, CHUNK_SIZE as i32);
                    let y = rand.gen_range(0, CHUNK_SIZE as i32);

                    // find accessible place in world
                    if let target @ Some(_) = world.find_accessible_block_in_column(x, y) {
                        break target;
                    }

                    attempts_left -= 1;
                    if attempts_left < 0 {
                        break None;
                    }
                };

                // TODO calculate path and set as target
/*
                let path = target.and_then(|target| {
                    // TODO calculate target cross chunks?!
                    unimplemented!();
                });
*/
            }
        }
    }
}
