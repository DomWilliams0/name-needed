use float_ord::FloatOrd;

use crate::consideration::InputCache;
use crate::decision::Dse;
use crate::{AiBox, Context};
use common::*;

struct Decision<C: Context> {
    dse: AiBox<dyn Dse<C>>,
    score: f32,
}

// TODO pool/arena
/// Non-empty collection of DSEs and scores
pub struct Smarts<C: Context>(Vec<Decision<C>>);

pub struct Intelligence<C: Context> {
    base: Smarts<C>,
    additional: Vec<Smarts<C>>,
    last_decision: C::Action,
    input_cache: InputCache<C>,
}

pub enum IntelligentDecision<'a, C: Context> {
    Unchanged,
    New {
        dse: &'a dyn Dse<C>,
        action: C::Action,
    },
}

impl<C: Context> Smarts<C> {
    pub fn new(dses: impl Iterator<Item = AiBox<dyn Dse<C>>>) -> Option<Self> {
        let dses: Vec<_> = dses.map(move |dse| Decision { dse, score: 0.0 }).collect();
        if dses.is_empty() {
            None
        } else {
            Some(Self(dses))
        }
    }

    pub fn score(&mut self, input_cache: &mut InputCache<C>, blackboard: &mut C::Blackboard) {
        // TODO optimize
        for Decision { dse, score } in &mut self.0 {
            // TODO + momentum to discourage changing so often
            let bonus = dse.weight().multiplier();

            *score = dse.score(blackboard, input_cache, bonus);
            trace!("DSE '{}' scored {:?}", dse.name(), *score);
        }
    }
}

impl<C: Context> Intelligence<C> {
    pub fn new(base_smarts: Smarts<C>) -> Self {
        Self {
            base: base_smarts,
            additional: Vec::new(),
            last_decision: Default::default(),
            input_cache: InputCache::default(),
        }
    }

    pub fn choose(&mut self, blackboard: &mut C::Blackboard) -> IntelligentDecision<C> {
        self.input_cache.reset();

        // score all possible decisions
        self.base.score(&mut self.input_cache, blackboard);
        for dse in self.additional.iter_mut() {
            dse.score(&mut self.input_cache, blackboard)
        }

        // choose the best
        // TODO dumber agents shouldn't always choose the best
        let choice = self
            .base
            .0
            .iter()
            .chain(self.additional.iter().map(|s| s.0.iter()).flatten())
            .max_by_key(|d| FloatOrd(d.score))
            .unwrap()
            .dse
            .as_ref();

        trace!("intelligence chose DSE {}", choice.name());

        let action = choice.action(blackboard);
        let last_decision = std::mem::replace(&mut self.last_decision, action.clone());

        if action == last_decision {
            IntelligentDecision::Unchanged
        } else {
            IntelligentDecision::New {
                dse: choice,
                action,
            }
        }
    }

    pub fn drain_input_cache(&mut self) -> impl Iterator<Item = (C::Input, f32)> + '_ {
        self.input_cache.drain()
    }
}
