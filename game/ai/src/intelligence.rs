use float_ord::FloatOrd;

use crate::consideration::InputCache;
use crate::decision::Dse;
use crate::{AiBox, Context};
use common::*;
use std::cell::Cell;
use std::collections::HashMap;

// TODO pool/arena allocator
/// Collection of DSEs
pub struct Smarts<C: Context> {
    decisions: Vec<Decision<C>>,
}

pub struct Intelligence<C: Context> {
    /// Unchanging base behaviours e.g. from species
    base: Smarts<C>,

    /// Additional, temporary behaviours based on context e.g. in a particular location
    additional: HashMap<C::AdditionalDseId, Smarts<C>>,

    last_action: Cell<C::Action>,
    input_cache: InputCache<C>,
}

pub enum IntelligentDecision<'a, C: Context> {
    Unchanged,
    New {
        dse: &'a dyn Dse<C>,
        action: C::Action,
        src: DecisionSource<C>,
    },
}

#[derive(Copy, Clone)]
pub enum DecisionSource<C: Context> {
    Base(usize),
    Additional(C::AdditionalDseId, usize),
    Stream(usize),
}

struct Decision<C: Context> {
    dse: AiBox<dyn Dse<C>>,
    score: f32,
}

impl<C: Context> Smarts<C> {
    pub fn new(dses: impl Iterator<Item = AiBox<dyn Dse<C>>>) -> Self {
        let decisions: Vec<_> = dses.map(Decision::new).collect();
        if decisions.is_empty() {
            warn!("smarts has zero DSEs");
        }
        Self { decisions }
    }

    pub fn score(&mut self, input_cache: &mut InputCache<C>, blackboard: &mut C::Blackboard) {
        let dses = self.decisions.iter_mut().map(Decision::as_mut);
        Self::score_dses(input_cache, blackboard, dses)
    }

    fn score_dses<'dse>(
        input_cache: &mut InputCache<C>,
        blackboard: &mut C::Blackboard,
        dses: impl Iterator<Item = (&'dse dyn Dse<C>, &'dse mut f32)>,
    ) where
        C: 'dse,
    {
        // TODO optimize: not all decisions need to be checked each time, but at least zero all scores
        // TODO DSEs should be immutable, with scores stored somewhere else e.g. parallel array
        for (dse, score) in dses {
            // TODO add momentum to discourage changing mind so often
            let bonus = dse.weight().multiplier();

            log_scope!(o!("dse" => dse.name()));
            *score = dse.score(blackboard, input_cache, bonus);
            trace!("DSE scored {score}", score = *score);
        }
    }
}

impl<C: Context> Intelligence<C> {
    pub fn new(base_dses: impl Iterator<Item = AiBox<dyn Dse<C>>>) -> Self {
        let base = Smarts::new(base_dses);
        assert!(
            !base.decisions.is_empty(),
            "at least 1 DSE needed for species"
        );
        Self {
            base,
            additional: HashMap::new(),
            last_action: Default::default(),
            input_cache: InputCache::default(),
        }
    }

