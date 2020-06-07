use std::iter::once;
use std::mem::MaybeUninit;

use common::*;
use unit::world::{SliceIndex, WorldPoint};
use world::NavigationError;
use world::{InnerWorldRef, WorldPath};

use crate::ecs::*;
use crate::path::follow::PathFollowing;
use crate::path::WANDER_SPEED;
use crate::steer::{SteeringBehaviour, SteeringComponent};
use crate::{TransformComponent, WorldRef};

/// Holds the current path to follow
#[derive(Component)]
#[storage(VecStorage)]
pub struct FollowPathComponent {
    path: Option<PathFollowing>,
    /// If set, will be popped in next tick and `path` updated
    new_target: Option<WorldPoint>,
    follow_speed: NormalizedFloat,
    prev_z: Option<SliceIndex>,
}

#[derive(Component, Default)]
#[storage(NullStorage)]
pub struct WanderComponent;

/// System to assign steering behaviour from current path, if any
pub struct PathSteeringSystem;

impl<'a> System<'a> for PathSteeringSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, WorldRef>,
        WriteStorage<'a, TransformComponent>,
        WriteStorage<'a, FollowPathComponent>,
        WriteStorage<'a, SteeringComponent>,
    );

    fn run(&mut self, (entities, world, mut transform, mut path, mut steer): Self::SystemData) {
        for (e, transform, mut path, steer) in
            (&entities, &mut transform, &mut path, &mut steer).join()
        {
            // new path request
            if let Some(target) = path.new_target.take() {
                // skip path finding if destination is the same
                if Some(target) != path.path.as_ref().map(|path| path.target()) {
                    let world = (*world).borrow();
                    let new_path = match path_find(&world, transform.position, target) {
                        Err(e) => {
                            warn!("failed to find path to target {:?}: {:?}", target, e); // TODO {} for error
                            continue;
                        }
                        Ok(path) => path,
                    };

                    debug!("{:?}: following new path to {:?}", e, transform.position);
                    path.path.replace(PathFollowing::new(new_path, target));
                }
            }

            let following = match path.path.as_mut() {
                Some(p) => p,
                None => continue,
            };

            if steer.behaviour.is_nop() {
                // assume entity is now at the same z level as the last waypoint
                // FIXME GROSS HACK
                if let Some(last) = path.prev_z.take() {
                    transform.set_height(last.0);
                }

                // move onto next waypoint
                match following.next_waypoint() {
                    None => {
                        trace!("{:?}: path finished", e);
                        path.path = None;
                    }
                    Some((next_block, _cost)) => {
                        trace!("{:?}: next waypoint: {:?}", e, next_block);
                        steer.behaviour = SteeringBehaviour::seek(next_block, path.follow_speed);
                        path.prev_z = Some(next_block.slice());
                    }
                }
            }
        }
    }
}

pub struct WanderPathAssignmentSystem;

impl<'a> System<'a> for WanderPathAssignmentSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, WorldRef>,
        ReadStorage<'a, WanderComponent>,
        ReadStorage<'a, TransformComponent>,
        WriteStorage<'a, FollowPathComponent>,
    );

    fn run(&mut self, (entities, world, wander, transform, mut path_follow): Self::SystemData) {
        let world: InnerWorldRef = (*world).borrow();

        for (e, _, transform, mut path_follow) in
            (&entities, &wander, &transform, &mut path_follow).join()
        {
            // only assign paths if not following one already
            if path_follow.path.is_none() {
                // may take a few iterations to find a valid wander target, so do the path finding
                // manually here rather than setting path.new_target and maybe waiting a few ticks
                path_follow.path = world.choose_random_walkable_block(10).and_then(|target| {
                    let target = WorldPoint::from(target);
                    match path_find(&world, transform.position, target) {
                        Err(NavigationError::SourceNotWalkable(_)) => {
                            warn!("{:?}: stuck in a non walkable position", e);
                            None
                        }
                        Err(err) => {
                            trace!(
                                "{:?}: failed to find wander path to random position: {:?}",
                                e,
                                err
                            );
                            None
                        }
                        Ok(path) => {
                            debug!("{:?} new wander path to {:?}", e, path.target());
                            Some(PathFollowing::new(path, target))
                        }
                    }
                });

                // wander slowly
                path_follow.follow_speed = NormalizedFloat::new(WANDER_SPEED);
            }
        }
    }
}

fn path_find(
    world: &InnerWorldRef,
    src: WorldPoint,
    tgt: WorldPoint,
) -> Result<WorldPath, NavigationError> {
    // try floor'd pos first, then ceil'd if it fails
    let srcs = src.floor_then_ceil();

    let mut last_err = MaybeUninit::uninit();
    let mut results = srcs
        .map(|src| world.find_path(src, tgt.floor()))
        .skip_while(|res| {
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

impl Default for FollowPathComponent {
    fn default() -> Self {
        Self {
            path: None,
            new_target: None,
            prev_z: None,
            follow_speed: NormalizedFloat::one(),
        }
    }
}

impl FollowPathComponent {
    pub fn new_path(&mut self, target: WorldPoint, speed: NormalizedFloat) {
        if let Some(old) = self.new_target.replace(target) {
            warn!("follow path target was overwritten before it could be used (prev: {:?}, overwritten with: {:?})", old, target);
        }

        self.follow_speed = speed;
        self.prev_z = None;
    }

    pub fn target(&self) -> Option<WorldPoint> {
        self.path.as_ref().map(|path| path.target())
    }

    pub fn waypoints(&self, out: &mut Vec<WorldPoint>) {
        if let Some(path) = self.path.as_ref() {
            out.extend(
                path.waypoints()
                    .map(|&pos| pos.into())
                    .chain(once(path.target())),
            );
        }
    }
    /*
    pub fn new_path_to_pos(&mut self, target: WorldPosition, speed: NormalizedFloat) {
        let mut vec = SmallVec::new();
        vec.push(target);
        self.update(vec, speed);
    }

    pub fn new_path_to_point(&mut self, target: WorldPoint, speed: NormalizedFloat) {
        let mut vec = SmallVec::new();
        vec.extend(target.floor_then_ceil());
        self.update(vec, speed);
    }

    fn update(&mut self, target: SmallVec<[WorldPosition; 2]>, speed: NormalizedFloat) {
        if let Some(old) = self.new_target.replace(target) {
            warn!("follow path target was overwritten before it could be used (prev: {:?}, overwritten with: {:?})", old, target);
        }

        self.follow_speed = speed;
    }
    */
}
