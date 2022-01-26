use std::cell::RefCell;
use std::collections::HashMap;
use std::iter::once;
use std::rc::Rc;

use ai::{
    AiBox, DecisionProgress, DecisionSource, Dse, DseSkipper, Intelligence, IntelligentDecision,
    WeightedDse,
};

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
                DecisionSource::Stream(_, _) => {
                    if let Some(DecisionSource::Stream(_, _)) = new_src {
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
}

/// Actually a pipeline of a few systems
pub struct AiSystem;

const MAX_DSE_CANDIDATES: usize = 5;

/// State shared between steps
struct PipelineState {
    best_society_dse_candidate: HashMap<
        AiBox<dyn Dse<AiContext>>,
        ArrayVec<(Entity, OrderedFloat<f32>), MAX_DSE_CANDIDATES>,
    >,
}

/// Step 1: tick all societies to prune finished and cancelled jobs
struct UpdateSocietyJobs;

/// Step 2: score all DSEs individually and choose the best as the initial choice. Tracks the top
/// candidates with the best scores for shareable society jobs
struct MakeInitialChoice<'a>(&'a mut PipelineState);

/// Step 3: accept non-disputed choices, and find the next best scored DSE that isn't disputed for
/// those that weren't among the best candidates for society jobs
struct FinaliseChoice<'a>(&'a mut PipelineState);

/// Step 4: assign decision and propagate to activity system
struct ConsumeDecision(PipelineState);

/// Data associated with society stream DSEs
#[derive(Clone)]
pub struct StreamDseData {
    pub society_task: SocietyTask,
    pub job_idx: JobIndex,
}

