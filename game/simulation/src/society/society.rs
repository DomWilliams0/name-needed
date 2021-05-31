use crate::ecs::*;
use crate::item::ContainerComponent;
use crate::job::SocietyJobList;
use crate::{ComponentWorld, SocietyHandle};
use common::*;
use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashSet;

pub struct Society {
    name: String,
    handle: SocietyHandle,
    jobs: RefCell<SocietyJobList>,

    /// Communal containers
    containers: HashSet<Entity>,
}

impl Society {
    pub(crate) fn with_name(handle: SocietyHandle, name: String) -> Self {
        Self {
            name,
            handle,
            jobs: RefCell::new(Default::default()),
            containers: HashSet::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn handle(&self) -> SocietyHandle {
        self.handle
    }

    pub fn jobs(&self) -> Ref<SocietyJobList> {
        self.jobs.borrow()
    }

    pub fn jobs_mut(&self) -> RefMut<SocietyJobList> {
        self.jobs.borrow_mut()
    }

    /// The given container must already be set to communal, returns true if successful
    pub fn add_communal_container(
        &mut self,
        container: Entity,
        world: &impl ComponentWorld,
    ) -> bool {
        if !self.is_communal_to_this(container, world) {
            warn!("communal container is not communal"; "container" => E(container), "society" => ?self);
            false
        } else {
            self.containers.insert(container);
            true
        }
    }

    /// The given container must still be set to communal
    pub fn remove_communal_container(
        &mut self,
        container: Entity,
        world: &impl ComponentWorld,
    ) -> bool {
        if !self.is_communal_to_this(container, world) {
            warn!("communal container is not communal"; "container" => E(container), "society" => ?self);
            false
        } else {
            let contained = self.containers.remove(&container);
            debug_assert!(
                contained,
                "society did not contain removed communal container {}",
                E(container)
            );
            true
        }
    }

    fn is_communal_to_this(&self, container: Entity, world: &impl ComponentWorld) -> bool {
        let communal = world
            .component::<ContainerComponent>(container)
            .ok()
            .and_then(|comp| comp.communal());

        communal == Some(self.handle)
    }
}

impl Debug for Society {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("Society")
            .field("name", &self.name)
            .field("handle", &self.handle)
            .field("jobs", &*self.jobs.borrow())
            .field("containers", &self.containers.len())
            .finish()
    }
}

slog_value_debug!(Society);
