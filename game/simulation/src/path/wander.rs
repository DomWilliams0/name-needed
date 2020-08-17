use common::*;
use world::{InnerWorldRef, WorldRef};

use crate::ecs::*;
use crate::path::{FollowPathComponent, WANDER_SPEED};
use crate::TransformComponent;

#[derive(Component, Default)]
#[storage(NullStorage)]
pub struct WanderComponent;

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

        for (e, _, transform, path_follow) in
            (&entities, &wander, &transform, &mut path_follow).join()
        {
            log_scope!(o!("system" => "wander", E(e)));

            // only assign paths if not following one already
            if path_follow.target().is_none() {
                let target = world.choose_random_accessible_block(transform.position.floor(), 20);
                if let Some(pos) = target {
                    let token =
                        path_follow.new_path_to(pos.centred(), NormalizedFloat::new(WANDER_SPEED));
                    my_trace!("new wander target"; "target" => %pos, "token" => ?token);
                } else {
                    my_warn!(
                        "failed to find wander destination, we are probably stuck";
                        "position" => %transform.position
                    );
                }
            }
        }
    }
}