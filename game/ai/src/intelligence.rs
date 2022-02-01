use std::cell::Cell;
use std::collections::HashMap;
use std::hint::unreachable_unchecked;

use derivative::Derivative;

use common::bumpalo::Bump;
use common::*;

use crate::context::Action;
use crate::decision::Dse;
pub use crate::intelligence::realisation::{RealisedDseIndex, RealisedDses};
use crate::{AiBox, Consideration, Context, Input, WeightedDse};

// TODO bump allocator should not expose bumpalo specifically

// TODO pool/arena allocator
/// Collection of DSEs
pub struct Smarts<C: Context>(Vec<AiBox<dyn Dse<C>>>);

pub struct Intelligence<C: Context> {
    /// Unchanging base behaviours e.g. from species
    base: Smarts<C>,

    /// Additional, temporary behaviours based on context e.g. in a particular location
    additional: HashMap<C::AdditionalDseId, Smarts<C>>,

    last_action: Cell<C::Action>,

    /// Only populated during thinking
    decision_progress: Option<DecisionProgress<C>>,
}

/// Not actually static, but only lives as long as the thinking process this tick
type RealisedDsesForTick<C> = RealisedDses<'static, C>;

pub enum DecisionProgress<C: Context> {
    NoChoice,

    TakenWhileInProgress,

    InitialChoice {
        dses: RealisedDsesForTick<C>,
        candidate: RealisedDseIndex,
        blackboard: Box<C::Blackboard>,
        score: f32,
    },

    InitialChoiceDenied {
        dses: RealisedDsesForTick<C>,
        blackboard: Box<C::Blackboard>,
    },

    Decided {
        dses: RealisedDsesForTick<C>,
        candidate: RealisedDseIndex,
        blackboard: Box<C::Blackboard>,
    },
}

pub struct InitialChoice<'a, C: Context> {
    pub source: DecisionSource<C>,
    pub target: Option<C::DseTarget>,
    pub dse: &'a dyn Dse<C>,
    pub score: f32,
}

pub struct IntelligenceContext<'a, C: Context> {
    pub blackboard: &'a mut C::Blackboard,
    pub target: Option<C::DseTarget>,
    pub input_cache: InputCache<'a, C>,
    pub best_so_far: f32,
    pub alloc: &'a bumpalo::Bump,
}

/// Final decision
pub enum IntelligentDecision<C: Context> {
    Undecided,
    Unchanged,
    New {
        dse_name: &'static str,
        action: C::Action,
        src: DecisionSource<C>,
    },
}

#[derive(Derivative)]
#[derivative(Debug(bound = ""), Clone(bound = ""))]
pub enum DecisionSource<C: Context> {
    Base(DseIndex),
    Additional(C::AdditionalDseId, DseIndex),
    Stream(
        DseIndex,
        #[derivative(Debug = "ignore")] C::StreamDseExtraData,
    ),
}

pub trait DseSkipper<C: Context> {
    /// When the initial choice was denied, all scored DSEs will be sorted and iterated in order,
    /// and the first to return false from this will be chosen as the final decision.
    fn should_skip(
        &self,
        dse: &dyn Dse<C>,
        tgt: Option<&C::DseTarget>,
        src: &DecisionSource<C>,
    ) -> bool;
}

pub struct InputCache<'a, C: Context>(BumpVec<'a, (C::Input, Option<C::DseTarget>, f32)>);

impl<'a, C: Context> InputCache<'a, C> {
    pub fn new(alloc: &'a bumpalo::Bump) -> Self {
        InputCache(BumpVec::with_capacity_in(16, alloc))
    }

    pub fn get(
        &mut self,
        input: C::Input,
        blackboard: &mut C::Blackboard,
        target: Option<&C::DseTarget>,
    ) -> f32 {
        // TODO use an arena-allocator hashmap
        // TODO perfect hash on C::Input
        if let Some((_, _, val)) = self.0.iter().find(|(ty, tgt, _)| {
            if *ty == input {
                match (tgt, target) {
                    (None, None) => true, // no target for either
                    (Some(a), Some(b)) if a == b => true,
                    _ => false,
                }
            } else {
                false
            }
        }) {
            *val
        } else {
            let val = input.get(blackboard, target);
            self.0.push((input, target.cloned(), val));
            val
        }
    }
}

