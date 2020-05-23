use std::ops::DerefMut;

use common::*;
use unit::dim::CHUNK_SIZE;
use unit::world::{SliceBlock, WorldPoint, WorldPosition};
use world::NavigationError;
use world::{InnerWorldRef, WorldPath};

use crate::ecs::*;
use crate::path::follow::PathFollowing;
use crate::steer::{SteeringBehaviour, SteeringComponent};
use crate::{TransformComponent, WorldRef};
use std::iter::{once, once_with};
use std::mem::MaybeUninit;

/// Holds the current path to follow
#[derive(Default)]
pub struct FollowPathComponent {
    path: Option<PathFollowing>,
}

impl Component for FollowPathComponent {
    type Storage = VecStorage<Self>;
}

/// System to assign steering behaviour from current path, if any
pub struct PathSteeringSystem;

impl<'a> System<'a> for PathSteeringSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        WriteStorage<'a, TransformComponent>,
        WriteStorage<'a, FollowPathComponent>,
        WriteStorage<'a, SteeringComponent>,
    );

    fn run(&mut self, (entities, mut transform, mut path, mut steer): Self::SystemData) {
        for (e, transform, mut path, steer) in
            (&entities, &mut transform, &mut path, &mut steer).join()
        {
            let following = match path.path.as_mut() {
                Some(p) => p,
                None => continue,
            };

            if steer.behaviour.is_nop() {
                // assume entity is now at the same z level as the last waypoint
                // FIXME GROSS HACK
                if let Some(last) = following.last_waypoint() {
                    transform.set_height(last.2);
                }

                // move onto next waypoint
                match following.next_waypoint() {
                    None => {
                        trace!("{:?}: path finished", e);
                        path.path = None;
                    }
                    Some((next_block, _cost)) => {
                        trace!("{:?}: next waypoint: {:?}", e, next_block);
                        steer.behaviour = SteeringBehaviour::seek(next_block);
                    }
                }
            }
        }
    }
}

pub struct RandomPathAssignmentSystem;

impl<'a> System<'a> for RandomPathAssignmentSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, WorldRef>,
        ReadStorage<'a, TransformComponent>,
        WriteStorage<'a, FollowPathComponent>,
    );

    fn run(&mut self, (entities, world, transform, mut path_follow): Self::SystemData) {
        let world: InnerWorldRef = (*world).borrow();

        for (e, transform, mut path_follow) in (&entities, &transform, &mut path_follow).join() {
            // only assign paths if not following one already
            if path_follow.path.is_none() {
                path_follow.path =
                    choose_random_target(&world).and_then(|target| {
                        match path_find(&world, transform.position, target) {
                            Err(NavigationError::SourceNotWalkable(_)) => {
                                warn!("{:?}: stuck in a non walkable position", e);
                                None
                            }
                            Err(err) => {
                                trace!(
                                    "{:?}: failed to find path between random positions: {:?}",
                                    e,
                                    err
                                );
                                None
                            }
                            Ok(path) => {
                                debug!("{:?} new path to {:?}", e, path.target());
                                Some(PathFollowing::new(path))
                            }
                        }
                    });
            }
        }
    }
}

fn choose_random_target(world: &InnerWorldRef) -> Option<WorldPosition> {
    let mut rand = random::get();
    for _ in 0..10 {
        let chunk = world.all_chunks().choose(rand.deref_mut()).unwrap(); // chunks wont be empty

        let x = rand.gen_range(0, CHUNK_SIZE.as_block_coord());
        let y = rand.gen_range(0, CHUNK_SIZE.as_block_coord());
        if let Some(block_pos) = chunk.find_accessible_block(SliceBlock(x, y), None) {
            return Some(block_pos.to_world_position(chunk.pos()));
        }
    }
    None
}

fn path_find(
    world: &InnerWorldRef,
    src: WorldPoint,
    tgt: WorldPosition,
) -> Result<WorldPath, NavigationError> {
    // try floor'd pos first, then ceil'd if it fails
    let srcs = once(src.floor()).chain(once_with(|| src.ceil()));

    let mut last_err = MaybeUninit::uninit();
    let mut results = srcs.map(|src| world.find_path(src, tgt)).skip_while(|res| {
        if let Err(e) = res {
            last_err = MaybeUninit::new(e.clone());
        }
        res.is_err()
    });

    results.next().unwrap_or_else(|| {
        Err(
            // Safety: if results is empty, it's because path navigation failed so last_err is
            // definitely set
            unsafe { last_err.assume_init() },
        )
    })
}
