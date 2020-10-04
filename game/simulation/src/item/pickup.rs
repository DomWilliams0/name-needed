use std::borrow::Cow;

use common::*;
use unit::world::WorldPoint;

use crate::ecs::*;
use crate::item::ItemFilter;

/// Newtype to compare GoPickupItem just by the filter and number of results, and includes a
/// description of the items to pick up.
/// Items are in *reverse desirability order* - last is the most desirable, pop that
/// and try the next last if that becomes unavailable
#[derive(Debug, Clone)]
pub struct ItemsToPickUp(
    pub Cow<'static, str>,
    pub ItemFilter,
    pub Vec<(Entity, WorldPoint)>,
);

impl PartialEq for ItemsToPickUp {
    fn eq(&self, other: &Self) -> bool {
        if self.0 == other.0 {
            // consider equal if the number of matching items is within a margin
            const MARGIN: usize = 16;

            let diff = {
                let a = self.2.len();
                let b = other.2.len();
                if a > b {
                    a - b
                } else {
                    b - a
                }
            };

            diff <= MARGIN
        } else {
            false
        }
    }
}

impl Eq for ItemsToPickUp {}