    pub fn choose<'a>(&'a mut self, blackboard: &'a mut C::Blackboard) -> IntelligentDecision<C> {
        self.choose_with_stream_dses(blackboard, empty())
    }

    /// "Stream" behaviours only apply to a single tick, avoiding the overhead of adding then
    /// immediately removing additional behaviours
    pub fn choose_with_stream_dses<'a, 'b>(
        &'a mut self,
        blackboard: &'b mut C::Blackboard,
        streams: impl Iterator<Item = &'b dyn Dse<C>>,
    ) -> IntelligentDecision<'b, C>
    where
        'a: 'b,
    {
        self.input_cache.reset();

        // score all possible decisions
        self.base.score(&mut self.input_cache, blackboard);
        for (_, smarts) in self.additional.iter_mut() {
            smarts.score(&mut self.input_cache, blackboard)
        }

        // score streams in a parallel array of scores
        // TODO reuse allocation
        let mut streams: Vec<_> = streams.map(|dse| (dse, 0.0f32)).collect();
        Smarts::score_dses(
            &mut self.input_cache,
            blackboard,
            streams.iter_mut().map(|(dse, score)| (*dse, score)),
        );

        // choose the best out of all scores
        let (choice, _, choice_src) = {
            let decision_scores = self.all_decisions();
            let stream_scores = streams
                .iter()
                .enumerate()
                .map(|(i, (dse, score))| (*dse, *score, DecisionSource::Stream(i)));

            let all_scores = decision_scores.chain(stream_scores);

            all_scores
                .max_by_key(|(_, score, _)| FloatOrd(*score))
                .unwrap() // not empty
        };

        trace!("intelligence chose {dse}", dse = choice.name(); "source" => ?choice_src);

        let action = choice.action(blackboard);
        let last_action = self.last_action.replace(action.clone());

        if action == last_action {
            IntelligentDecision::Unchanged
        } else {
            IntelligentDecision::New {
                dse: choice,
                action,
                src: choice_src,
            }
        }
    }

    pub fn drain_input_cache(&mut self) -> impl Iterator<Item = (C::Input, f32)> + '_ {
        self.input_cache.drain()
    }

    // TODO benchmark adding and popping smarts

    pub fn add_smarts(
        &mut self,
        id: C::AdditionalDseId,
        dses: impl Iterator<Item = AiBox<dyn Dse<C>>>,
    ) {
        let smarts = Smarts::new(dses);
        let count = smarts.decisions.len();
        if let Some(old) = self.additional.insert(id.clone(), smarts) {
            // TODO reuse allocation
            debug!(
                "replaced {prev_count} additional DSEs with {count}",
                prev_count = old.decisions.len(),
                count = count;
                "dse_id" => ?id
            );
        }
    }

    pub fn pop_smarts(&mut self, id_to_remove: &C::AdditionalDseId) {
        if self.additional.remove(id_to_remove).is_none() {
            warn!(
                "didn't have any additional smarts to remove";
                "dse_id" => ?id_to_remove
            );
        }
    }

    fn all_decisions(&self) -> impl Iterator<Item = (&dyn Dse<C>, f32, DecisionSource<C>)> {
        let base = self
            .base
            .decisions
            .iter()
            .enumerate()
            .map(|(i, d)| (DecisionSource::Base(i), d));

        let additional = self
            .additional
            .iter()
            .map(|(key, smarts)| {
                smarts
                    .decisions
                    .iter()
                    .enumerate()
                    .map(move |(i, d)| (DecisionSource::Additional(key.clone(), i), d))
            })
            .flatten();

        base.chain(additional)
            .map(|(src, decision)| (decision.dse.as_ref(), decision.score, src))
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
}

impl<C: Context> Decision<C> {
    fn new(dse: AiBox<dyn Dse<C>>) -> Self {
        Self { dse, score: 0.0 }
    }

    fn as_mut(&mut self) -> (&dyn Dse<C>, &mut f32) {
        (self.dse.as_ref(), &mut self.score)
    }
}