impl<C: Context> Smarts<C> {
    pub fn new(dses: impl Iterator<Item = AiBox<dyn Dse<C>>>) -> Self {
        let dses = dses.collect_vec();
        if dses.is_empty() {
            warn!("smarts has zero DSEs");
        }
        Self(dses)
    }

    pub fn count(&self) -> usize {
        self.0.len()
    }
}

/// An index into DSEs for current tick. Valid while thinking is in progress as indices don't change
#[derive(Copy, Clone, Debug)]
pub struct DseIndex(usize);

impl<C: Context> Intelligence<C> {
    pub fn new(base_dses: impl Iterator<Item = AiBox<dyn Dse<C>>>) -> Self {
        let base = Smarts::new(base_dses);
        assert!(!base.0.is_empty(), "at least 1 DSE needed for species");
        Self {
            base,
            additional: HashMap::new(),
            last_action: Cell::default(),
            decision_progress: None,
        }
    }

    /// "Stream" behaviours only apply to a single tick, avoiding the overhead of adding then
    /// immediately removing additional behaviours.
    ///
    /// Returns None if no DSE is chosen
    pub fn choose_with_stream_dses(
        &mut self,
        mut blackboard: Box<C::Blackboard>,
        alloc: &bumpalo::Bump,
        streams: impl Iterator<Item = (WeightedDse<C>, C::StreamDseExtraData)>,
    ) -> Option<InitialChoice<C>> {
        // realise all dses and assign targets if any
        let mut dses = RealisedDses::new(alloc, self, streams, &mut blackboard);

        // score all dses
        let mut context = IntelligenceContext::<C>::new(&mut blackboard, alloc);
        for dse in dses.iter() {
            if *dse.score < context.best_so_far {
                trace!("skipping {dse} entirely due to its initial bonus weight being below the best result so far",
                    dse = dse.name; "best_so_far" => context.best_so_far);
                *dse.score = 0.0;
                continue;
            }

            // assign target
            context.target = dse.target.clone();

            // TODO add momentum to initial weight to discourage changing mind so often

            log_scope!(o!("dse" => dse.name));
            let dse_score = dse.score(&mut context, *dse.score);
            trace!("DSE scored {score}", score = dse_score; "target" => ?context.target);

            if dse_score > context.best_so_far {
                context.best_so_far = dse_score;
            }

            *dse.score = dse_score;
        }
        drop(context);

        // find the best score for initial choice
        let best = dses.find_best();
        match best {
            Some((candidate, score)) => {
                let dses = unsafe {
                    std::mem::transmute::<RealisedDses<'_, C>, RealisedDses<'static, C>>(dses)
                };

                self.decision_progress = Some(DecisionProgress::InitialChoice {
                    score,
                    dses,
                    blackboard,
                    candidate,
                });

                let (dse, target, source) = {
                    let dses = match &self.decision_progress {
                        Some(DecisionProgress::InitialChoice { dses, .. }) => dses,
                        _ => {
                            debug_assert!(false);
                            // safety: just assigned
                            unsafe { unreachable_unchecked() }
                        }
                    };

                    dses.resolve_dse(candidate, self)
                        .expect("candidate should be valid")
                };

                Some(InitialChoice {
                    dse,
                    source,
                    target,
                    score,
                })
            }

            None => {
                self.decision_progress = Some(DecisionProgress::NoChoice);
                None
            }
        }
    }

    pub fn choose(
        &mut self,
        blackboard: Box<C::Blackboard>,
        alloc: &bumpalo::Bump,
        cmp_arg: &<C::Action as Action>::Arg,
    ) -> IntelligentDecision<C> {
        let _ = self.choose_with_stream_dses(blackboard, alloc, empty());
        self.consume_decision(cmp_arg)
    }

    #[cfg(test)]
    pub fn choose_now_with_stream_dses(
        &mut self,
        blackboard: Box<C::Blackboard>,
        alloc: &bumpalo::Bump,
        cmp_arg: &<C::Action as Action>::Arg,
        streams: impl Iterator<Item = (WeightedDse<C>, C::StreamDseExtraData)>,
    ) -> IntelligentDecision<C> {
        let _ = self.choose_with_stream_dses(blackboard, alloc, streams);
        self.consume_decision(cmp_arg)
    }

