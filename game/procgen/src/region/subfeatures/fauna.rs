use std::borrow::Cow;

use common::Itertools;
use unit::world::{WorldPoint, WorldPosition};
use world_types::{BlockType, EntityDescription, PlantDescription};

use crate::region::subfeature::{Rasterizer, Subfeature, SubfeatureEntity};

#[derive(Debug)]
pub struct Fauna {
    pub species: &'static str,
}

impl Subfeature for Fauna {
    fn rasterize(
        &mut self,
        root: WorldPosition,
        _rasterizer: &mut Rasterizer,
    ) -> Option<SubfeatureEntity> {
        // no blocks

        let position = {
            // TODO randomise fauna position within block
            let pos = root.centred();
            WorldPoint::new_unchecked(pos.x(), pos.y(), pos.z() + 1.0)
        };
        Some(SubfeatureEntity(EntityDescription {
            position,
            desc: PlantDescription {
                species: Cow::Borrowed(self.species),
            },
        }))
    }
}
