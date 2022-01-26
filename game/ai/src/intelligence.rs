use std::cell::Cell;
use std::collections::HashMap;
use std::mem::ManuallyDrop;

use float_ord::FloatOrd;

use common::bumpalo::Bump;
use common::*;

use crate::decision::Dse;
use crate::{AiBox, Context, Input, WeightedDse};

// TODO bump allocator should not expose bumpalo specifically

// TODO pool/arena allocator
/// Collection of DSEs
pub struct Smarts<C: Context>(Vec<AiBox<dyn Dse<C>>>);

pub struct Intelligence<C: Context> {
    /// Unchanging base behaviours e.g. from species
    base: Smarts<C>,

    /// Additional, temporary behaviours based on context e.g. in a particular location
    additional: HashMap<C::AdditionalDseId, Smarts<C>>,

    /// Stream behaviours for the current tick
    // TODO framealloc this, with helper type to assert same tick i.e. still populated
    stream: Vec<(WeightedDse<C>, C::StreamDseExtraData)>,

    /// Parallel array that maps to Intelligence::all_dses
    scores: Vec<f32>,

    sorted_score_indices: Vec<usize>,

    last_action: Cell<C::Action>,

    /// Only populated during thinking
    decision_progress: Option<DecisionProgress<C>>,
}

pub enum DecisionProgress<C: Context> {
    NoChoice,

    InitialChoice {
        source: DecisionSource<C>,
        blackboard: Box<C::Blackboard>,
        score: f32,
    },

    Decided {
        source: DecisionSource<C>,
        blackboard: Box<C::Blackboard>,
    },
}

pub struct InitialChoice<C: Context> {
    pub source: DecisionSource<C>,
    pub score: f32,
}

pub struct IntelligenceContext<'a, C: Context> {
    pub blackboard: &'a mut C::Blackboard,
    pub input_cache: InputCache<'a, C>,
    pub best_so_far: f32,
    pub alloc: &'a bumpalo::Bump,
}

/// Final decision
pub enum IntelligentDecision<'a, C: Context> {
    Undecided,
    Unchanged,
    New {
        dse: &'a dyn Dse<C>,
        action: C::Action,
        src: DecisionSource<C>,
    },
}

pub enum DecisionSource<C: Context> {
    Base(DseIndex),
    Additional(C::AdditionalDseId, DseIndex),
    Stream(DseIndex, C::StreamDseExtraData),
}

pub trait DseSkipper<C: Context> {
    /// When the initial choice was denied, all scored DSEs will be sorted and iterated in order,
    /// and the first to return false from this will be chosen as the final decision.
    fn should_skip(&self, dse: &dyn Dse<C>, src: &DecisionSource<C>) -> bool;
}

pub struct InputCache<'a, C: Context>(BumpVec<'a, (C::Input, f32)>);

impl<'a, C: Context> InputCache<'a, C> {
    pub fn new(alloc: &'a bumpalo::Bump) -> Self {
        InputCache(BumpVec::with_capacity_in(16, alloc))
    }

