use common::*;

use crate::consideration::InputCache;
use crate::{AiBox, Consideration, Context};

#[derive(Copy, Clone, Debug)]
pub enum DecisionWeight {
    Idle,
    Normal,
    /// This normally follows another decision and is disabled by a switch - once the switch toggles
    /// this is more likely to be chosen
    Dependent,
    BasicNeeds,
    Emergency,
    /// Obedience without question, for dev mode and debugging
    AbsoluteOverride,
}

pub trait Dse<C: Context> {
    fn name(&self) -> &str;
    /// TODO pooled vec/slice rather than Vec each time
    fn considerations(&self) -> Vec<AiBox<dyn Consideration<C>>>;
    fn weight(&self) -> DecisionWeight;
    fn action(&self, blackboard: &mut C::Blackboard) -> C::Action;

    fn score(
        &self,
        blackboard: &mut C::Blackboard,
        input_cache: &mut InputCache<C>,
        bonus: f32,
    ) -> f32 {
        let mut final_score = bonus;

        let considerations = self.considerations();
        let modification_factor = 1.0 - (1.0 / considerations.len() as f32);
        for c in considerations.iter() {
            // TODO optimization: dont consider all considerations every time

            let score = c.consider(blackboard, input_cache).value();

            // compensation factor balances overall drop when multiplying multiple floats by
            // taking into account the number of considerations
            let make_up_value = (1.0 - score) * modification_factor;
            let compensated_score = score + (make_up_value * score);
            debug_assert!(compensated_score <= 1.0);

            let evaluated_score = c
                .curve()
                .evaluate(NormalizedFloat::new(compensated_score))
                .value();
            trace!(
                "consideration '{}' raw value is {:?} and scored {:?}",
                c.name(),
                score,
                evaluated_score
            );

            #[cfg(feature = "logging")]
            {
                use crate::Blackboard;
                c.log_metric(&blackboard.entity(), evaluated_score);
            }

            final_score *= evaluated_score;
        }

        final_score * self.weight().multiplier()
    }
}

impl<C: Context> Debug for dyn Dse<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("Dse")
            .field("name", &self.name())
            .field("weight", &self.weight())
            .field("considerations", &self.considerations().len())
            .finish()
    }
}

impl DecisionWeight {
    pub fn multiplier(self) -> f32 {
        match self {
            DecisionWeight::Idle => 1.0,
            DecisionWeight::Normal => 2.0,
            DecisionWeight::Dependent => 2.5,
            DecisionWeight::BasicNeeds => 3.5,
            DecisionWeight::Emergency => 4.0,
            DecisionWeight::AbsoluteOverride => 1000.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::consideration::InputCache;

    use crate::test_utils::*;
    use crate::*;

    #[test]
    fn score() {
        let mut blackboard = TestBlackboard { my_hunger: 0.5 };
        let mut cache = InputCache::default();

        assert!(EatDse.score(&mut blackboard, &mut cache, 1.0) > 0.9);
        assert!(BadDse.score(&mut blackboard, &mut cache, 1.0) < 0.1);
    }
}