// deriving incorrectly assumes C must be Debug too: https://github.com/rust-lang/rust/issues/26925
impl<C: Context> Debug for DecisionSource<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            DecisionSource::Base(i) => write!(f, "Base({:?})", i),
            DecisionSource::Additional(id, i) => write!(f, "Additional({:?}, {:?})", id, i),
            DecisionSource::Stream(i) => write!(f, "Stream({:?})", i),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::decision::WeightedDse;
    use crate::test_utils::*;
    use crate::{
        AiBox, Consideration, DecisionSource, DecisionWeightType, Dse, Intelligence,
        IntelligentDecision,
    };
    use common::once;

    #[test]
    fn extra_dses() {
        let mut blackboard = TestBlackboard { my_hunger: 0.5 };

        let dses = vec![
            AiBox::new(EatDse) as AiBox<dyn Dse<TestContext>>,
            AiBox::new(BadDse) as AiBox<dyn Dse<TestContext>>,
        ];

        let mut intelligence = Intelligence::new(dses.into_iter());

        // eat wins
        assert!(matches!(
            intelligence.choose(&mut blackboard),
            IntelligentDecision::New {
                action: TestAction::Eat,
                ..
            }
        ));
        assert!(matches!(
            intelligence.choose(&mut blackboard),
            IntelligentDecision::Unchanged
        ));

        // add additional emergency
        intelligence.add_smarts(
            100,
            vec![AiBox::new(EmergencyDse) as AiBox<dyn Dse<TestContext>>].into_iter(),
        );
        assert!(matches!(
            intelligence.choose(&mut blackboard),
            IntelligentDecision::New {
                action: TestAction::CancelExistence,
                ..
            }
        ));

        // pop it, back to original
        intelligence.pop_smarts(&100);
        assert!(matches!(
            intelligence.choose(&mut blackboard),
            IntelligentDecision::New {
                action: TestAction::Eat,
                ..
            }
        ));

        // add emergency as stream
        let emergency = Box::new(EmergencyDse);
        let streams = once(emergency.as_ref() as &dyn Dse<TestContext>);
        assert!(matches!(
            intelligence.choose_with_stream_dses(&mut blackboard, streams),
            IntelligentDecision::New {
                action: TestAction::CancelExistence,
                ..
            }
        ));
    }

    //noinspection DuplicatedCode
    #[test]
    fn society_task_reservation_weight() {
        let mut blackboard = TestBlackboard { my_hunger: 0.5 };

        pub struct ConfigurableDse(TestAction);

        impl Dse<TestContext> for ConfigurableDse {
            fn name(&self) -> &'static str {
                "Configurable"
            }

            fn considerations(&self) -> Vec<AiBox<dyn Consideration<TestContext>>> {
                vec![AiBox::new(ConstantConsideration(50))]
            }

            fn weight_type(&self) -> DecisionWeightType {
                DecisionWeightType::Normal
            }

            fn action(&self, _: &mut TestBlackboard) -> TestAction {
                self.0.clone()
            }
        }

        let dses =
            vec![AiBox::new(ConfigurableDse(TestAction::Eat)) as AiBox<dyn Dse<TestContext>>];

        let mut intelligence = Intelligence::new(dses.into_iter());

        // choose the only available Eat
        match intelligence.choose(&mut blackboard) {
            IntelligentDecision::New { action, src, .. } => {
                assert_eq!(action, TestAction::Eat);
                assert!(matches!(src, DecisionSource::Base(0)));
            }
            _ => unreachable!(),
        };

        // add a weighted dse
        intelligence.add_smarts(
            5,
            once(
                AiBox::new(WeightedDse::new(ConfigurableDse(TestAction::Nop), 1.1))
                    as AiBox<dyn Dse<TestContext>>,
            ),
        );

        // choose the new weighted Nop
        match intelligence.choose(&mut blackboard) {
            IntelligentDecision::New { action, src, .. } => {
                assert_eq!(action, TestAction::Nop); // changed
                assert!(matches!(src, DecisionSource::Additional(5, 0)));
            }
            _ => unreachable!(),
        };

        // back to a higher Eat
        intelligence.add_smarts(
            8,
            once(
                AiBox::new(WeightedDse::new(ConfigurableDse(TestAction::Eat), 1.9))
                    as AiBox<dyn Dse<TestContext>>,
            ),
        );

        match intelligence.choose(&mut blackboard) {
            IntelligentDecision::New { action, src, .. } => {
                assert_eq!(action, TestAction::Eat); // changed
                assert!(matches!(src, DecisionSource::Additional(8, 0)));
            }
            _ => unreachable!(),
        };
    }
}
