use std::iter::once;

use common::*;
use unit::world::WorldPoint;
use world::SearchGoal;

use crate::ecs::*;
use crate::event::{EntityEvent, EntityEventPayload, EntityEventQueue};
use crate::path::follow::{PathFollowing, PathRequest};
use crate::steer::{SteeringBehaviour, SteeringComponent};
use crate::{TransformComponent, WorldRef};

/// Holds the current path to follow
#[derive(Component)]
#[storage(VecStorage)]
pub struct FollowPathComponent {
    path: Option<PathFollowing>,
    follow_speed: NormalizedFloat,
    current_token: Option<PathToken>,

    /// If set, will be popped in next tick and `path` updated
    request: Option<(PathRequest, PathToken)>,
    next_token: u64,
}

/// Entity-specific opaque unique token to differentiate path requests
#[derive(Eq, PartialEq, Copy, Clone)]
pub struct PathToken(u64);

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
            if let Some((req, token)) = path.pop_request() {
                match req {
                    PathRequest::ClearCurrent => {
                        if path.target().is_some() {
                            debug!("clearing current path by request");
                        }
                        path.path = None;
                    }
                    PathRequest::NewTarget {
                        target,
                        goal,
                        speed,
                    } => {
                        // skip path finding if destination is the same
                        let current_target = path.target();
                        if current_target != Some(target) {
                            let world = (*world).borrow();
                            let new_path = match world.find_path_with_goal(
                                transform.position.floor(),
                                target.floor(),
                                goal,
                            ) {
                                Err(err) => {
                                    warn!("failed to find path to target {:?}: {}", target, err);

                                    event_queue.post(EntityEvent {
                                        subject: e,
                                        payload: EntityEventPayload::Arrived(token, Err(err)),
                                    });

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
                            path.follow_speed = speed;
                            path.current_token = Some(token);
                        }
                    }
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
                        trace!("{:?}: path finished, arrived at {}", e, target);

                        // indicate arrival to other systems
                        // TODO remove this
                        lazy_update.insert(e, ArrivedAtTargetEventComponent(target));

                        let token = path.current_token.take().expect("should have token");
                        event_queue.post(EntityEvent {
                            subject: e,
                            payload: EntityEventPayload::Arrived(token, Ok(target)),
                        });

                        path.path = None;
                    }
                    Some((next_block, _cost)) => {
                        trace!("{:?}: next waypoint: {:?}", e, next_block);
                        steer.behaviour = SteeringBehaviour::seek(next_block, path.follow_speed);
                    }
                }
            }
        }
    }
}

impl FollowPathComponent {
    // TODO return a monotonic token representing this assignment, so the caller can later identify if the target is still its doing
    fn request_new_path(&mut self, req: PathRequest) -> PathToken {
        if let Some(old) = self.request.as_ref() {
            warn!("follow path target was overwritten before it could be used (prev: {:?}, overwritten with: {:?})", old, req);
        }

        let token = PathToken(self.next_token);
        self.next_token = self.next_token.wrapping_add(1);

        self.request = Some((req, token));
        // preserve current_token until done
        token
    }

    pub fn new_path_to(&mut self, target: WorldPoint, speed: NormalizedFloat) -> PathToken {
        self.new_path_with_goal(target, SearchGoal::Arrive, speed)
    }

    pub fn new_path_with_goal(
        &mut self,
        target: WorldPoint,
        goal: SearchGoal,
        speed: NormalizedFloat,
    ) -> PathToken {
        self.request_new_path(PathRequest::NewTarget {
            target,
            goal,
            speed,
        })
    }

    pub fn clear_path(&mut self) {
        // ignore token
        let _ = self.request_new_path(PathRequest::ClearCurrent);
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

    pub fn pop_request(&mut self) -> Option<(PathRequest, PathToken)> {
        self.request.take()
    }

    pub fn current_token(&self) -> Option<PathToken> {
        self.current_token
    }
}

impl Default for FollowPathComponent {
    fn default() -> Self {
        Self {
            path: None,
            request: None,
            follow_speed: NormalizedFloat::one(),
            next_token: 0x1000,
            current_token: None,
        }
    }
}

impl Debug for PathToken {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "PathToken({:#x})", self.0)
    }
}
