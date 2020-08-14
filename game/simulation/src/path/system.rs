use std::iter::once;

use common::*;
use unit::world::{GlobalSliceIndex, WorldPoint};
use world::InnerWorldRef;
use world::{NavigationError, SearchGoal};

use crate::ecs::*;
use crate::event::{EntityEvent, EntityEventPayload, EntityEventQueue};
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
    new_target: Option<(WorldPoint, SearchGoal)>,
    follow_speed: NormalizedFloat,
    prev_z: Option<GlobalSliceIndex>,
}

#[derive(Component, Default)]
#[storage(NullStorage)]
pub struct WanderComponent;

/// System to assign steering behaviour from current path, if any
pub struct PathSteeringSystem;

/// Event component to indicate arrival at the given target position
/// TODO should be an enum and represent interruption too, i.e. path was invalidated
#[derive(Component, Default)]
#[storage(HashMapStorage)]
pub struct ArrivedAtTargetEventComponent(pub WorldPoint);

impl<'a> System<'a> for PathSteeringSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, WorldRef>,
        Read<'a, LazyUpdate>,
        Write<'a, EntityEventQueue>,
        WriteStorage<'a, TransformComponent>,
        WriteStorage<'a, FollowPathComponent>,
        WriteStorage<'a, SteeringComponent>,
    );

    fn run(
        &mut self,
        (entities, world, lazy_update, mut event_queue, mut transform, mut path, mut steer): Self::SystemData,
    ) {
        for (e, transform, mut path, steer) in
            (&entities, &mut transform, &mut path, &mut steer).join()
        {
            // new path request
            if let Some((target, goal)) = path.new_target.take() {
                // skip path finding if destination is the same
                if Some(target) != path.path.as_ref().map(|path| path.target()) {
                    let world = (*world).borrow();
                    let new_path = match world.find_path_with_goal(
                        transform.position.floor(),
                        target.floor(),
                        goal,
                    ) {
                        Err(e) => {
                            warn!("failed to find path to target {:?}: {}", target, e);
                            continue;
                        }
                        Ok(path) => path,
                    };

                    let new_following = PathFollowing::new(new_path, target, goal);
                    debug!(
                        "{:?}: following new path to {:?}",
                        e,
                        new_following.target()
                    );
                    path.path.replace(new_following);
                }
            }

            let following = match path.path.as_mut() {
                Some(p) => p,
                None => continue,
            };

            if steer.behaviour.is_nop() {
                // move onto next waypoint
                match following.next_waypoint() {
                    None => {
                        let target = path.target().unwrap();
                        trace!("{:?}: path finished, arrived at {}", e, target,);

                        // indicate arrival to other systems
                        // TODO remove this
                        lazy_update.insert(e, ArrivedAtTargetEventComponent(target));

                        event_queue.post(EntityEvent {
                            subject: e,
                            payload: EntityEventPayload::Arrived(target),
                        });

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
                    let target = target.centred();
                    match world.find_path_with_goal(
                        transform.position.floor(),
                        target.floor(),
                        SearchGoal::Arrive,
                    ) {
                        Err(NavigationError::SourceNotWalkable(_)) => {
                            warn!("{:?}: stuck in a non walkable position", e);
                            None
                        }
                        Err(err) => {
                            trace!(
                                "{:?}: failed to find wander path to random position: {}",
                                e,
                                err
                            );
                            None
                        }
                        Ok(path) => {
                            debug!("{:?} new wander path to {:?}", e, path.target());
                            Some(PathFollowing::new(path, target, SearchGoal::Arrive))
                        }
                    }
                });

                // wander slowly
                path_follow.follow_speed = NormalizedFloat::new(WANDER_SPEED);
            }
        }
    }
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
    // TODO dont manually set the exact follow speed - choose a preset e.g. wander,dawdle,walk,fastwalk,run,sprint
    // TODO return a monotonic token representing this assignment, so the caller can later identify if the target is still its doing
    pub fn new_path(&mut self, target: WorldPoint, goal: SearchGoal, speed: NormalizedFloat) {
        if let Some((old, _)) = self.new_target.replace((target, goal)) {
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
                    .map(|&pos| pos.centred())
                    .chain(once(path.target())),
            );
        }
    }
}
