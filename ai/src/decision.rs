use std::any::{Any, TypeId};
use std::hash::Hasher;

use common::bumpalo::Bump;
use common::*;

use crate::consideration::Considerations;
use crate::context::pretty_type_name;

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

#[derive(Derivative)]
#[derivative(Clone(bound = ""))]
pub struct WeightedDse<C: Context> {
    // TODO cow type for dse (aibox, framealloc, borrowed)
    dse: AiBox<dyn Dse<C>>,
    multiplier: f32,
}

/// For emitting targets in a DSE
pub struct Targets<'a, C: Context>(BumpVec<'a, C::DseTarget>);

pub enum TargetOutput {
    Untargeted,
    TargetsCollected,
}

pub trait Dse<C: Context>: DseExt<C> {
    fn considerations(&self, out: &mut Considerations<C>);
    fn weight(&self) -> DecisionWeight;

    /// Calculate targets for each instance of this DSE. Must return [TargetsCollected] if an
    /// attempt to find targets is made
    #[allow(unused_variables)]
    fn target(&self, targets: &mut Targets<C>, blackboard: &mut C::Blackboard) -> TargetOutput {
        TargetOutput::Untargeted
    }

    fn action(&self, blackboard: &mut C::Blackboard, target: Option<C::DseTarget>) -> C::Action;

    fn name(&self) -> &'static str {
        let name = pretty_type_name(std::any::type_name::<Self>());
        name.strip_suffix("Dse").unwrap_or(name)
    }

    fn as_debug(&self) -> Option<&dyn Debug> {
        None
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

    pub fn multiplier(&self) -> f32 {
        self.multiplier
    }

    pub fn weight(&self) -> f32 {
        self.dse.weight().multiplier() * self.multiplier
    }
}

impl<'a, C: Context> Targets<'a, C> {
    pub fn new(alloc: &'a Bump) -> Self {
        Self(BumpVec::new_in(alloc))
    }

    pub fn add(&mut self, target: C::DseTarget) {
        trace!("adding target"; "target" => ?target);
        debug_assert!(!self.0.contains(&target), "duplicate target");
        self.0.push(target);
    }

    pub fn drain(&mut self) -> impl Iterator<Item = C::DseTarget> + '_ {
        self.0.drain(..)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    use crate::test_utils::*;
    use crate::*;

    macro_rules! impl_dse {
        ($ty:ty) => {
            impl Dse<TestContext> for $ty {
                fn considerations(&self, _: &mut Considerations<TestContext>) {
                    unreachable!()
                }

                fn weight(&self) -> DecisionWeight {
                    unreachable!()
                }

                fn action(&self, _: &mut TestBlackboard, _: Option<u32>) -> TestAction {
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
