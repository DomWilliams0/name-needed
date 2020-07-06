use crate::ecs::Entity;
use crate::society::job::Task;
use std::collections::HashMap;

#[derive(Default)]
pub struct TaskReservations {
    entity_to_task: HashMap<Entity, Task>,
    task_to_entity: HashMap<Task, Entity>,
}

impl TaskReservations {
    pub fn reserve(&mut self, entity: Entity, task: Task) -> (Option<Task>, Option<Entity>) {
        let old_task = self.entity_to_task.insert(entity, task.clone());
        let old_entity = self.task_to_entity.insert(task, entity);

        (old_task, old_entity)
    }

    pub fn is_reserved(&self, task: &Task) -> bool {
        self.task_to_entity.contains_key(task)
    }

    /// Not reserved or reserved by the given entity
    pub fn is_available_to(&self, task: &Task, entity: Entity) -> bool {
        match self.task_to_entity.get(task) {
            Some(e) => *e == entity,
            None => true, // unreserved
        }
    }
}
