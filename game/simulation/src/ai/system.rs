use std::collections::HashMap;

use ai::{AiBox, DecisionSource, Dse, Intelligence, IntelligentDecision};
use common::*;

use crate::activity::ActivityComponent;
use crate::ai::dse::{human_dses, AdditionalDse, ObeyDivineCommandDse};
use crate::ai::{AiAction, AiBlackboard, AiContext, SharedBlackboard};
use crate::ecs::*;
use crate::item::InventoryComponent;
use crate::needs::HungerComponent;

use crate::simulation::Tick;
use crate::society::job::{JobList, Task};
use crate::society::{Society, SocietyComponent};
use crate::TransformComponent;
use crate::{dse, Societies};
use specs::{Builder, EntityBuilder};
use std::iter::once;
use world::WorldRef;

#[derive(Component)]
#[storage(DenseVecStorage)]
pub struct AiComponent {
    intelligence: ai::Intelligence<AiContext>,
    // last_completed_action: Option<AiAction>,
    // current_action: Option<AiAction>,
}

impl AiComponent {
    fn with_species(species: &Species) -> Self {
        let intelligence = match species {
            Species::Human => Intelligence::new(human_dses()),
        };

        Self {
            intelligence,
            // last_completed_action: None,
            // current_action: None,
        }
    }

    pub fn add_divine_command(&mut self, command: AiAction) {
        let dse = dse!(ObeyDivineCommandDse(command));
        self.intelligence
            .add_smarts(AdditionalDse::DivineCommand, once(dse));
    }

    pub fn remove_divine_command(&mut self) {
        self.intelligence.pop_smarts(&AdditionalDse::DivineCommand);
    }

    pub fn clear_last_action(&mut self) {
        self.intelligence.clear_last_action();
    }

    // pub fn last_completed_action(&self) -> Option<&AiAction> {
    //     self.last_completed_action.as_ref()
    // }
}

pub struct AiSystem;

impl<'a> System<'a> for AiSystem {
    type SystemData = (
        Read<'a, Tick>,
        Read<'a, EntitiesRes>,
        Read<'a, EcsWorldFrameRef>,
        Read<'a, WorldRef>,
        Write<'a, Societies>,
        ReadStorage<'a, TransformComponent>,
        ReadStorage<'a, HungerComponent>,
        ReadStorage<'a, InventoryComponent>,
        WriteStorage<'a, AiComponent>,
        WriteStorage<'a, ActivityComponent>,
        WriteStorage<'a, SocietyComponent>,
    );

    fn run(
        &mut self,
        (
            tick,
            entities,
            ecs_world,
            voxel_world,
            mut societies,
            transform,
            hunger,
            inventory,
            mut ai,
            mut activity,
            society,
        ): Self::SystemData,
    ) {
        // TODO only run occasionally - FIXME TERRIBLE HACK
        if **tick % 10 != 0 {
            return;
        }

        let ecs_world: &EcsWorld = &*ecs_world;

        let mut shared_bb = SharedBlackboard {
            area_link_cache: HashMap::new(),
        };

        for (e, transform, hunger, ai, activity, society) in (
            &entities,
            &transform,
            &hunger,
            &mut ai,
            &mut activity,
            society.maybe(),
        )
            .join()
        {
            // initialize blackboard
            // TODO use arena/bump allocator and share instance between entities
            let mut bb = AiBlackboard {
                entity: e,
                position: transform.position,
                hunger: hunger.hunger(),
                inventory_search_cache: HashMap::new(),
                local_area_search_cache: HashMap::new(),
                inventory: inventory.get(e),
                world: ecs_world,
                shared: &mut shared_bb,
            };

            // Safety: can't use true lifetime on Blackboard so using 'static and transmuting until
            // we get our GATs
            let bb_ref: &mut AiBlackboard = unsafe { std::mem::transmute(&mut bb) };

            // collect extra actions from society job list, if any
            // TODO provide READ ONLY DSEs to ai intelligence
            // TODO use dynstack to avoid so many small temporary allocations?
            let mut extra_dses = Vec::<(Task, Box<dyn Dse<AiContext>>)>::new();
            let mut society: Option<&mut Society> =
                society.and_then(|society_comp| society_comp.resolve(&mut *societies));

            if let Some(ref mut society) = society {
                trace!("considering tasks for society {:?}", society);

                let jobs: &mut JobList = (*society).jobs_mut();
                let (is_cached, tasks) = jobs.collect_cached_tasks_for(*tick, &*voxel_world, e);

                debug_assert!(
                    extra_dses.is_empty(),
                    "society tasks is the only source of extra dses"
                );
                extra_dses.extend(tasks.map(|task| {
                    let dse = (&task).into();
                    (task, dse)
                }));

                trace!(
                    "there are {} tasks{} for {:?}",
                    extra_dses.len(),
                    if is_cached { " (cached)" } else { "" },
                    e
                );
            }

            // choose best action
            let streamed_dse = extra_dses.iter().map(|(_task, dse)| &**dse);
            if let IntelligentDecision::New { dse, action, src } = ai
                .intelligence
                .choose_with_stream_dses(bb_ref, streamed_dse)
            {
                debug!("{:?}: new activity: {} (from {:?})", e, dse.name(), src);
                trace!("activity: {:?}", action);

                // pass on to activity system
                activity.new_activity = Some(action);

                if let DecisionSource::Stream(i) = src {
                    // a society task was chosen, reserve this so others can't try to do it too
                    let society = society
                        .as_mut()
                        .expect("streamed DSEs expected to come from a society only");

                    let task = &extra_dses[i].0;
                    society.jobs_mut().reserve_task(e, task.clone());
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Species {
    Human,
}

#[derive(Debug)]
pub struct IntelligenceComponentTemplate {
    species: Species,
}

impl<V: Value> ComponentTemplate<V> for IntelligenceComponentTemplate {
    fn construct(values: &mut Map<V>) -> Result<Box<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized,
    {
        let species = values.get_string("species")?;
        let species = match species.as_str() {
            "human" => Species::Human,
            _ => {
                return Err(ComponentBuildError::TemplateSpecific(format!(
                    "unknown species {:?}",
                    species
                )))
            }
        };

        Ok(Box::new(Self { species }))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        let ai = AiComponent::with_species(&self.species);
        builder.with(ai).with(ActivityComponent::default())
    }
}

register_component_template!("intelligence", IntelligenceComponentTemplate);
