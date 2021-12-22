use common::*;
use unit::space::length::Length2;

use crate::ecs::*;
use crate::event::DeathReason;
use crate::job::SocietyJobHandle;
use crate::SocietyHandle;
use crate::{QueuedUpdates, Societies};

/// A UI element that exists in the game world
#[derive(Debug, Clone, Component, EcsComponent)]
#[storage(HashMapStorage)]
#[name("ui-element")]
#[clone(disallow)]
pub struct UiElementComponent {
    // TODO generalise when more ui elements are added
    pub build_job: SocietyJobHandle,

    /// For rendering
    pub size: Length2,
}

impl UiElementComponent {
    pub fn society(&self) -> Option<SocietyHandle> {
        Some(self.build_job.society())
    }
}

/// Destroys UI elements for expired jobs
pub struct UiElementPruneSystem;

impl<'a> System<'a> for UiElementPruneSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, Societies>,
        Read<'a, QueuedUpdates>,
        ReadStorage<'a, UiElementComponent>,
    );

    fn run(&mut self, (entities, societies, updates, ui): Self::SystemData) {
        let mut expired = Vec::new();

        for (e, ui) in (&entities, &ui).join() {
            if ui.build_job.resolve(&societies).is_none() {
                // job is expired
                expired.push(Entity::from(e));
            }
        }

        if !expired.is_empty() {
            debug!("killing {} ui elements for expired jobs", expired.len(); "dying" => ?expired);
            updates.queue("kill expired job ui elements", move |world| {
                world.kill_entities(&expired, DeathReason::Unknown);
                Ok(())
            })
        }
    }
}