impl<'a> System<'a> for AiSystem {
    type SystemData = (Read<'a, EcsWorldRef>,);

    fn run(&mut self, (world,): Self::SystemData) {
        // TODO only run occasionally - FIXME TERRIBLE HACK
        let tick = Tick::fetch();
        if tick.value() % 10 != 0 {
            return;
        }

        UpdateSocietyJobs.run(<UpdateSocietyJobs as System<'a>>::SystemData::fetch(&world));

        // TODO bump alloc this
        let mut state = PipelineState {
            best_society_dse_candidate: HashMap::default(),
        };

        // everyone scores all dses and makes their initial choice
        MakeInitialChoice(&mut state)
            .run(<MakeInitialChoice as System<'a>>::SystemData::fetch(&world));

        // finalise choices so everyone has a decision
        FinaliseChoice(&mut state).run(<FinaliseChoice as System<'a>>::SystemData::fetch(&world));

        // consume final decision
        ConsumeDecision(state).run(<ConsumeDecision as System<'a>>::SystemData::fetch(&world));
    }
}

impl<'a> System<'a> for UpdateSocietyJobs {
    type SystemData = (Read<'a, EcsWorldRef>, Read<'a, Societies>);

    fn run(&mut self, (ecs_world, societies): Self::SystemData) {
        for society in societies.iter() {
            log_scope!(o!("society" => society.handle()));
            let mut jobs = society.jobs_mut();
            jobs.refresh_jobs(&ecs_world);
        }
    }
}

impl<'a> System<'a> for MakeInitialChoice<'a> {
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, EcsWorldRef>,
        Read<'a, Societies>,
        Read<'a, FrameAllocator>,
        ReadStorage<'a, TransformComponent>,
        ReadStorage<'a, HungerComponent>,    // optional
        ReadStorage<'a, InventoryComponent>, // optional
        WriteStorage<'a, AiComponent>,
        ReadStorage<'a, SocietyComponent>, // optional
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
            society,
        ): Self::SystemData,
    ) {
        let shared_bb = Rc::new(RefCell::new(SharedBlackboard::default()));

        for (e, transform, hunger_opt, inventory_opt, ai, society_opt) in (
            &entities,
            &transform,
            (&hunger).maybe(),
            (&inventory).maybe(),
            &mut ai,
            (&society).maybe(),
        )
            .join()
        {
            let e = Entity::from(e);
            trace!("making initial ai choice"; e);

            // initialize blackboard
            let bb = Box::new(AiBlackboard::new(
                e,
                transform,
                hunger_opt,
                inventory_opt,
                society_opt,
                ai,
                shared_bb.clone(),
                &ecs_world,
            ));

            // safety: can't use true lifetime on Blackboard so using 'static and transmuting until
            // we get our GATs
            let bb_ref =
                unsafe { std::mem::transmute::<Box<AiBlackboard>, Box<AiBlackboard<'static>>>(bb) };

            // collect extra actions from society job list, if any
            // TODO fix eventually false assumption that all stream DSEs come from a society
            let society = society_opt.and_then(|comp| comp.resolve(&*societies));
            let extra_dses = {
                let mut dses = BumpVec::new_in(alloc.allocator());
                collect_society_tasks(e, society, &ecs_world, |society_task, job_idx, dse| {
                    dses.push((
                        dse,
                        StreamDseData {
                            society_task,
                            job_idx,
                        },
                    ));
                });
                dses
            };

            // make initial choice
            let choice = ai.intelligence.choose_with_stream_dses(
                bb_ref,
                alloc.allocator(),
                extra_dses.into_iter(),
            );

            if let Some(choice) = choice.as_ref() {
                trace!("initial ai choice"; "source" => ?choice.source, "score" => ?choice.score, e);

                if let DecisionSource::Stream(_, data) = &choice.source {
                    // track top few best candidates for society jobs
                    let dse = ai
                        .intelligence
                        .dse(&choice.source)
                        .expect("source should be valid");

                    let score = OrderedFloat(choice.score);
                    self.0
                        .best_society_dse_candidate
                        .entry(dse.clone_dse()) // TODO frame alloc this
                        .and_modify(|best| {
                            let max_candidates = (data.society_task
                                .max_workers()
                                .get() as usize)
                                .min(MAX_DSE_CANDIDATES);

                            let end = best.len().min(max_candidates);
                            match best[..end].binary_search_by_key(&score, |(_, f)| *f) {
                                Ok(_) => unreachable!(), // entity has not been seen before
                                Err(idx) if idx >= max_candidates => {
                                    // not in top n
                                }
                                Err(idx) => {
                                    // insert into position, popping the current worst if necessary
                                    if best.len() == max_candidates {best.pop();}
                                    best.insert(idx, (e, score));
                                    trace!("new candidate chosen"; "dse" => ?dse.name(), e, "score" => ?score, "index" => idx);
                                }
                            }

                            // ensure sorted
                            debug_assert_eq!(
                                &best[..],
                                &best.iter()
                                    .copied()
                                    .sorted_by_key(|(_, f)| *f)
                                    .collect::<ArrayVec<_, MAX_DSE_CANDIDATES>>()[..]
                            );
                        })
                        .or_insert_with(|| {
                            trace!("initial candidate"; "dse" => ?dse.name(), e, "score" => ?score);
                            let mut vec = ArrayVec::new();
                            vec.push((e, score));
                            vec
                        });
                }
            }
        }
    }
}

impl<'a> System<'a> for FinaliseChoice<'a> {
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, FrameAllocator>,
        WriteStorage<'a, AiComponent>,
    );

    fn run(&mut self, (entities, alloc, mut ai): Self::SystemData) {
        // all entities now have an initial choice

        let mut ai_restricted = ai.restrict_mut();
        let mut denied_choices = BumpVec::new_in(alloc.allocator());
        for (e, mut ai_ref) in (&entities, &mut ai_restricted).join() {
            let e = Entity::from(e);
            trace!("finalising ai choice"; e);

            let ai = ai_ref.get_mut_unchecked();
            let progress = ai
                .intelligence
                .decision_in_progress()
                .expect("decision should be in progress");

            let mut denied = false;

            match progress {
                DecisionProgress::NoChoice => {}
                DecisionProgress::InitialChoice {
                    source: source @ DecisionSource::Stream(_, _),
                    blackboard,
                    ..
                } => {
                    // only accept the best candidates for the society dse
                    let dse = ai
                        .intelligence
                        .dse(&source)
                        .expect("source should be valid");
                    let best_candidates = self
                        .0
                        .best_society_dse_candidate
                        .get(dse)
                        .expect("society dse should be found");

                    if best_candidates.iter().any(|(candidate, _)| *candidate == e) {
                        // this is one of the best candidates
                        trace!("accepting candidate for society dse"; "source" => ?source, e);
                        ai.intelligence
                            .update_decision_in_progress(DecisionProgress::Decided {
                                source,
                                blackboard,
                            });
                    } else {
                        // need to choose the next best dse
                        trace!("denied for society job"; "source" => ?source, e);
                        denied = true;
                    }
                }
                DecisionProgress::InitialChoice {
                    source, blackboard, ..
                } => {
                    // non-societal choice, accept unconditionally
                    trace!("accepting non-societal choice"; "source" => ?source, e);
                    ai.intelligence
                        .update_decision_in_progress(DecisionProgress::Decided {
                            source,
                            blackboard,
                        });
                }
                DecisionProgress::Decided { .. } => {
                    unreachable!() // not used yet
                }
            }

            if denied {
                denied_choices.push((e, ai_ref));
            }
        }

        // find a better choice for those who were denied their initial choice
        for (e, mut ai) in denied_choices {
            let ai = ai.get_mut_unchecked();
            ai.intelligence.choose_best_with_skipper((self as _, e));
        }
    }
}

impl<'a> System<'a> for ConsumeDecision {
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, Societies>,
        WriteStorage<'a, AiComponent>,
        WriteStorage<'a, ActivityComponent>,
        WriteStorage<'a, SocietyComponent>,       // optional
        WriteStorage<'a, EntityLoggingComponent>, // optional
    );