    pub fn add_smarts(
        &mut self,
        id: C::AdditionalDseId,
        dses: impl Iterator<Item = AiBox<dyn Dse<C>>>,
    ) {
        self.ensure_modifications_allowed();
        let smarts = Smarts::new(dses);
        let count = smarts.0.len();
        if let Some(old) = self.additional.insert(id, smarts) {
            // TODO reuse allocation
            debug!(
                "replaced {prev_count} additional DSEs with {count}",
                prev_count = old.0.len(),
                count = count;
                "dse_id" => ?id
            );
        }
    }

    pub fn pop_smarts(&mut self, id_to_remove: &C::AdditionalDseId) {
        self.ensure_modifications_allowed();
        let _ = self.additional.remove(id_to_remove);
    }

    /// If in progress, do not allow any modifications
    fn thinking_in_progress(&self) -> bool {
        self.decision_progress.is_some()
    }

    fn ensure_modifications_allowed(&self) {
        assert!(
            !self.thinking_in_progress(),
            "cannot modify behaviours while thinking"
        )
    }

    pub fn last_action(&self) -> &C::Action {
        let ptr = self.last_action.as_ptr();
        // safety: same lifetime as self
        unsafe { &*ptr }
    }

    pub fn clear_last_action(&mut self) {
        trace!("clearing last action to Nop");
        self.last_action.replace(C::Action::default());
    }

    pub fn take_decision_in_progress(&mut self) -> Option<DecisionProgress<C>> {
        std::mem::replace(
            &mut self.decision_progress,
            Some(DecisionProgress::TakenWhileInProgress),
        )
    }

    pub fn update_decision_in_progress(&mut self, new_progress: DecisionProgress<C>) {
        let progress = match self.decision_progress.as_mut() {
            Some(prog @ DecisionProgress::TakenWhileInProgress) => prog,
            _ => unreachable!("decision should be in progress"),
        };
        *progress = new_progress;
    }

    /// Arrives at final decision
    pub fn choose_best_with_skipper(&mut self, skipper: impl DseSkipper<C>) {
        let (blackboard, mut dses) = match self.decision_progress.take() {
            Some(DecisionProgress::InitialChoiceDenied { blackboard, dses }) => (blackboard, dses),
            _ => unreachable!("unexpected decision progress"),
        };

        self.decision_progress = Some(match dses.find_next_best(self, skipper) {
            Some(idx) => DecisionProgress::Decided {
                candidate: idx,
                blackboard,
                dses,
            },
            None => DecisionProgress::NoChoice,
        });
    }

    pub fn consume_decision(&mut self, arg: &<C::Action as Action>::Arg) -> IntelligentDecision<C> {
        let (action, source) = match self.decision_progress.take().expect("thinking expected") {
            DecisionProgress::NoChoice => {
                trace!("intelligence chose nothing");
                (C::Action::default(), None)
            }
            DecisionProgress::TakenWhileInProgress
            | DecisionProgress::InitialChoiceDenied { .. } => unreachable!(),
            DecisionProgress::Decided {
                mut blackboard,
                candidate,
                dses,
            }
            | DecisionProgress::InitialChoice {
                mut blackboard,
                candidate,
                dses,
                ..
            } => {
                let (dse, target, source) = dses
                    .resolve_dse(candidate, self)
                    .expect("dse source expected to be valid");

                trace!("intelligence chose {dse}", dse = dse.name(); "index" => ?candidate, "detail" => ?dse.as_debug(),
                "target" => ?target);

                let action = dse.action(&mut blackboard, target);
                (action, Some((dse.name(), source)))
            }
        };

        let last_action = self.last_action.replace(action.clone());
        if last_action.cmp(&action, arg) {
            IntelligentDecision::Unchanged
        } else if let Some((dse_name, src)) = source {
            IntelligentDecision::New {
                dse_name,
                action,
                src,
            }
        } else {
            IntelligentDecision::Undecided
        }
    }

