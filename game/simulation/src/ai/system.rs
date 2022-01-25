use std::iter::once;
use std::rc::Rc;

use ai::{AiBox, DecisionSource, Dse, Intelligence, IntelligentDecision};
use common::bumpalo::Bump;
use common::*;

use crate::activity::ActivityComponent;
use crate::ai::dse::{dog_dses, human_dses, AdditionalDse, ObeyDivineCommandDse};
use crate::ai::{AiAction, AiBlackboard, AiContext, SharedBlackboard};
use crate::alloc::FrameAllocator;
use crate::ecs::*;
use crate::item::InventoryComponent;
use crate::job::JobIndex;
use crate::needs::HungerComponent;
use crate::simulation::{EcsWorldRef, Tick};
use crate::society::job::SocietyTask;
use crate::society::{Society, SocietyComponent};
use crate::string::StringCache;
use crate::{dse, Societies};
use crate::{EntityLoggingComponent, TransformComponent};

#[derive(Component, EcsComponent)]
#[storage(DenseVecStorage)]
#[name("ai")]
#[clone(disallow)]
pub struct AiComponent {
    intelligence: ai::Intelligence<AiContext>,
    current: Option<DecisionSource<AiContext>>,
}

impl AiComponent {
    pub fn with_species(species: &Species) -> Self {
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

    pub fn last_action(&self) -> &AiAction {
        self.intelligence.last_action()
    }

    pub fn is_current_divine(&self) -> bool {
        self.current
            .as_ref()
            .map(|src| {
                matches!(
                    src,
                    DecisionSource::Additional(AdditionalDse::DivineCommand, _)
                )
            })
            .unwrap_or_default()
    }

    pub fn clear_last_action(&mut self) {
        self.intelligence.clear_last_action();
    }

    /// Panics if society is None if current action is a streamed source
    pub fn interrupt_current_action<'a>(
        &mut self,
        this_entity: Entity,
        new_src: Option<&DecisionSource<AiContext>>,
        society: Option<&'a Society>,
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
                        let society =
                            society.expect("streamed DSEs expected to come from a society only");
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
        Read<'a, EcsWorldRef>,
        Read<'a, Societies>,
        Read<'a, FrameAllocator>,
        ReadStorage<'a, TransformComponent>,
        ReadStorage<'a, HungerComponent>,    // optional
        ReadStorage<'a, InventoryComponent>, // optional
        WriteStorage<'a, AiComponent>,
        WriteStorage<'a, ActivityComponent>,
        WriteStorage<'a, SocietyComponent>,       // optional
        WriteStorage<'a, EntityLoggingComponent>, // optional
    );

    fn run(
        &mut self,
        (
            entities,
            ecs_world,
            societies,
            alloc,
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

        // TODO use frame allocator in ai blackboards
        let mut shared_bb = SharedBlackboard::default();

        for (e, transform, hunger_opt, inventory_opt, ai, activity, society_opt) in (
            &entities,
            &transform,
            (&hunger).maybe(),
            (&inventory).maybe(),
            &mut ai,
            &mut activity,
            (&society).maybe(),
        )
            .join()
        {
            let e = Entity::from(e);

            // initialize blackboard
            let mut bb = AiBlackboard::new(
                e,
                transform,
                hunger_opt,
                inventory_opt,
                society_opt,
                ai,
                &mut shared_bb,
                &ecs_world,
            );

            // safety: can't use true lifetime on Blackboard so using 'static and transmuting until
            // we get our GATs
            let bb_ref = unsafe {
                std::mem::transmute::<&mut AiBlackboard, &mut AiBlackboard<'static>>(&mut bb)
            };

            // collect extra actions from society job list, if any
            // TODO provide READ ONLY DSEs to ai intelligence
            // TODO fix eventually false assumption that all stream DSEs come from a society
            let society = society_opt.and_then(|comp| comp.resolve(&*societies));
            let extra_dses = {
                let mut dses = BumpVec::new_in(alloc.allocator());
                collect_society_tasks(
                    e,
                    tick,
                    society,
                    &ecs_world,
                    alloc.allocator(),
                    |society_task, job_idx, dse| {
                        dses.push((society_task, job_idx, dse));
                    },
                );
                dses
            };

            // choose best action
            let decision = ai.intelligence.choose_with_stream_dses(
                bb_ref,
                alloc.allocator(),
                extra_dses.iter().map(|(_, _, dse)| &**dse),
            );

            if let IntelligentDecision::New { dse, action, src } = decision {
                debug!("new activity"; "dse" => dse.name(), "source" => ?src);
                trace!("activity action"; "action" => ?action);

                // register interruption
                ai.interrupt_current_action(e, Some(&src), society);

                let society_task = if let DecisionSource::Stream(i) = src {
                    // a society task was chosen, reserve it
                    let society =
                        society.expect("streamed DSEs expected to come from a society only");

                    let (task, job_idx, _) = &extra_dses[i];

                    // reserve task
                    let mut jobs = society.jobs_mut();
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

                // pass decision on to activity system
                activity.interrupt_with_new_activity(action, society_task);
            }

            // job indices are finished with, allow modifications again
            if let Some(society) = society {
                society.jobs_mut().allow_jobs_again();
            }
        }
    }
}

/// Prevents further jobs being added to society until manually cleared
fn collect_society_tasks<'bump>(
    entity: Entity,
    this_tick: Tick,
    society: Option<&Society>,
    ecs_world: &EcsWorld,
    alloc: &'bump Bump,
    mut add_dse: impl FnMut(SocietyTask, JobIndex, BumpBox<'bump, dyn Dse<AiContext>>),
) {
    if let Some(society) = society {
        trace!("considering tasks for society"; "society" => ?society);

        // TODO collect jobs from society directly, which can filter them from the applicable work items too
        let mut jobs = society.jobs_mut();
        let mut n = 0usize;
        jobs.filter_applicable_tasks(
            entity,
            this_tick,
            ecs_world,
            |task, job_idx, reservations| match task.as_dse(ecs_world, reservations, alloc) {
                Some(dse) => {
                    add_dse(task, job_idx, dse);
                    n += 1;
                }
                None => {
                    warn!("task failed conversion to DSE"; "task" => ?task);
                }
            },
        );

        trace!("there are {count} tasks available", count = n);
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
    fn construct(
        values: &mut Map<V>,
        _: &StringCache,
    ) -> Result<Rc<dyn ComponentTemplate<V>>, ComponentBuildError>
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

        Ok(Rc::new(Self { species }))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        let ai = AiComponent::with_species(&self.species);
        builder.with(ai).with(ActivityComponent::default())
    }

    crate::as_any!();
}

register_component_template!("intelligence", IntelligenceComponentTemplate);
