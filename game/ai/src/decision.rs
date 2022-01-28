use std::any::{Any, TypeId};
use std::hash::Hasher;

use common::bumpalo::Bump;
use common::*;

use crate::consideration::Considerations;
use crate::context::pretty_type_name;
use crate::intelligence::IntelligenceContext;
use crate::{AiBox, Context};

#[derive(Copy, Clone, Debug)]
pub enum DecisionWeight {
    Idle,
    Normal,
    BasicNeeds,
    Emergency,
    /// Obedience without question, for dev mode and debugging
    AbsoluteOverride,
}

pub trait DseExt<C: Context>: Any {
    fn clone_dse(&self) -> AiBox<dyn Dse<C>>;
    fn compare_dse(&self, other: &dyn Dse<C>) -> bool;
    fn hash_dse(&self, hasher: &mut dyn Hasher);
}

pub struct WeightedDse<C: Context> {
    // TODO cow type for dse (aibox, framealloc, borrowed)
    dse: AiBox<dyn Dse<C>>,
    multiplier: f32,
}

pub trait Dse<C: Context>: DseExt<C> {
    fn considerations(&self, out: &mut Considerations<C>);
    fn weight(&self) -> DecisionWeight;
    fn action(&self, blackboard: &mut C::Blackboard) -> C::Action;

    fn name(&self) -> &'static str {
        let name = pretty_type_name(std::any::type_name::<Self>());
        name.strip_suffix("Dse").unwrap_or(name)
    }

    fn as_debug(&self) -> Option<&dyn Debug> {
        None
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
                return final_score;
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

        debug_assert!(final_score <= bonus);
        final_score
    }
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

impl<C, D> DseExt<C> for D
where
    C: Context,
    D: Dse<C> + Clone + Eq + Hash + 'static,
{
    fn clone_dse(&self) -> AiBox<dyn Dse<C>> {
        AiBox::new(self.clone())
    }

    fn compare_dse(&self, other: &dyn Dse<C>) -> bool {
        if other.type_id() == self.type_id() {
            // safety: compared types
            let other = unsafe { &*(other as *const dyn Dse<C> as *const D) };
            self == other
        } else {
            false
        }
    }

    fn hash_dse(&self, mut hasher: &mut dyn Hasher) {
        TypeId::of::<D>().hash(&mut hasher); // ensure different hash for different types
        self.hash(&mut hasher);
    }
}

impl<C: Context> Clone for AiBox<dyn Dse<C>> {
    fn clone(&self) -> Self {
        self.clone_dse()
    }
}

impl<C: Context> Clone for WeightedDse<C> {
    fn clone(&self) -> Self {
        Self {
            dse: self.dse.clone(),
            multiplier: self.multiplier,
        }
    }
}

impl<C: Context> PartialEq<dyn Dse<C>> for dyn Dse<C> {
    fn eq(&self, other: &dyn Dse<C>) -> bool {
        self.compare_dse(other)
    }
}

impl<C: Context> Eq for dyn Dse<C> {}

impl<C: Context> Hash for dyn Dse<C> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash_dse(state);
    }
}

impl DecisionWeight {
    pub fn multiplier(self) -> f32 {
        use DecisionWeight::*;
        match self {
            Idle => 1.0,
            Normal => 2.0,
            BasicNeeds => 3.5,
            Emergency => 4.0,
            AbsoluteOverride => 1000.0,
        }
    }
}

impl<C: Context> WeightedDse<C> {
    pub fn new(dse: impl Dse<C> + 'static, weight: f32) -> Self {
        assert!(weight.is_sign_positive() && weight.is_finite());
        Self {
            dse: AiBox::new(dse) as AiBox<dyn Dse<C>>,
            multiplier: weight,
        }
    }

    pub fn dse(&self) -> &dyn Dse<C> {
        &*self.dse
    }

    pub fn weight(&self) -> f32 {
        self.dse.weight().multiplier() * self.multiplier
    }
}

#[cfg(test)]
mod tests {
    use crate::intelligence::IntelligenceContext;
    use crate::test_utils::*;
    use crate::*;
    use common::bumpalo::Bump;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    #[test]
    fn score() {
        let mut blackboard = TestBlackboard { my_hunger: 0.5 };
        let alloc = Bump::new();
        let mut ctx = IntelligenceContext::new(&mut blackboard, &alloc);

        assert!(EatDse.score(&mut ctx, 1.0) > 0.9);
        assert!(BadDse.score(&mut ctx, 1.0) < 0.1);
    }

    macro_rules! impl_dse {
        ($ty:ty) => {
            impl Dse<TestContext> for $ty {
                fn considerations(&self, _: &mut Considerations<TestContext>) {
                    unreachable!()
                }

                fn weight(&self) -> DecisionWeight {
                    unreachable!()
                }

                fn action(&self, _: &mut TestBlackboard) -> TestAction {
                    unreachable!()
                }
            }
        };
    }

    #[test]
    fn compare_dses() {
        #[derive(Hash, Eq)]
        struct A(u32);

        #[derive(Hash, Eq)]
        struct B;

        impl_dse!(A);
        impl_dse!(B);

        impl Clone for A {
            fn clone(&self) -> Self {
                eprintln!("clone A");
                Self(self.0)
            }
        }

        impl PartialEq for A {
            fn eq(&self, other: &Self) -> bool {
                eprintln!("compare A");
                self.0 == other.0
            }
        }

        impl Clone for B {
            fn clone(&self) -> Self {
                eprintln!("clone B");
                Self
            }
        }

        impl PartialEq for B {
            fn eq(&self, _: &Self) -> bool {
                eprintln!("compare B");
                true
            }
        }

        let a1 = Box::new(A(10)) as Box<dyn Dse<TestContext>>;
        let a2 = a1.clone();
        let a3 = Box::new(A(20)) as Box<dyn Dse<TestContext>>;
        let b = Box::new(B) as Box<dyn Dse<TestContext>>;

        assert_eq!(&a1, &a1);
        assert_eq!(&a1, &a2);
        assert_eq!(&a2, &a1);
        assert_ne!(&a1, &a3);

        assert_ne!(&a1, &b);
        assert_ne!(&a2, &b);
        assert_ne!(&a3, &b);
        assert_ne!(&b, &a1);

        assert_eq!(&b, &b);
    }

    #[test]
    fn hash_dse() {
        #[derive(Hash, Eq, PartialEq, Clone)]
        struct A(i32, &'static str);
        #[derive(Hash, Eq, PartialEq, Clone)]
        struct B(i32, &'static str); // same fields

        impl_dse!(A);
        impl_dse!(B);

        fn do_hash(dse: &dyn Dse<TestContext>) -> u64 {
            let mut hasher = DefaultHasher::new();
            dse.hash(&mut hasher);
            hasher.finish()
        }

        let a1 = do_hash(&A(50, "nice"));
        let a2 = do_hash(&A(50, "nice"));
        let b1 = do_hash(&B(50, "nice"));

        // same type
        assert_eq!(a1, a2);

        // same fields but different type
        assert_ne!(a1, b1, "different types have the same hash");
    }
}