    #[cfg(test)]
    pub fn iter_scores(
        &mut self,
    ) -> impl Iterator<Item = (&'static str, f32, Option<C::DseTarget>)> + '_ {
        let mut dses = match self.decision_progress.as_mut() {
            Some(
                DecisionProgress::InitialChoice { dses, .. }
                | DecisionProgress::Decided { dses, .. },
            ) => dses,
            _ => unreachable!("not thinking"),
        };

        dses.iter().map(|dse| (dse.name, *dse.score, dse.target))
    }
}

impl<'a, C: Context> IntelligenceContext<'a, C> {
    pub fn new(blackboard: &'a mut C::Blackboard, alloc: &'a Bump) -> Self {
        Self {
            blackboard,
            input_cache: InputCache::new(alloc),
            best_so_far: 0.0,
            alloc,
            target: None,
        }
    }
}

pub struct DseToScore<'a, C: Context> {
    pub name: &'static str,
    pub considerations: &'a [&'a dyn Consideration<C>],
    pub target: Option<C::DseTarget>,
    pub score: &'a mut f32,
}

impl<'a, C: Context> DseToScore<'a, C> {
    fn score(&self, context: &mut IntelligenceContext<C>, bonus: f32) -> f32 {
        // starts as the maximum possible score (i.e. all considerations are 1.0)
        let mut final_score = bonus;

        let modification_factor = 1.0 - (1.0 / self.considerations.len() as f32);
        for c in self.considerations {
            if final_score < context.best_so_far {
                trace!("skipping {dse} due to falling below best result found so far", dse = self.name;
                       "current_score" => final_score, "best_so_far" => context.best_so_far);
                return 0.0;
            }

            let score = c
                .consider(
                    context.blackboard,
                    context.target.as_ref(),
                    &mut context.input_cache,
                )
                .value();

            // compensation factor balances overall drop when multiplying multiple floats by
            // taking into account the number of considerations
            let make_up_value = (1.0 - score) * modification_factor;
            let compensated_score = score + (make_up_value * score);
            debug_assert!(compensated_score <= 1.0);

            let evaluated_score = c
                .curve()
                .evaluate(NormalizedFloat::new(compensated_score))
                .value();

            trace!("consideration scored {score}", score = evaluated_score; "consideration" => c.name(), "raw" => score);

            #[cfg(feature = "logging")]
            {
                use crate::Blackboard;
                c.log_metric(&blackboard.entity(), evaluated_score);
            }

            debug_assert!(
                (0.0..=1.0).contains(&evaluated_score),
                "evaluated score {} out of range",
                evaluated_score
            );

            if evaluated_score <= 0.0 {
                // will never financially recover from this
                final_score = 0.0;
                trace!("bailing out of dse early due to reaching 0");
                break;
            }

            final_score *= evaluated_score;
        }

        debug_assert!(final_score <= bonus);
        final_score
    }
}

mod realisation {
    use derivative::Derivative;
    use float_ord::FloatOrd;

    use common::bumpalo::collections::CollectIn;
    use common::bumpalo::Bump;
    use common::BumpVec;

    use crate::intelligence::{DseIndex, DseToScore};
    use crate::{
        Consideration, Considerations, Context, DecisionSource, Dse, DseSkipper, Intelligence,
        TargetOutput, Targets, WeightedDse,
    };

