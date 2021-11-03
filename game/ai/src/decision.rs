use common::*;

use crate::consideration::InputCache;
use crate::{pretty_type_name, AiBox, Consideration, Context};

#[derive(Copy, Clone, Debug)]
pub enum DecisionWeightType {
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
#[derive(Copy, Clone, Debug)]

pub enum DecisionWeight {
    Plain(DecisionWeightType),
    /// Extra multiplier
    Weighted(DecisionWeightType, f32),
}

pub trait Dse<C: Context> {
    /// TODO pooled vec/slice rather than Vec each time
    fn considerations(&self) -> Vec<AiBox<dyn Consideration<C>>>;
    fn weight_type(&self) -> DecisionWeightType;
    fn action(&self, blackboard: &mut C::Blackboard) -> C::Action;

    fn name(&self) -> &'static str {
        let name = pretty_type_name(std::any::type_name::<Self>());
        name.strip_suffix("Dse").unwrap_or(name)
    }

    fn weight(&self) -> DecisionWeight {
        DecisionWeight::Plain(self.weight_type())
    }

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

            trace!("consideration scored {score}", score = evaluated_score; "consideration" => c.name(), "raw" => score);

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

/// A DSE with an additional weight multiplier
pub struct WeightedDse<C: Context, D: Dse<C>> {
    dse: D,
    additional_weight: f32,
    phantom: PhantomData<C>,
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
        use DecisionWeightType::*;
        let ty = match self {
            DecisionWeight::Plain(ty) | DecisionWeight::Weighted(ty, _) => ty,
        };

        let mut weight = match ty {
            Idle => 1.0,
            Normal => 2.0,
            Dependent => 2.5,
            BasicNeeds => 3.5,
            Emergency => 4.0,
            AbsoluteOverride => 1000.0,
        };

        if let DecisionWeight::Weighted(_, mul) = self {
            weight *= mul;
        }

        weight
    }
}

impl From<DecisionWeightType> for DecisionWeight {
    fn from(ty: DecisionWeightType) -> Self {
        Self::Plain(ty)
    }
}

impl<C: Context, D: Dse<C>> WeightedDse<C, D> {
    pub fn new(dse: D, weight: f32) -> Self {
        debug_assert!(weight.is_sign_positive());
        Self {
            dse,
            additional_weight: weight,
            phantom: PhantomData,
        }
    }
}

impl<C: Context, D: Dse<C>> Dse<C> for WeightedDse<C, D> {
    fn name(&self) -> &'static str {
        self.dse.name()
    }

    fn considerations(&self) -> Vec<AiBox<dyn Consideration<C>>> {
        self.dse.considerations()
    }

    fn weight_type(&self) -> DecisionWeightType {
        unreachable!()
    }

    fn action(&self, blackboard: &mut <C as Context>::Blackboard) -> <C as Context>::Action {
        self.dse.action(blackboard)
    }

    fn weight(&self) -> DecisionWeight {
        DecisionWeight::Weighted(self.dse.weight_type(), self.additional_weight)
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