    fn run(
        &mut self,
        (entities, societies, mut ai, mut activity, mut society, mut logging): Self::SystemData,
    ) {
        for (e, mut ai, activity, society, logging) in (
            &entities,
            &mut ai,
            &mut activity,
            (&mut society).maybe(),
            (&mut logging).maybe(),
        )
            .join()
        {
            let e = Entity::from(e);
            let decision = ai.intelligence.consume_decision();

            let (src, action) = match decision {
                IntelligentDecision::New { src, action, dse } => {
                    debug!("new activity chosen"; "dse" => dse.name(), "source" => ?src, e);
                    (Some(src), action)
                }
                IntelligentDecision::Undecided => {
                    debug!("no activity chosen so defaulting to nop"; e);
                    (None, AiAction::default())
                }
                IntelligentDecision::Unchanged => continue,
            };

            trace!("activity action"; "action" => ?action);

            // register interruption
            let society = society.and_then(|comp| comp.resolve(&societies));
            ai.interrupt_current_action(e, src.as_ref(), society);

            if let Some(src) = src.as_ref() {
                let society_task = if let DecisionSource::Stream(_, data) = src {
                    // a society task was chosen, reserve it
                    let society =
                        society.expect("streamed DSEs expected to come from a society only");

                    let StreamDseData {
                        society_task,
                        job_idx,
                    } = data;
                    let mut jobs = society.jobs_mut();

                    // reserve task
                    jobs.reservations_mut().reserve(society_task.clone(), e);

                    // get job reference from index (avoiding the need to clone all job refs even
                    // when not chosen)
                    let job = jobs
                        .by_index(*job_idx)
                        .expect("jobs should not have changed");

                    Some((job, society_task.clone()))
                } else {
                    None
                };

                // log decision
                if let Some(logs) = logging {
                    logs.log_event(&action);
                }

                // pass on to activity system
                activity.interrupt_with_new_activity(action, society_task);
            }

            ai.current = src;
        }
    }
}

impl DseSkipper<AiContext> for (&'_ FinaliseChoice<'_>, Entity) {
    fn should_skip(&self, dse: &dyn Dse<AiContext>, src: &DecisionSource<AiContext>) -> bool {
        let (system, me) = *self;
        let system_state = &system.0;

        if let DecisionSource::Stream(_, _) = src {
            let best_candidates = match system_state.best_society_dse_candidate.get(dse) {
                Some(vec) => vec,
                None => return false,
            };

            // dont skip if we're a candidate
            !best_candidates
                .iter()
                .any(|(candidate, _)| me == *candidate)
        } else {
            // dont skip non-societal dses
            false
        }
    }
}

/// Prevents further jobs being added to society until manually cleared
fn collect_society_tasks(
    entity: Entity,
    society: Option<&Society>,
    ecs_world: &EcsWorld,
    mut add_dse: impl FnMut(SocietyTask, JobIndex, WeightedDse<AiContext>),
) {
    if let Some(society) = society {
        trace!("considering tasks for society"; "society" => ?society);

        // TODO collect jobs from society directly, which can filter them from the applicable work items too
        let jobs = society.jobs();
        let mut n = 0usize;
        jobs.filter_applicable_tasks(entity, |task, job_idx, reservations| {
            match task.as_dse(ecs_world, reservations) {
                Some(dse) => {
                    add_dse(task, job_idx, dse);
                    n += 1;
                }
                None => {
                    warn!("task failed conversion to DSE"; "task" => ?task);
                }
            }
        });

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
