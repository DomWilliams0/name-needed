use std::fmt::Debug;
use std::hash::Hash;

pub trait Context: Sized + 'static {
    type Blackboard: Blackboard;
    type Input: Input<Self>;
    type Action: Default + Eq + Clone;
    type AdditionalDseId: Hash + Eq + Copy + Debug;
    type StreamDseExtraData: Clone;
    type DseTarget: PartialEq + Clone + Debug;
}

pub trait Input<C: Context>: Hash + Clone + Eq {
    fn get(&self, blackboard: &mut C::Blackboard, target: Option<&C::DseTarget>) -> f32;
}

pub trait Blackboard: Clone {
    #[cfg(feature = "logging")]
    fn entity(&self) -> String;
}

// TODO use a separate allocator for ai to avoid fragmentation
pub type AiBox<T> = Box<T>;

pub(crate) fn pretty_type_name(name: &str) -> &str {
    let split_idx = name.rfind(':').map(|i| i + 1).unwrap_or(0);
    &name[split_idx..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_type_names() {
        assert_eq!(pretty_type_name("this::is::my::type::Lmao"), "Lmao");
        assert_eq!(pretty_type_name("boop"), "boop");
        assert_eq!(pretty_type_name("malformed:"), "");
        assert_eq!(pretty_type_name(":malformed"), "malformed");
    }
}