    pub fn get(&mut self, input: C::Input, blackboard: &mut C::Blackboard) -> f32 {
        // TODO use an arena-allocator hashmap
        // TODO perfect hash on C::Input
        if let Some((_, val)) = self.0.iter().find(|(ty, _)| *ty == input) {
            *val
        } else {
            let val = input.get(blackboard);
            self.0.push((input, val));
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

/// Index into all_dses* functions. Valid while thinking is in progress as indices don't change
#[derive(Copy, Clone)]
pub struct DseIndex(usize);

impl<C: Context> Intelligence<C> {
    pub fn new(base_dses: impl Iterator<Item = AiBox<dyn Dse<C>>>) -> Self {
        let base = Smarts::new(base_dses);
        assert!(!base.0.is_empty(), "at least 1 DSE needed for species");
        Self {
            base,
            additional: HashMap::new(),
            stream: Vec::new(),
            scores: Vec::new(),
            sorted_score_indices: Vec::new(),
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
        // store stream behaviours
        self.stream.clear();
        self.stream.extend(streams);

        // alloc parallel scores array
        let total_count = self.base.count()
            + self
                .additional
                .values()
                .map(|smarts| smarts.count())
                .sum::<usize>()
            + self.stream.len();
        debug_assert_eq!(total_count, self.all_dses().count());
        self.scores.resize(total_count, 0.0);

        // take out of self for concurrent mutable access
        let mut scores = std::mem::take(&mut self.scores);

        // score all dses
        let mut context = IntelligenceContext::<C>::new(&mut blackboard, alloc);
        for ((dse, weight), score) in self.all_dses_with_weights().zip(scores.iter_mut()) {
            log_scope!(o!("dse" => dse.name()));

            // TODO add momentum to weight to discourage changing mind so often
            let dse_score = dse.score(&mut context, weight);
            trace!("DSE scored {score}", score = dse_score);

            if dse_score > context.best_so_far {
                context.best_so_far = dse_score;
            }

            *score = dse_score;
        }
        drop(context);

        // put scores back into self
        let empty = ManuallyDrop::new(std::mem::replace(&mut self.scores, scores));
        debug_assert!(empty.is_empty());

        // find the best score for initial choice
        let best = self
            .all_dses_with_scores()
            .max_by_key(|(_, _, score)| FloatOrd(*score));

        match best {
            Some((_, source, score)) => {
                self.decision_progress = Some(DecisionProgress::InitialChoice {
                    source: source.clone(),
                    score,
                    blackboard,
                });
                Some(InitialChoice { source, score })
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
    ) -> IntelligentDecision<C> {
        let _ = self.choose_with_stream_dses(blackboard, alloc, empty());
        self.consume_decision()
    }

    #[cfg(test)]
    pub fn choose_now_with_stream_dses(
        &mut self,
        blackboard: Box<C::Blackboard>,
        alloc: &bumpalo::Bump,
        streams: impl Iterator<Item = (WeightedDse<C>, C::StreamDseExtraData)>,
    ) -> IntelligentDecision<C> {
        let _ = self.choose_with_stream_dses(blackboard, alloc, streams);
        self.consume_decision()
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

    fn all_dses(&self) -> impl Iterator<Item = &dyn Dse<C>> {
        let base = self.base.0.iter().map(|dse| &**dse);

        let additional = self
            .additional
            .iter()
            .map(|(_, smarts)| smarts.0.iter().map(|dse| &**dse))
            .flatten();

        let stream = self.stream.iter().map(|(weighted, _)| weighted.dse());

        base.chain(additional).chain(stream)
    }

    fn all_dses_with_weights(&self) -> impl Iterator<Item = (&dyn Dse<C>, f32)> {
        let base = self.base.0.iter().map(|dse| (&**dse, 1.0));

        let additional = self
            .additional
            .iter()
            .map(|(_, smarts)| smarts.0.iter().map(|dse| (&**dse, 1.0)))
            .flatten();

        let stream = self
            .stream
            .iter()
            .map(|(weighted, _)| (weighted.dse(), weighted.weight()));

        base.chain(additional).chain(stream)
    }

    fn all_dses_with_sources(&self) -> impl Iterator<Item = (&dyn Dse<C>, DecisionSource<C>)> {
        let base = self
            .base
            .0
            .iter()
            .enumerate()
            .map(|(i, dse)| (&**dse, DecisionSource::Base(DseIndex(i))));

        let additional =
            self.additional
                .iter()
                .map(|(key, smarts)| {
                    smarts.0.iter().enumerate().map(move |(i, dse)| {
                        (&**dse, DecisionSource::Additional(*key, DseIndex(i)))
                    })
                })
                .flatten();

        let stream = self.stream.iter().enumerate().map(|(i, (weighted, data))| {
            (
                weighted.dse(),
                DecisionSource::Stream(DseIndex(i), data.clone()),
            )
        });

        base.chain(additional).chain(stream)
    }

    fn all_dses_with_scores(&self) -> impl Iterator<Item = (&dyn Dse<C>, DecisionSource<C>, f32)> {
        self.all_dses_with_sources()
            .zip(self.scores.iter())
            .map(|((dse, src), score)| (dse, src, *score))
    }

    pub fn dse(&self, source: &DecisionSource<C>) -> Option<&dyn Dse<C>> {
        match source {
            DecisionSource::Base(i) => self.base.0.get(i.0).map(|dse| &**dse),
            DecisionSource::Additional(key, i) => self
                .additional
                .get(key)
                .and_then(|dses| dses.0.get(i.0).map(|dse| &**dse)),
            DecisionSource::Stream(i, _) => {
                self.stream.get(i.0).map(|(weighted, _)| weighted.dse())
            }
        }
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
        self.ensure_modifications_allowed();

        trace!("clearing last action to Nop");
        self.last_action.replace(C::Action::default());
    }

    pub fn decision_in_progress(&self) -> Option<DecisionProgress<C>> {
        self.decision_progress.clone()
    }

    pub fn update_decision_in_progress(&mut self, new_progress: DecisionProgress<C>) {
        let progress = self
            .decision_progress
            .as_mut()
            .expect("decision should already be in progress");
        *progress = new_progress;
    }

    /// Arrives at final decision
    pub fn choose_best_with_skipper(&mut self, skipper: impl DseSkipper<C>) {
        let blackboard = match self.decision_progress.take() {
            Some(DecisionProgress::InitialChoice { blackboard, .. }) => blackboard,
            _ => unreachable!("unexpected decision progress"),
        };

        // sort scores but keep the original order
        self.sorted_score_indices.clear();
        let mut i = 0;
        self.sorted_score_indices
            .resize_with(self.scores.len(), || {
                let this = i;
                i += 1;
                this
            });

        {
            let scores_ref = &self.scores[..];
            self.sorted_score_indices.sort_unstable_by_key(|i| {
                debug_assert!(scores_ref.get(*i).is_some());
                let score = unsafe { scores_ref.get_unchecked(*i) };
                FloatOrd(*score)
            });
        }

        // find the best that shouldn't be skipped
        let new_choice = self
            .sorted_score_indices
            .iter()
            .rev() // best at the end
            .map(|idx| {
                self.all_dses_with_sources()
                    .nth(*idx)
                    .expect("dse index expected to be valid")
            })
            .find(|(dse, src)| !skipper.should_skip(*dse, src));

        self.decision_progress = Some(match new_choice {
            Some((_, source)) => DecisionProgress::Decided { source, blackboard },
            None => DecisionProgress::NoChoice,
        });
    }

    pub fn consume_decision(&mut self) -> IntelligentDecision<C> {
        let (action, source) = match self.decision_progress.take().expect("thinking expected") {
            DecisionProgress::NoChoice => {
                trace!("intelligence chose nothing");
                (C::Action::default(), None)
            }
            DecisionProgress::Decided {
                source,
                mut blackboard,
            }
            | DecisionProgress::InitialChoice {
                source,
                mut blackboard,
                ..
            } => {
                let dse = self.dse(&source).expect("dse source expected to be valid");
                trace!("intelligence chose {dse}", dse = dse.name(); "source" => ?source, "detail" => ?dse.as_debug());

                let action = dse.action(&mut blackboard);
                (action, Some((dse, source)))
            }
        };

        let last_action = self.last_action.replace(action.clone());
        if last_action == action {
            IntelligentDecision::Unchanged
        } else if let Some((dse, src)) = source {
            IntelligentDecision::New { dse, action, src }
        } else {
            IntelligentDecision::Undecided
        }
    }
}

// deriving incorrectly assumes C must be Debug too: https://github.com/rust-lang/rust/issues/26925
impl<C: Context> Debug for DecisionSource<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            DecisionSource::Base(i) => write!(f, "Base({:?})", i.0),
            DecisionSource::Additional(id, i) => write!(f, "Additional({:?}, {:?})", id, i.0),
            DecisionSource::Stream(i, _) => write!(f, "Stream({:?})", i.0),
        }
    }
}

impl<C: Context> Clone for DecisionSource<C> {
    fn clone(&self) -> Self {
        use DecisionSource::*;
        match self {
            Base(a) => Base(*a),
            Additional(a, b) => Additional(*a, *b),
            Stream(a, b) => Stream(*a, b.clone()),
        }
    }
}

impl<C: Context> Clone for DecisionProgress<C> {
    fn clone(&self) -> Self {
        use DecisionProgress::*;
        match self {
            NoChoice => NoChoice,
            InitialChoice {
                source,
                blackboard,
                score,
            } => InitialChoice {
                source: source.clone(),
                blackboard: blackboard.clone(),
                score: *score,
            },
            Decided { source, blackboard } => Decided {
                source: source.clone(),
                blackboard: blackboard.clone(),
            },
        }
    }
}

impl<'a, C: Context> IntelligenceContext<'a, C> {
    pub fn new(blackboard: &'a mut C::Blackboard, alloc: &'a Bump) -> Self {
        Self {
            blackboard,
            input_cache: InputCache::new(alloc),
            best_so_far: 0.0,
            alloc,
        }
    }
}

#[cfg(test)]
mod tests {
    use common::{bumpalo, once};

    use crate::consideration::Considerations;
    use crate::decision::WeightedDse;
    use crate::intelligence::DseIndex;
    use crate::test_utils::*;
    use crate::{AiBox, DecisionSource, DecisionWeight, Dse, Intelligence, IntelligentDecision};

    #[test]
    fn extra_dses() {
        let blackboard = Box::new(TestBlackboard { my_hunger: 0.5 });

        let dses = vec![
            AiBox::new(EatDse) as AiBox<dyn Dse<TestContext>>,
            AiBox::new(BadDse) as AiBox<dyn Dse<TestContext>>,
        ];

        let mut intelligence = Intelligence::new(dses.into_iter());
        let alloc = bumpalo::Bump::new();

        // eat wins
        assert!(matches!(
            intelligence.choose(blackboard.clone(), &alloc),
            IntelligentDecision::New {
                action: TestAction::Eat,
                ..
            }
        ));
        assert!(matches!(
            intelligence.choose(blackboard.clone(), &alloc),
            IntelligentDecision::Unchanged
        ));

        // add additional emergency
        intelligence.add_smarts(
            100,
            vec![AiBox::new(EmergencyDse) as AiBox<dyn Dse<TestContext>>].into_iter(),
        );
        assert!(matches!(
            intelligence.choose(blackboard.clone(), &alloc),
            IntelligentDecision::New {
                action: TestAction::CancelExistence,
                ..
            }
        ));

        // pop it, back to original
        intelligence.pop_smarts(&100);
        assert!(matches!(
            intelligence.choose(blackboard.clone(), &alloc),
            IntelligentDecision::New {
                action: TestAction::Eat,
                ..
            }
        ));

        // add emergency as stream
        let streams = once((WeightedDse::new(EmergencyDse, 1.0), ()));
        assert!(matches!(
            intelligence.choose_now_with_stream_dses(blackboard.clone(), &alloc, streams),
            IntelligentDecision::New {
                action: TestAction::CancelExistence,
                ..
            }
        ));
    }

    //noinspection DuplicatedCode
    #[test]
    fn society_task_reservation_weight() {
        let blackboard = Box::new(TestBlackboard { my_hunger: 0.5 });

        #[derive(Clone, Hash, Eq, PartialEq)]
        pub struct ConfigurableDse(TestAction);

        impl Dse<TestContext> for ConfigurableDse {
            fn considerations(&self, out: &mut Considerations<TestContext>) {
                out.add(ConstantConsideration(50));
            }

            fn weight(&self) -> DecisionWeight {
                DecisionWeight::Normal
            }

            fn action(&self, _: &mut TestBlackboard) -> TestAction {
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
        match intelligence.choose(blackboard.clone(), &alloc) {
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
            streams.into_iter(),
        ) {
            IntelligentDecision::New { action, src, .. } => {
                assert_eq!(action, TestAction::Eat); // changed
                assert!(matches!(src, DecisionSource::Stream(DseIndex(1), _)));
            }
            _ => unreachable!(),
        };
    }
}
