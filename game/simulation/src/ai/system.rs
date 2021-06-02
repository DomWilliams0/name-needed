use std::collections::HashMap;
use std::iter::once;

use ai::{AiBox, DecisionSource, Dse, Intelligence, IntelligentDecision};
use common::*;

use crate::activity::{ActivityComponent, ActivityComponent2};
use crate::ai::dse::{dog_dses, human_dses, AdditionalDse, ObeyDivineCommandDse};
use crate::ai::{AiAction, AiBlackboard, AiContext, SharedBlackboard};
use crate::ecs::*;
use crate::item::InventoryComponent;
use crate::needs::HungerComponent;
use crate::simulation::Tick;
use crate::society::job::SocietyTask;
use crate::society::{Society, SocietyComponent};
use crate::{EntityLoggingComponent, TransformComponent};

use crate::job::JobIndex;
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
        new_src: Option<&DecisionSource<AiContext>>,
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
                    if let Some(DecisionSource::Stream(_)) = new_src {
                        // interrupting society task with a new society task, no need to manually cancel
                    } else {
                        // society task interrupted by a non-society task, so manual cancellation is needed
                        let society = society_fn();
                        society.jobs_mut().reservations_mut().cancel(this_entity);
                    }
                }
                _ => {}
            }
        }
    }
    // }
}

pub struct AiSystem;

impl<'a> System<'a> for AiSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, EcsWorldFrameRef>,
        Write<'a, Societies>,
        ReadStorage<'a, TransformComponent>,
        ReadStorage<'a, HungerComponent>,    // optional
        ReadStorage<'a, InventoryComponent>, // optional
        WriteStorage<'a, AiComponent>,
        WriteStorage<'a, ActivityComponent2>,
        WriteStorage<'a, SocietyComponent>,       // optional
        WriteStorage<'a, EntityLoggingComponent>, // optional
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

        for (e, transform, hunger_opt, inventory_opt, ai, activity, society_opt) in (
            &entities,
            &transform,
            hunger.maybe(),
            inventory.maybe(),
            &mut ai,
            &mut activity,
            society.maybe(),
        )
            .join()
        {
            let e = Entity::from(e);
            log_scope!(o!("system" => "ai", e));
            let society_opt: Option<&SocietyComponent> = society_opt; // for IDE

            // initialize blackboard
            // TODO use arena/bump allocator and share instance between entities
            let mut bb = AiBlackboard {
                entity: e,
                accessible_position: transform.accessible_position(),
                position: transform.position,
                hunger: hunger_opt.map(|h| h.hunger()),
                inventory_search_cache: HashMap::new(),
                local_area_search_cache: HashMap::new(),
                inventory: inventory_opt,
                world: ecs_world,
                shared: &mut shared_bb,
            };

            // safety: can't use true lifetime on Blackboard so using 'static and transmuting until
            // we get our GATs
            let bb_ref: &'a mut AiBlackboard = unsafe { std::mem::transmute(&mut bb) };

            // collect extra actions from society job list, if any
            // TODO provide READ ONLY DSEs to ai intelligence
            // TODO use dynstack to avoid so many small temporary allocations, or arena allocator
            // TODO fix eventually false assumption that all stream DSEs come from a society
            let mut society = society_opt.and_then(|comp| comp.resolve(societies));
            let extra_dses = self.collect_society_tasks(e, tick, society.as_mut(), ecs_world);

            // choose best action
            let streamed_dse = extra_dses.iter().map(|(_, _, dse)| &**dse);
            let decision = ai
                .intelligence
                .choose_with_stream_dses(bb_ref, streamed_dse);

            if let IntelligentDecision::New { dse, action, src } = decision {
                debug!("new activity"; "dse" => dse.name(), "source" => ?src);
                trace!("activity action"; "action" => ?action);

                // register interruption
                ai.interrupt_current_action(e, Some(&src), || {
                    society
                        .as_mut()
                        .expect("streamed DSEs expected to come from a society only")
                });

                let society_task = if let DecisionSource::Stream(i) = src {
                    // a society task was chosen, reserve it
                    let society = society
                        .as_mut()
                        .expect("streamed DSEs expected to come from a society only");

                    let (task, job_idx, _) = &extra_dses[i];
                    let mut jobs = society.jobs_mut();

                    // reserve task
                    jobs.reservations_mut().reserve(task.clone(), e);

                    // get job reference from index (avoiding the need to clone all job refs even
                    // when not chosen)
                    let job = jobs
                        .by_index(*job_idx)
                        .expect("jobs should not have changed");

                    Some((job, task.clone()))
                } else {
                    None
                };

                // log decision
                if let Some(logs) = e.get_mut(&mut logging) {
                    logs.log_event(&action);
                }

                ai.current = Some(src);

                // pass on to activity system
                activity.interrupt_with_new_activity(action, society_task, e, ecs_world);
            }

            // job indices are finished with, allow modifications again
            if let Some(society) = society {
                society.jobs_mut().allow_jobs_again();
            }
        }
    }
}

impl AiSystem {
    // TODO dont return a new vec of boxes, have some dignity
    /// Prevents further jobs being added to society until manually cleared
    fn collect_society_tasks(
        &self,
        entity: Entity,
        this_tick: Tick,
        society: Option<&mut &mut Society>,
        ecs_world: &EcsWorld,
    ) -> Vec<(SocietyTask, JobIndex, Box<dyn Dse<AiContext>>)> {
        let mut extra_dses = Vec::new();

        if let Some(society) = society {
            trace!("considering tasks for society"; "society" => ?society);

            let mut applicable_tasks = Vec::new(); // TODO reuse allocation
            let mut jobs = society.jobs_mut();
            jobs.filter_applicable_tasks(entity, this_tick, ecs_world, &mut applicable_tasks);

            debug_assert!(
                extra_dses.is_empty(),
                "society tasks expected to be the only source of extra dses"
            );

            extra_dses.extend(applicable_tasks.into_iter().filter_map(
                |(task, job_idx, reservations)| match task.as_dse(ecs_world, reservations) {
                    Some(dse) => Some((task, job_idx, dse)),
                    None => {
                        warn!("task failed conversion to DSE"; "task" => ?task);
                        None
                    }
                },
            ));

            trace!(
                "there are {count} tasks available",
                count = extra_dses.len();
            );
        }

        extra_dses
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
        builder.with(ai).with(ActivityComponent2::default())
    }
}

register_component_template!("intelligence", IntelligenceComponentTemplate);
