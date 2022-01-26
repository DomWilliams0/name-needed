use common::*;

use crate::bumpalo::Bump;
use crate::consideration::Considerations;
use crate::intelligence::IntelligenceContext;
use crate::{pretty_type_name, Context};

#[derive(Copy, Clone, Debug)]
pub enum DecisionWeightType {
    Idle,
    Normal,
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
    fn considerations(&self, out: &mut Considerations<C>);
    fn weight_type(&self) -> DecisionWeightType;
    fn action(&self, blackboard: &mut C::Blackboard) -> C::Action;

    fn name(&self) -> &'static str {
        let name = pretty_type_name(std::any::type_name::<Self>());
        name.strip_suffix("Dse").unwrap_or(name)
    }

    fn as_debug(&self) -> Option<&dyn Debug> {
        None
    }

    fn weight(&self) -> DecisionWeight {
        DecisionWeight::Plain(self.weight_type())
    }

    fn score(&self, context: &mut IntelligenceContext<C>, bonus: f32) -> f32 {
        // starts as the maximum possible score (i.e. all considerations are 1.0)
        let mut final_score = bonus;

        let considerations = {
            let mut considerations = Considerations::new(context.alloc);
            self.considerations(&mut considerations);
            considerations.into_vec()
        };

        let modification_factor = 1.0 - (1.0 / considerations.len() as f32);
        for c in considerations {
            if final_score < context.best_so_far {
                trace!("skipping {dse} due to falling below best result found so far", dse = self.name();
                       "current_score" => final_score, "best_so_far" => context.best_so_far);
                return 0.0;
            }

            let score = c
                .consider(context.blackboard, &mut context.input_cache)
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
                break;
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

impl<'a, C: Context> Debug for dyn Dse<C> + 'a {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let alloc = Bump::new();
        let mut considerations = Considerations::new(&alloc);
        self.considerations(&mut considerations);
        f.debug_struct("Dse")
            .field("name", &self.name())
            .field("weight", &self.weight())
            .field("considerations", &considerations)
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
    fn considerations(&self, out: &mut Considerations<C>) {
        self.dse.considerations(out)
    }

    fn weight_type(&self) -> DecisionWeightType {
        unreachable!()
    }

    fn action(&self, blackboard: &mut <C as Context>::Blackboard) -> <C as Context>::Action {
        self.dse.action(blackboard)
    }

    fn name(&self) -> &'static str {
        self.dse.name()
    }

    fn as_debug(&self) -> Option<&dyn Debug> {
        self.dse.as_debug()
    }

    fn weight(&self) -> DecisionWeight {
        DecisionWeight::Weighted(self.dse.weight_type(), self.additional_weight)
    }
}

#[cfg(test)]
mod tests {
    use crate::bumpalo::Bump;
    use crate::intelligence::{InputCache, IntelligenceContext};
    use crate::test_utils::*;
    use crate::*;

    #[test]
    fn score() {
        let mut blackboard = TestBlackboard { my_hunger: 0.5 };
        let alloc = Bump::new();
        let mut ctx = IntelligenceContext {
            blackboard: &mut blackboard,
            input_cache: InputCache::new(&alloc),
            best_so_far: 0.0,
            alloc: &Default::default(),
        };

        assert!(EatDse.score(&mut ctx, 1.0) > 0.9);
        assert!(BadDse.score(&mut ctx, 1.0) < 0.1);
    }
}