    #[derive(Derivative)]
    #[derivative(Clone(bound = ""))]
    struct RealisedDse<'a, C: Context> {
        name: &'static str,
        considerations: BumpVec<'a, &'a dyn Consideration<C>>,
        target: Option<C::DseTarget>,
        source: DecisionSource<C>,
    }

    pub struct RealisedDses<'a, C: Context> {
        dses: BumpVec<'a, RealisedDse<'a, C>>,

        /// Parallel to `dses`
        scores: BumpVec<'a, f32>,

        /// Stream DSEs for this tick
        streams: BumpVec<'a, (WeightedDse<C>, C::StreamDseExtraData)>,

        /// Sorted parallel to `scores` that points into `dses`
        sorted_score_indices: BumpVec<'a, RealisedDseIndex>,
    }

    #[derive(Copy, Clone, Debug)]
    pub struct RealisedDseIndex(usize);

    impl<'a, C: Context> RealisedDses<'a, C> {
        pub fn new(
            bump: &'a Bump,
            intelligence: &Intelligence<C>,
            streams: impl Iterator<Item = (WeightedDse<C>, C::StreamDseExtraData)>,
            blackboard: &mut C::Blackboard,
        ) -> Self {
            let streams = streams.collect_in::<BumpVec<_>>(bump);
            let mut scores = BumpVec::with_capacity_in(
                intelligence.base.count()
                    + intelligence
                        .additional
                        .values()
                        .map(|smarts| smarts.count())
                        .sum::<usize>()
                    + streams.len(),
                bump,
            );

            let mut dses = BumpVec::with_capacity_in(scores.capacity(), bump);

            let mut considerations = Considerations::new(bump);
            let mut targets = Targets::new(bump);
            for (dse, multiplier, src) in iter_all_dses_with_sources(intelligence, &streams) {
                let score = dse.weight().multiplier() * multiplier;
                dse.considerations(&mut considerations);

                let realised = RealisedDse {
                    name: dse.name(),
                    considerations: considerations.drain().collect_in(bump),
                    target: None,
                    source: src,
                };

                match dse.target(&mut targets, blackboard) {
                    TargetOutput::Untargeted => {
                        dses.push(realised);
                        scores.push(score);
                    }
                    TargetOutput::TargetsCollected => dses.extend(targets.drain().map(|tgt| {
                        scores.push(score);
                        RealisedDse {
                            target: Some(tgt),
                            ..realised.clone()
                        }
                    })),
                }
            }

            assert_eq!(dses.len(), scores.len());

            RealisedDses {
                dses,
                scores,
                sorted_score_indices: BumpVec::new_in(bump),
                streams,
            }
        }

        pub fn iter(&mut self) -> impl Iterator<Item = DseToScore<C>> + '_ {
            self.dses
                .iter()
                .zip(self.scores.iter_mut())
                .map(|(dse, score)| DseToScore {
                    name: dse.name,
                    considerations: &dse.considerations,
                    target: dse.target.clone(),
                    score,
                })
        }

        pub fn find_best(&self) -> Option<(RealisedDseIndex, f32)> {
            self.scores
                .iter()
                .enumerate()
                .max_by_key(|(_, f)| FloatOrd(**f))
                .map(|(idx, score)| (RealisedDseIndex(idx), *score))
        }

        pub fn resolve_dse<'me: 'i, 'i>(
            &'me self,
            idx: RealisedDseIndex,
            intelligence: &'i Intelligence<C>,
        ) -> Option<(&'i dyn Dse<C>, Option<C::DseTarget>, DecisionSource<C>)> {
            let realised = self.dses.get(idx.0)?;

            let dse = match realised.source {
                DecisionSource::Base(i) => intelligence.base.0.get(i.0).map(|dse| &**dse),
                DecisionSource::Additional(key, i) => intelligence
                    .additional
                    .get(&key)
                    .and_then(|dses| dses.0.get(i.0).map(|dse| &**dse)),
                DecisionSource::Stream(i, _) => {
                    self.streams.get(i.0).map(|(weighted, _)| weighted.dse())
                }
            }?;

            Some((dse, realised.target.clone(), realised.source.clone()))
        }

        pub fn find_next_best(
            &mut self,
            intelligence: &Intelligence<C>,
            skipper: impl DseSkipper<C>,
        ) -> Option<RealisedDseIndex> {
            // sort scores but keep the original order
            self.sorted_score_indices
                .resize(self.scores.len(), RealisedDseIndex(0));
            self.sorted_score_indices
                .iter_mut()
                .enumerate()
                .for_each(|(i, idx)| *idx = RealisedDseIndex(i));

            {
                let scores_ref = &self.scores[..];
                self.sorted_score_indices.sort_unstable_by_key(|i| {
                    debug_assert!(scores_ref.get(i.0).is_some());
                    let score = unsafe { scores_ref.get_unchecked(i.0) };
                    FloatOrd(*score)
                });
            }

            // find the best that shouldn't be skipped
            self.sorted_score_indices
                .iter()
                .rev() // best at the end
                .find_map(|idx| {
                    let (dse, tgt, src) = self
                        .resolve_dse(*idx, intelligence)
                        .expect("dse index expected to be valid");
                    if !skipper.should_skip(dse, tgt.as_ref(), &src) {
                        Some(*idx)
                    } else {
                        None
                    }
                })
        }
    }

    fn iter_all_dses_with_sources<'a, C: Context>(
        intel: &'a Intelligence<C>,
        streams: &'a [(WeightedDse<C>, C::StreamDseExtraData)],
    ) -> impl Iterator<Item = (&'a dyn Dse<C>, f32, DecisionSource<C>)> {
        let base = intel
            .base
            .0
            .iter()
            .enumerate()
            .map(|(i, dse)| (&**dse, 1.0, DecisionSource::Base(DseIndex(i))));

        let additional = intel
            .additional
            .iter()
            .map(|(key, smarts)| {
                smarts.0.iter().enumerate().map(move |(i, dse)| {
                    (&**dse, 1.0, DecisionSource::Additional(*key, DseIndex(i)))
                })
            })
            .flatten();

        let stream = streams.iter().enumerate().map(|(i, (weighted, data))| {
            (
                weighted.dse(),
                weighted.multiplier(),
                DecisionSource::Stream(DseIndex(i), data.clone()),
            )
        });

        base.chain(additional).chain(stream)
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;
    use std::iter::empty;

    use float_ord::FloatOrd;

    use common::{bumpalo, once, Itertools};

    use crate::consideration::Considerations;
    use crate::decision::WeightedDse;
    use crate::intelligence::realisation::RealisedDses;
    use crate::intelligence::{DseIndex, IntelligenceContext};
    use crate::test_utils::*;
    use crate::{
        AiBox, Consideration, ConsiderationParameter, Context, Curve, DecisionSource,
        DecisionWeight, Dse, Intelligence, IntelligentDecision, TargetOutput, Targets,
    };

    #[test]
    fn extra_dses() {
        let blackboard = Box::new(TestBlackboard {
            my_hunger: 0.5,
            ..Default::default()
        });

        let dses = vec![
            AiBox::new(EatDse) as AiBox<dyn Dse<TestContext>>,
            AiBox::new(BadDse) as AiBox<dyn Dse<TestContext>>,
        ];

        let mut intelligence = Intelligence::new(dses.into_iter());
        let alloc = bumpalo::Bump::new();

        // eat wins
        assert!(matches!(
            intelligence.choose(blackboard.clone(), &alloc, &()),
            IntelligentDecision::New {
                action: TestAction::Eat,
                ..
            }
        ));
        assert!(matches!(
            intelligence.choose(blackboard.clone(), &alloc, &()),
            IntelligentDecision::Unchanged
        ));

        // add additional emergency
        intelligence.add_smarts(
            100,
            vec![AiBox::new(EmergencyDse) as AiBox<dyn Dse<TestContext>>].into_iter(),
        );
        assert!(matches!(
            intelligence.choose(blackboard.clone(), &alloc, &()),
            IntelligentDecision::New {
                action: TestAction::CancelExistence,
                ..
            }
        ));

        // pop it, back to original
        intelligence.pop_smarts(&100);
        assert!(matches!(
            intelligence.choose(blackboard.clone(), &alloc, &()),
            IntelligentDecision::New {
                action: TestAction::Eat,
                ..
            }
        ));

        // add emergency as stream
        let streams = once((WeightedDse::new(EmergencyDse, 1.0), ()));
        assert!(matches!(
            intelligence.choose_now_with_stream_dses(blackboard.clone(), &alloc, &(), streams),
            IntelligentDecision::New {
                action: TestAction::CancelExistence,
                ..
            }
        ));
    }

    //noinspection DuplicatedCode
    #[test]
    fn society_task_reservation_weight() {
        let blackboard = Box::new(TestBlackboard {
            my_hunger: 0.5,
            ..Default::default()
        });

        #[derive(Clone, Hash, Eq, PartialEq)]
        pub struct ConfigurableDse(TestAction);

        impl Dse<TestContext> for ConfigurableDse {
            fn considerations(&self, out: &mut Considerations<TestContext>) {
                out.add(ConstantConsideration(50));
            }

            fn weight(&self) -> DecisionWeight {
                DecisionWeight::Normal
            }

            fn action(&self, blackboard: &mut TestBlackboard, target: Option<u32>) -> TestAction {
                self.0.clone()
            }

            fn name(&self) -> &'static str {
                "Configurable"
            }
        }

        let dses =
            vec![AiBox::new(ConfigurableDse(TestAction::Eat)) as AiBox<dyn Dse<TestContext>>];

        let mut intelligence = Intelligence::new(dses.into_iter());

        // choose the only available Eat
        let alloc = bumpalo::Bump::new();
        match intelligence.choose(blackboard.clone(), &alloc, &()) {
            IntelligentDecision::New { action, src, .. } => {
                assert_eq!(action, TestAction::Eat);
                assert!(matches!(src, DecisionSource::Base(DseIndex(0))));
            }
            _ => unreachable!(),
        };

        // add a weighted dse
        let weighted = WeightedDse::new(ConfigurableDse(TestAction::Nop), 1.1);
        let mut streams = vec![(weighted, ())];

        // choose the new weighted Nop
        let alloc = bumpalo::Bump::new();
        match intelligence.choose_now_with_stream_dses(
            blackboard.clone(),
            &alloc,
            &(),
            streams.iter().cloned(),
        ) {
            IntelligentDecision::New { action, src, .. } => {
                assert_eq!(action, TestAction::Nop); // changed
                assert!(matches!(src, DecisionSource::Stream(DseIndex(0), _)));
            }
            _ => unreachable!(),
        };

        // back to a higher Eat
        streams.push((WeightedDse::new(ConfigurableDse(TestAction::Eat), 1.9), ()));

        match intelligence.choose_now_with_stream_dses(
            blackboard.clone(),
            &alloc,
            &(),
            streams.into_iter(),
        ) {
            IntelligentDecision::New { action, src, .. } => {
                assert_eq!(action, TestAction::Eat); // changed
                assert!(matches!(src, DecisionSource::Stream(DseIndex(1), _)));
            }
            _ => unreachable!(),
        };
    }

    #[derive(Clone, Hash, Eq, PartialEq)]
    pub struct TargetedDse;

    pub struct IsTargetFiveConsideration;

    impl Consideration<TestContext> for IsTargetFiveConsideration {
        fn curve(&self) -> Curve {
            Curve::Identity
        }

        fn input(&self) -> TestInput {
            TestInput::IsTargetFive
        }

        fn parameter(&self) -> ConsiderationParameter {
            ConsiderationParameter::Nop
        }
    }

    impl Dse<TestContext> for TargetedDse {
        fn considerations(&self, out: &mut Considerations<TestContext>) {
            out.add(IsTargetFiveConsideration);
        }

        fn weight(&self) -> DecisionWeight {
            DecisionWeight::BasicNeeds
        }

        fn action(&self, blackboard: &mut TestBlackboard, target: Option<u32>) -> TestAction {
            let target = target.expect("expected target");
            TestAction::Attack(target)
        }

        fn target(
            &self,
            targets: &mut Targets<TestContext>,
            blackboard: &mut TestBlackboard,
        ) -> TargetOutput {
            for tgt in blackboard.targets.iter().copied() {
                targets.add(tgt);
            }
            TargetOutput::TargetsCollected
        }
    }

    #[test]
    fn dse_realisation() {
        let mut blackboard = Box::new(TestBlackboard {
            my_hunger: 0.5,
            targets: vec![100, 5],
        });
        let alloc = bumpalo::Bump::new();

        let dses = vec![
            AiBox::new(EatDse) as AiBox<dyn Dse<TestContext>>,
            AiBox::new(TargetedDse) as AiBox<dyn Dse<TestContext>>,
            AiBox::new(BadDse) as AiBox<dyn Dse<TestContext>>,
        ];

        let mut intelligence = Intelligence::new(dses.into_iter());

        let _ = intelligence.choose_with_stream_dses(blackboard, &alloc, empty());

        let scores = intelligence
            .iter_scores()
            .sorted_by_key(|(_, score, _)| FloatOrd(*score))
            .collect_vec();

        assert_eq!(scores.len(), 4); // 2 targeted + 2 others

        let best = &scores[3];
        assert_eq!(best.0, "Targeted");
        assert_eq!(best.2, Some(5)); // it was 5 so its the best

        let worst = &scores[0];
        assert_eq!(worst.0, "Targeted");
        assert_eq!(worst.2, Some(100)); // it wasn't 5 so its the worst

        assert_eq!(scores[1].1, 0.0); // Bad
        assert_eq!(scores[2].1, 1.0); // Eat

        match intelligence.consume_decision(&()) {
            IntelligentDecision::New {
                dse_name: "Targeted",
                action: TestAction::Attack(5),
                src: DecisionSource::Base(_),
            } => {}
            _ => unreachable!(),
        };
    }
}
