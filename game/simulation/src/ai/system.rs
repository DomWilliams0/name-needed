use std::collections::HashMap;
use std::iter::once;

use ai::{AiBox, DecisionSource, Dse, Intelligence, IntelligentDecision};
use common::*;

use crate::activity::ActivityComponent;
use crate::ai::dse::{dog_dses, human_dses, AdditionalDse, ObeyDivineCommandDse};
use crate::ai::{AiAction, AiBlackboard, AiContext, SharedBlackboard};
use crate::ecs::*;
use crate::item::InventoryComponent;
use crate::needs::HungerComponent;
use crate::simulation::Tick;
use crate::society::job::Task;
use crate::society::{Society, SocietyComponent};
use crate::{EntityLoggingComponent, TransformComponent};

use crate::{dse, Societies};

#[derive(Component, EcsComponent)]
#[storage(DenseVecStorage)]
#[name("ai")]
pub struct AiComponent {
    intelligence: ai::Intelligence<AiContext>,
    current: Option<DecisionSource<AiContext>>,
}

impl AiComponent {
    fn with_species(species: &Species) -> Self {
        let intelligence = match species {
            Species::Human => Intelligence::new(human_dses()),
            Species::Dog => Intelligence::new(dog_dses()),
        };

        Self {
            intelligence,
            current: None,
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

    pub fn interrupt_current_action<'a>(
        &mut self,
        this_entity: Entity,
        society_fn: impl FnOnce() -> &'a mut Society,
    ) {
        if let Some(interrupted_source) = self.current.take() {
            match interrupted_source {
                DecisionSource::Additional(AdditionalDse::DivineCommand, _) => {
                    // divine command interrupted, assume completed
                    debug!("removing interrupted divine command");
                    self.remove_divine_command();
                }
                DecisionSource::Stream(_) => {
                    // unreserve interrupted society task
                    let society = society_fn();
                    society.jobs_mut().unreserve_task(this_entity);
                }
                _ => {}
            }
        }
    }

    // pub fn last_completed_action(&self) -> Option<&AiAction> {
    //     self.last_completed_action.as_ref()
    // }
}

pub struct AiSystem;

impl<'a> System<'a> for AiSystem {
    // TODO optional components for ai: hunger, inventory
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, EcsWorldFrameRef>,
        Write<'a, Societies>,
        ReadStorage<'a, TransformComponent>,
        ReadStorage<'a, HungerComponent>,
        ReadStorage<'a, InventoryComponent>,
        WriteStorage<'a, AiComponent>,
        WriteStorage<'a, ActivityComponent>,
        WriteStorage<'a, SocietyComponent>,
        WriteStorage<'a, EntityLoggingComponent>,
    );

    fn run(
        &mut self,
        (
            entities,
            ecs_world,
            mut societies,
            transform,
            hunger,
            inventory,
            mut ai,
            mut activity,
            society,
            mut logging,
        ): Self::SystemData,
    ) {
        // TODO only run occasionally - FIXME TERRIBLE HACK
        let tick = Tick::fetch();
        if tick.value() % 10 != 0 {
            return;
        }

        let ecs_world: &EcsWorld = &*ecs_world;
        let societies: &mut Societies = &mut *societies;

        let mut shared_bb = SharedBlackboard {
            area_link_cache: HashMap::new(),
        };

        for (e, transform, hunger, ai, activity, society_opt) in (
            &entities,
            &transform,
            &hunger,
            &mut ai,
            &mut activity,
            society.maybe(),
        )
            .join()
        {
            log_scope!(o!("system" => "ai", E(e)));
            let society_opt: Option<&SocietyComponent> = society_opt; // for IDE

            // initialize blackboard
            // TODO use arena/bump allocator and share instance between entities
            let mut bb = AiBlackboard {
                entity: e,
                accessible_position: transform.accessible_position(),
                position: transform.position,
                hunger: hunger.hunger(),
                inventory_search_cache: HashMap::new(),
                local_area_search_cache: HashMap::new(),
                inventory: inventory.get(e),
                world: ecs_world,
                shared: &mut shared_bb,
            };

            // safety: can't use true lifetime on Blackboard so using 'static and transmuting until
            // we get our GATs
            let bb_ref: &'a mut AiBlackboard = unsafe { std::mem::transmute(&mut bb) };

            // collect extra actions from society job list, if any
            // TODO provide READ ONLY DSEs to ai intelligence
            // TODO use dynstack to avoid so many small temporary allocations, or arena allocator
            let mut extra_dses = Vec::<(Task, Box<dyn Dse<AiContext>>)>::new();
            let society = society_opt.and_then(|comp| comp.resolve(societies));

            if let Some(society) = society {
                trace!("considering tasks for society"; "society" => ?society);

                let mut jobs = society.jobs_mut();
                let (is_cached, tasks) = jobs.collect_cached_tasks_for(tick, ecs_world, society, e);

                // TODO fix (eventually) false assumption that all stream DSEs come from a society
                //  - this will help remove the multiple silly `society.as_mut().expect()` here
                debug_assert!(
                    extra_dses.is_empty(),
                    "society tasks expected to be the only source of extra dses"
                );
                extra_dses.extend(tasks.filter_map(|task| match task.as_dse(ecs_world) {
                    Some(dse) => Some((task, dse)),
                    None => {
                        warn!("task failed to conversion to DSE"; "task" => ?task);
                        None
                    }
                }));

                trace!(
                    "there are {count} tasks available",
                    count = extra_dses.len();
                    "cached" => is_cached,
                );
            }

            // choose best action
            let streamed_dse = extra_dses.iter().map(|(_task, dse)| &**dse);
            let decision = ai
                .intelligence
                .choose_with_stream_dses(bb_ref, streamed_dse);

            if let IntelligentDecision::New { dse, action, src } = decision {
                debug!("new activity"; "dse" => dse.name(), "source" => ?src);
                trace!("activity action"; "action" => ?action);

                // register interruption
                let mut society = society_opt.and_then(|comp| comp.resolve(societies));
                ai.interrupt_current_action(e, || {
                    society
                        .as_mut()
                        .expect("streamed DSEs expected to come from a society only")
                });

                if let DecisionSource::Stream(i) = src {
                    // a society task was chosen, reserve this so others can't try to do it too
                    let society =
                        society.expect("streamed DSEs expected to come from a society only");

                    let task = &extra_dses[i].0;
                    society.jobs_mut().reserve_task(e, task.clone());
                }

                // log decision
                if let Some(logs) = logging.get_mut(e) {
                    logs.log_event(&action);
                }

                ai.current = Some(src);

                // pass on to activity system
                activity.interrupt_with_new_activity(action, e, ecs_world);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Species {
    Human,
    Dog,
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
            "dog" => Species::Dog,
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
