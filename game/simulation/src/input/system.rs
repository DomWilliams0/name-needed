use crate::ecs::*;
use crate::input::{InputEvent, WorldColumn};
use crate::{RenderComponent, TransformComponent};
use common::*;
use world::WorldRef;

pub struct InputSystem<'a> {
    pub events: &'a [InputEvent],
}

/// Marker for entity selection by the player
#[derive(Component, Default)]
#[storage(NullStorage)]
pub struct SelectedComponent;

/// Resource for selected entity - not guaranteed to be alive
/// `get()` will clear it if the entity is dead
#[derive(Default)]
pub struct SelectedEntity(Option<Entity>);

impl<'a> System<'a> for InputSystem<'a> {
    type SystemData = (
        Read<'a, WorldRef>,
        Read<'a, EntitiesRes>,
        Write<'a, SelectedEntity>,
        WriteStorage<'a, SelectedComponent>,
        ReadStorage<'a, TransformComponent>,
        ReadStorage<'a, RenderComponent>,
    );

    fn run(
        &mut self,
        (world, entities, mut selected, mut selecteds, transform, render): Self::SystemData,
    ) {
        let resolve_selected = |select_pos: &WorldColumn| {
            let world = (*world).borrow();
            select_pos.find_highest_walkable(&world).and_then(|point| {
                // TODO spatial query rather than checking every entity ever

                const SELECT_THRESHOLD: f32 = 1.0;
                let point = Point3::from(point);
                (&entities, &transform, &render)
                    .join()
                    .find(|(_, transform, _)| {
                        point.distance2(transform.position.into()) < SELECT_THRESHOLD.powi(2)
                    }) // just choose the first in range
                    .map(|(e, _, _)| e)
            })
        };

        for e in self.events {
            match e {
                // selecting a new entity
                InputEvent::LeftClick(pos) => {
                    // unselect current
                    unselect_current(&mut selected, &mut selecteds);

                    // find newly selected
                    if let Some(to_select) = resolve_selected(pos) {
                        debug!("selected entity {:?}", to_select);
                        let _ = selecteds.insert(to_select, SelectedComponent);
                        selected.0 = Some(to_select);
                    }
                }
                InputEvent::RightClick(_pos) => {
                    // TODO make selected entity go to pos
                }
            }
        }
    }
}

fn unselect_current(res: &mut Write<SelectedEntity>, comp: &mut WriteStorage<SelectedComponent>) {
    if let Some(old) = res.0.take() {
        debug!("unselected entity {:?}", old);
        comp.remove(old);
    }
}

impl SelectedEntity {
    pub fn get<W: ComponentWorld>(&mut self, world: &W) -> Option<Entity> {
        match self.0 {
            None => None,
            Some(e) if world.component::<TransformComponent>(e).is_err() => {
                // entity is dead or no longer has transform
                self.0 = None;
                None
            }
            nice => nice, // still alive
        }
    }
}
