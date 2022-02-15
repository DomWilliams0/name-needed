use std::iter::once;

use common::*;
use unit::world::WorldPoint;
use world::{ExplorationFilter, NavigationError, SearchGoal};

use crate::ecs::*;
use crate::event::{EntityEvent, EntityEventPayload, EntityEventQueue};
use crate::path::follow::{PathFollowing, PathRequest};
use crate::steer::{SteeringBehaviour, SteeringComponent};
use crate::{TransformComponent, WorldRef};

/// Holds the current path to follow
#[derive(Component, EcsComponent)]
#[storage(VecStorage)]
#[name("path")]
#[clone(disallow)]
pub struct FollowPathComponent {
    path: Option<PathFollowing>,
    follow_speed: NormalizedFloat,
    current_token: Option<PathToken>,

    /// If set, will be popped in next tick and `path` updated
    request: Option<PathRequest>,
    next_token: u64,
}

/// Entity-specific opaque unique token to differentiate path requests
#[derive(Eq, PartialEq, Copy, Clone)]
pub struct PathToken(u64);

/// System to assign steering behaviour from current path, if any
pub struct PathSteeringSystem;

impl<'a> System<'a> for PathSteeringSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, WorldRef>,
        Write<'a, EntityEventQueue>,
        WriteStorage<'a, TransformComponent>,
        WriteStorage<'a, FollowPathComponent>,
        WriteStorage<'a, SteeringComponent>,
    );

    fn run(
        &mut self,
        (entities, world, mut event_queue, mut transform, mut path, mut steer): Self::SystemData,
    ) {
        for (e, transform, mut path, steer) in
            (&entities, &mut transform, &mut path, &mut steer).join()
        {
            let e = Entity::from(e);
            log_scope!(o!("system" => "path steering", e));

            // new path request
            if let Some(req) = path.pop_request() {
                trace!("new path request"; "request" => ?req);

                // send failed arrived event for previous target
                if let Some(current) = path.current_token {
                    trace!("aborting previous path"; "token" => ?current, "target" => ?path.target());

                    event_queue.post(EntityEvent {
                        subject: e,
                        payload: EntityEventPayload::Arrived(
                            current,
                            Err(NavigationError::Aborted),
                        ),
                    });
                }

                // clobber current path
                path.path = None;
                path.current_token = None;

                let new_path = match req {
                    PathRequest::ClearCurrent => {
                        debug!("clearing current path by request");
                        None
                    }
                    PathRequest::NavigateTo {
                        target,
                        goal,
                        speed,
                        token,
                    } => {
                        let world = world.borrow();
                        Some((
                            world.find_path_with_goal(
                                transform.accessible_position(),
                                target.floor(),
                                goal,
                            ),
                            speed,
                            token,
                            goal,
                            Some(target),
                        ))
                    }
                    PathRequest::Explore {
                        speed,
                        token,
                        fuel: distance,
                        filter,
                    } => {
                        let world = world.borrow();
                        Some((
                            world.find_exploratory_path(
                                transform.accessible_position(),
                                distance,
                                filter,
                            ),
                            speed,
                            token,
                            SearchGoal::Arrive,
                            None,
                        ))
                    }
                };

                match new_path {
                    Some((Err(err), _, token, _, target)) => {
                        warn!("failed to find path"; "target" => ?target, "error" => %err);

                        event_queue.post(EntityEvent {
                            subject: e,
                            payload: EntityEventPayload::Arrived(token, Err(err)),
                        });
                    }
                    Some((Ok(new_path), speed, token, goal, _)) => {
                        let path_len = new_path.path().len();
                        let target = new_path.target().centred(); // TODO return random target point for unspecified too
                        let new_following = PathFollowing::new(new_path, target, goal);
                        debug!("following new path"; "target" => ?new_following.target(), "path_nodes" => path_len);

                        path.path = Some(new_following);
                        path.follow_speed = speed;
                        path.current_token = Some(token);
                    }
                    None => {}
                };
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
                        trace!("arrived at path target"; "target" => %target);

                        let token = path.current_token.take().expect("should have token");
                        event_queue.post(EntityEvent {
                            subject: e,
                            payload: EntityEventPayload::Arrived(token, Ok(target)),
                        });

                        path.path = None;
                    }
                    Some((next_block, cost)) => {
                        trace!("next waypoint"; "waypoint" => ?next_block, "cost" => ?cost);
                        steer.behaviour = SteeringBehaviour::seek(next_block, path.follow_speed);
                    }
                }
            }
        }
    }
}

impl FollowPathComponent {
    fn set_request(&mut self, req: PathRequest) {
        if let Some(prev @ PathRequest::NavigateTo { .. }) = self.request.as_ref() {
            warn!("follow path target was overwritten before it could be used";
                "previous" => ?prev, "new" => ?req
            );
        }

        trace!("assigning new follow path request"; "request" => ?req);
        self.request = Some(req);
    }

    pub fn request_navigation(&mut self, target: WorldPoint, speed: NormalizedFloat) -> PathToken {
        self.request_navigation_with_goal(target, SearchGoal::Arrive, speed)
    }

    pub fn request_navigation_with_goal(
        &mut self,
        target: WorldPoint,
        goal: SearchGoal,
        speed: NormalizedFloat,
    ) -> PathToken {
        let token = self.alloc_token();
        self.set_request(PathRequest::NavigateTo {
            target,
            goal,
            speed,
            token,
        });
        token
    }

    pub fn request_explore(
        &mut self,
        fuel: u32,
        speed: NormalizedFloat,
        filter: Option<ExplorationFilter>,
    ) -> PathToken {
        let token = self.alloc_token();
        self.set_request(PathRequest::Explore {
            fuel,
            speed,
            token,
            filter,
        });
        token
    }

    fn alloc_token(&mut self) -> PathToken {
        let token = PathToken(self.next_token);
        self.next_token = self.next_token.wrapping_add(1);
        token
    }

    pub fn clear_path(&mut self) {
        self.set_request(PathRequest::ClearCurrent);
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

    fn pop_request(&mut self) -> Option<PathRequest> {
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
